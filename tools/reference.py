"""Oráculos externos CPU: llama2.c original F32 y fórmulas explícitas F32/F64."""

import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import sys
import urllib.request
from unittest.mock import patch

# Antes de importar el runtime numérico, incluidos sus backends BLAS.
for variable in ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "VECLIB_MAXIMUM_THREADS"):
    os.environ[variable] = "1"

import numpy as np
import torch

from import_model import MAX_BYTES, MAX_TENSORS, decode_package
from prepare_tiny import ROOT, PreparationError, load_checkpoint, prepare_sources, sha256, verify_blob, verify_inputs, write_json


class ReferenceError(ValueError):
    """Divergencia, dtype incorrecto o referencia no finita."""


class Trace:
    def __init__(self, dtype):
        self.dtype = dtype
        self.values = {}

    def put(self, name, value):
        if value.dtype != self.dtype or value.device.type != "cpu":
            raise ReferenceError(f"dtype_or_device: {name}: {value.dtype}/{value.device}")
        if not torch.isfinite(value).all().item():
            raise ReferenceError(f"nonfinite: {name}")
        if name in self.values:
            raise ReferenceError(f"duplicate_stage: {name}")
        self.values[name] = value.detach().contiguous().clone()
        return value


def upstream_module(source_dir, *, offline):
    identity = json.loads((ROOT / "tools/reference.sources.json").read_text())
    path = source_dir / identity["name"]
    if not path.exists():
        if offline:
            raise PreparationError(f"missing_reference_source: {path}")
        url = f"https://raw.githubusercontent.com/karpathy/llama2.c/{identity['revision']}/model.py"
        with urllib.request.urlopen(url, timeout=60) as response:
            data = response.read(identity["size"] + 1)
        verify_blob(data, identity)
        source_dir.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    verify_blob(path.read_bytes(), identity)
    spec = importlib.util.spec_from_file_location("tiny_original_model", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module, identity


def canonical_weights(package, state):
    data = package.read_bytes()
    config, tensors = decode_package(data, max_bytes=MAX_BYTES, max_tensors=MAX_TENSORS)
    if tuple(config.values()) != (64, 172, 5, 8, 4, 512, 512, 16):
        raise ReferenceError("canonical_config")
    weights = {}
    for tensor in tensors:
        value = torch.from_numpy(np.frombuffer(tensor.data, dtype="<f4").copy()).reshape(tensor.shape)
        if tensor.kind <= 12 and tensor.data != state[tensor.name].numpy().astype("<f4", copy=False).tobytes():
            raise ReferenceError(f"canonical_weight_bits: {tensor.name}")
        weights[tensor.name] = value
    # También comprobar procedencia de las constantes regeneradas y el directorio.
    from import_model import model_tensors
    if tensors != model_tensors(state):
        raise ReferenceError("canonical_inventory_or_constants")
    return weights, sha256(data)


def pack_causal(value):
    """[1,H,T,T] -> [H,T*(T+1)/2], filas concatenadas en orden t,j<=t."""
    return torch.cat([value[0, :, t, :t + 1] for t in range(value.shape[-1])], dim=-1)


def original_reference(module, args, state, ids):
    model = module.Transformer(module.ModelArgs(**args))
    model.load_state_dict(state, strict=True)
    model.eval()
    # Elegir la rama manual existente. El código fuente se conserva sin editar.
    for block in model.layers:
        attention = block.attention
        attention.flash = False
        if not hasattr(attention, "mask"):
            mask = torch.full((1, 1, args["max_seq_len"], args["max_seq_len"]), -torch.inf, dtype=torch.float32)
            attention.register_buffer("mask", torch.triu(mask, diagonal=1), persistent=False)
    trace = Trace(torch.float32)
    hooks = []

    def capture(mod, name, *, before=False, shape=None):
        def hook(_module, inputs, output=None):
            value = inputs[0] if before else output
            value = value[0]
            if shape is not None:
                value = value.reshape(shape)
            trace.put(name, value)
        hooks.append(mod.register_forward_pre_hook(hook) if before else mod.register_forward_hook(hook))

    capture(model.tok_embeddings, "embedding")
    capture(model.norm, "norm")
    capture(model.output, "logits", shape=(512,))
    for i, block in enumerate(model.layers):
        p = f"layers.{i}."
        capture(block.attention_norm, p + "attention_norm")
        for name, mod, heads in [("query", block.attention.wq, 8), ("key", block.attention.wk, 4), ("value", block.attention.wv, 4)]:
            capture(mod, p + name, shape=(len(ids), heads, 8))
        capture(block.attention.wo, p + "context", before=True)
        capture(block.attention.wo, p + "attention_output")
        capture(block.ffn_norm, p + "attention_residual", before=True)
        capture(block.ffn_norm, p + "ffn_norm")
        capture(block.feed_forward.w1, p + "gate")
        capture(block.feed_forward.w3, p + "up")
        capture(block.feed_forward.w2, p + "gated", before=True)
        capture(block.feed_forward.w2, p + "down")
        capture(block, p + "residual")

    rope, softmax, silu = module.apply_rotary_emb, module.F.softmax, module.F.silu
    calls = {"rope": 0, "softmax": 0, "silu": 0}

    def traced_rope(*args, **kwargs):
        result = rope(*args, **kwargs)
        p = f"layers.{calls['rope']}."
        trace.put(p + "query_rope", result[0][0])
        trace.put(p + "key_rope", result[1][0])
        calls["rope"] += 1
        return result

    def traced_softmax(value, *args, **kwargs):
        result = softmax(value, *args, **kwargs)
        p = f"layers.{calls['softmax']}."
        trace.put(p + "scores", pack_causal(value))
        trace.put(p + "probabilities", pack_causal(result))
        calls["softmax"] += 1
        return result

    def traced_silu(value, *args, **kwargs):
        result = silu(value, *args, **kwargs)
        trace.put(f"layers.{calls['silu']}.silu", result[0])
        calls["silu"] += 1
        return result

    try:
        with torch.inference_mode(), patch.object(module, "apply_rotary_emb", traced_rope), patch.object(module.F, "softmax", traced_softmax), patch.object(module.F, "silu", traced_silu):
            result = model(torch.tensor([ids], dtype=torch.int64, device="cpu"))
        if calls != {"rope": 5, "softmax": 5, "silu": 5} or result.shape != (1, 1, 512):
            raise ReferenceError("original_trace_coverage")
    finally:
        for hook in hooks:
            hook.remove()
    return trace


def explicit_reference(weights, ids, dtype):
    """Sin .float() ni promoción implícita desde activaciones F32 en la ruta F64."""
    w = {name: value.to(dtype=dtype, device="cpu") for name, value in weights.items()}
    trace = Trace(dtype)
    put = trace.put
    one = torch.tensor(1, dtype=dtype, device="cpu")
    epsilon, scale = w["norm.epsilon"], w["attention.scale"]
    t = len(ids)
    cos, sin = w["rope.cos"][:t, None, :], w["rope.sin"][:t, None, :]

    def norm(name, value, weight):
        squares = put(name + ".squares", value * value)
        mean = put(name + ".mean", squares.mean(dim=-1, keepdim=True))
        denominator = put(name + ".denominator", mean + epsilon)
        inverse = put(name + ".inverse", torch.rsqrt(denominator))
        normalized = put(name + ".normalized", value * inverse)
        return put(name, normalized * weight)

    def rotate(name, value):
        pairs = value.reshape(t, value.shape[1], 4, 2)
        real, imag = pairs[..., 0], pairs[..., 1]
        a = put(name + ".real_cos", real * cos)
        b = put(name + ".imag_sin", imag * sin)
        c = put(name + ".real_sin", real * sin)
        d = put(name + ".imag_cos", imag * cos)
        return put(name, torch.stack([a - b, c + d], dim=-1).flatten(-2))

    with torch.inference_mode():
        x = put("embedding", w["tok_embeddings.weight"][ids])
        for layer in range(5):
            p = f"layers.{layer}."
            n = norm(p + "attention_norm", x, w[p + "attention_norm.weight"])
            q = put(p + "query", (n @ w[p + "attention.wq.weight"].T).reshape(t, 8, 8))
            k = put(p + "key", (n @ w[p + "attention.wk.weight"].T).reshape(t, 4, 8))
            v = put(p + "value", (n @ w[p + "attention.wv.weight"].T).reshape(t, 4, 8))
            q, k = rotate(p + "query_rope", q), rotate(p + "key_rope", k)
            k, v = k.repeat_interleave(2, dim=1), v.repeat_interleave(2, dim=1)
            scores, shifts, exponentials, probabilities, contexts = [], [], [], [], []
            for position in range(t):
                score = (q[position, :, None, :] @ k[:position + 1].permute(1, 2, 0)).squeeze(1) * scale
                shift = score - score[:, :1]
                exponent = shift.exp()
                probability = exponent / exponent.sum(dim=-1, keepdim=True)
                context = (probability[:, None, :] @ v[:position + 1].transpose(0, 1)).squeeze(1)
                scores.append(score)
                shifts.append(shift)
                exponentials.append(exponent)
                probabilities.append(probability)
                contexts.append(context)
            for name, values in [("scores", scores), ("score_shifts", shifts), ("score_exp", exponentials), ("probabilities", probabilities)]:
                put(p + name, torch.cat(values, dim=-1))
            context = put(p + "context", torch.stack(contexts).reshape(t, 64))
            attention = put(p + "attention_output", context @ w[p + "attention.wo.weight"].T)
            h = put(p + "attention_residual", x + attention)
            n = norm(p + "ffn_norm", h, w[p + "ffn_norm.weight"])
            gate = put(p + "gate", n @ w[p + "feed_forward.w1.weight"].T)
            up = put(p + "up", n @ w[p + "feed_forward.w3.weight"].T)
            negative_gate = put(p + "negative_gate", -gate)
            gate_exp = put(p + "gate_exp", negative_gate.exp())
            gate_denom = put(p + "gate_denominator", one + gate_exp)
            activation = put(p + "silu", gate / gate_denom)
            gated = put(p + "gated", activation * up)
            down = put(p + "down", gated @ w[p + "feed_forward.w2.weight"].T)
            x = put(p + "residual", h + down)
        n = norm("norm", x, w["norm.weight"])
        put("logits", n[-1] @ w["output.weight"].T)
    return trace


def compare(actual, reference, *, atol=1e-4, rtol=1e-4):
    a, b = actual.to(torch.float64), reference.to(torch.float64)
    if a.shape != b.shape or not torch.isfinite(a).all() or not torch.isfinite(b).all():
        raise ReferenceError("comparison_shape_or_nonfinite")
    error = (a - b).abs()
    bound = atol + rtol * b.abs()
    return {
        "count": a.numel(), "max_abs": error.max().item(),
        "rms": error.square().mean().sqrt().item(),
        "max_tolerance_ratio": (error / bound).max().item(),
        "failures": int((error > bound).sum().item()),
    }


def save_trace(directory, trace):
    directory.mkdir(parents=True, exist_ok=True)
    dtype = "<f4" if trace.dtype == torch.float32 else "<f8"
    metadata, offset = {}, 0
    with (directory / "values.bin").open("wb") as output:
        for name, value in trace.values.items():
            data = value.numpy().astype(dtype, copy=False).tobytes()
            output.write(data)
            metadata[name] = {"shape": list(value.shape), "dtype": dtype, "offset": offset,
                              "bytes": len(data), "sha256": sha256(data)}
            offset += len(data)
    manifest = {"schema_version": 1, "layout": "row-major; scores/probabilities [head, packed(t,j<=t)]",
                "file": "values.bin", "sha256": sha256((directory / "values.bin").read_bytes()),
                "tensors": metadata}
    return write_json(directory / "trace.json", manifest)


def generate(args):
    torch.set_num_threads(1)
    torch.set_num_interop_threads(1)
    torch.use_deterministic_algorithms(True)
    sources = prepare_sources(args.source_dir, offline=args.offline)
    state, model_args = load_checkpoint(args.source_dir)
    inputs = verify_inputs(args.source_dir)
    weights, package_hash = canonical_weights(args.package, state)
    module, source_identity = upstream_module(args.reference_source_dir, offline=args.offline)
    from reference_corpus import Corpus
    corpus = Corpus(weights)
    report = {"schema_version": 1, "source": sources, "reference_source": source_identity,
              "package_sha256": package_hash, "inputs_sha256": sha256((ROOT / "tests/fixtures/tiny-model-inputs.json").read_bytes()),
              "scripts": {name: sha256((ROOT / "tools" / name).read_bytes()) for name in ["reference.py", "reference_corpus.py"]},
              "policy": {"original": "CPU F32 eval, original manual attention branch, observational hooks",
                         "explicit": "CPU F32/F64 throughout; canonical F32 weights/constants promoted exactly; first causal score pivot",
                         "comparison_atol": 1e-4, "comparison_rtol": 1e-4}, "fixtures": {}}
    for fixture in inputs["fixtures"]:
        name, ids = fixture["name"], fixture["token_ids"]
        original = original_reference(module, model_args, state, ids)
        f32 = explicit_reference(weights, ids, torch.float32)
        f64 = explicit_reference(weights, ids, torch.float64)
        comparisons = {key: compare(f32.values[key], ref) for key, ref in original.values.items()}
        differences = {key: compare(f64.values[key], ref) for key, ref in f32.values.items()}
        logits = compare(f64.values["logits"], original.values["logits"])
        if any(value["failures"] for value in comparisons.values()):
            raise ReferenceError(f"explicit_f32_mismatch: {name}: {comparisons}")
        if logits["failures"]:
            raise ReferenceError(f"f64_logits_mismatch: {name}: {logits}")
        trace_hashes = {mode: save_trace(args.output_dir / name / mode, trace)
                        for mode, trace in [("original_f32", original), ("explicit_f32", f32), ("explicit_f64", f64)]}
        corpus.add_fixture(name, f32.values)
        report["fixtures"][name] = {"token_ids": ids, "positions": fixture["positions"], "traces": trace_hashes,
                                    "explicit_f32_vs_original": comparisons, "explicit_f64_vs_f32": differences,
                                    "f64_logits_vs_original": logits,
                                    "logits": {mode: trace.values["logits"].tolist() for mode, trace in [("original_f32", original), ("explicit_f32", f32), ("explicit_f64", f64)]},
                                    "top1": {mode: int(trace.values["logits"].argmax()) for mode, trace in [("original_f32", original), ("explicit_f32", f32), ("explicit_f64", f64)]}}
    report["corpus"] = corpus.save(args.output_dir / "corpus", package_hash, report["fixtures"])
    write_json(args.output_dir / "environment.json", {"python": platform.python_version(), "torch": torch.__version__,
               "numpy": np.__version__, "platform": platform.platform(), "device": "cpu",
               "threads": torch.get_num_threads(), "interop_threads": torch.get_num_interop_threads(),
               "thread_environment": {key: os.environ[key] for key in ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "VECLIB_MAXIMUM_THREADS")}})
    digest = write_json(args.output_dir / "report.json", report)
    return {"status": "ok", "report_sha256": digest, "corpus": report["corpus"],
            "fixtures": {name: {"logits": len(value["logits"]["original_f32"]), "f64_vs_original": value["f64_logits_vs_original"], "top1": value["top1"]} for name, value in report["fixtures"].items()}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=ROOT / "target/tiny-model/sources")
    parser.add_argument("--reference-source-dir", type=Path, default=ROOT / "target/tiny-model/reference-source")
    parser.add_argument("--package", type=Path, default=ROOT / "target/tiny-model/canonical/model.emltiny")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/tiny-model/reference")
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    try:
        result = generate(args)
    except (PreparationError, ReferenceError, OSError) as error:
        print(json.dumps({"status": "error", "stage": "reference", "message": str(error)}), file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
