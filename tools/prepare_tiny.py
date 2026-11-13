"""Prepara y verifica stories260K; no construye ni evalúa expresiones EML."""

import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
import platform
import struct
import sys
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / "tools/tiny-model.sources.json"
INPUTS = ROOT / "tests/fixtures/tiny-model-inputs.json"
CONFIG = {
    "dim": 64,
    "hidden_dim": 172,
    "n_layers": 5,
    "n_heads": 8,
    "n_kv_heads": 4,
    "vocab_size": 512,
    "max_seq_len": 512,
    "head_dim": 8,
    "norm_eps": 1e-5,
    "rope_theta": 10000.0,
    "tied_embeddings": True,
}


class PreparationError(ValueError):
    """Fallo de datos de entrada antes de generar un paquete o programa."""


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    data = (json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n").encode()
    path.write_bytes(data)
    return sha256(data)


def verify_blob(data, entry):
    if len(data) != entry["size"]:
        raise PreparationError(f"size_mismatch: {entry['name']}")
    if sha256(data) != entry["sha256"]:
        raise PreparationError(f"hash_mismatch: {entry['name']}")


def prepare_sources(source_dir, *, offline):
    manifest = json.loads(SOURCES.read_text())
    source_dir.mkdir(parents=True, exist_ok=True)
    for entry in manifest["files"]:
        path = source_dir / entry["name"]
        if path.exists():
            verify_blob(path.read_bytes(), entry)
        else:
            if offline:
                raise PreparationError(f"missing_source: {path}")
            url = (
                f"https://huggingface.co/{manifest['repository']}/resolve/"
                f"{manifest['revision']}/stories260K/{entry['name']}"
            )
            with urllib.request.urlopen(url, timeout=60) as response:
                data = response.read(entry["size"] + 1)
            verify_blob(data, entry)
            path.write_bytes(data)
    return manifest


def expected_shapes():
    shapes = {
        "tok_embeddings.weight": (512, 64),
        "norm.weight": (64,),
        "output.weight": (512, 64),
    }
    for layer in range(5):
        prefix = f"layers.{layer}."
        shapes.update({
            prefix + "attention.wq.weight": (64, 64),
            prefix + "attention.wk.weight": (32, 64),
            prefix + "attention.wv.weight": (32, 64),
            prefix + "attention.wo.weight": (64, 64),
            prefix + "feed_forward.w1.weight": (172, 64),
            prefix + "feed_forward.w2.weight": (64, 172),
            prefix + "feed_forward.w3.weight": (172, 64),
            prefix + "attention_norm.weight": (64,),
            prefix + "ffn_norm.weight": (64,),
        })
    return shapes


def load_checkpoint(source_dir):
    import torch

    checkpoint = torch.load(
        source_dir / "stories260K.pt", map_location="cpu", weights_only=True
    )
    args = checkpoint["model_args"]
    for key in ("dim", "n_layers", "n_heads", "n_kv_heads", "vocab_size", "max_seq_len"):
        if args.get(key) != CONFIG[key]:
            raise PreparationError(f"config_mismatch: {key}")
    if args.get("multiple_of") != 4:
        raise PreparationError("config_mismatch: multiple_of")
    # El checkpoint original fue guardado desde torch.compile.
    state = {}
    for key, tensor in checkpoint["model"].items():
        name = key.removeprefix("_orig_mod.")
        if name in state:
            raise PreparationError(f"duplicate_tensor: {name}")
        state[name] = tensor
    shapes = expected_shapes()
    if state.keys() != shapes.keys():
        raise PreparationError("tensor_inventory_mismatch")
    for name, shape in shapes.items():
        tensor = state[name]
        if tensor.dtype != torch.float32 or tuple(tensor.shape) != shape:
            raise PreparationError(f"tensor_shape_or_dtype: {name}")
        if not tensor.is_contiguous() or not torch.isfinite(tensor).all().item():
            raise PreparationError(f"tensor_layout_or_finiteness: {name}")
    embedding, output = state["tok_embeddings.weight"], state["output.weight"]
    if (
        embedding.untyped_storage().data_ptr() != output.untyped_storage().data_ptr()
        or embedding.storage_offset() != output.storage_offset()
        or not torch.equal(embedding.view(torch.int32), output.view(torch.int32))
    ):
        raise PreparationError("embedding_alias_mismatch")
    header = struct.unpack("<7i", (source_dir / "stories260K.bin").read_bytes()[:28])
    if header != (64, 172, 5, 8, 4, 512, 512):
        raise PreparationError("binary_header_mismatch")
    return state, args


def verify_inputs(source_dir):
    import sentencepiece

    expected = json.loads(INPUTS.read_text())
    model_bytes = (source_dir / "tok512.model").read_bytes()
    if sha256(model_bytes) != expected["tokenizer_sha256"]:
        raise PreparationError("tokenizer_hash_mismatch")
    tokenizer = sentencepiece.SentencePieceProcessor(model_proto=model_bytes)
    if (
        tokenizer.get_piece_size() != expected["vocab_size"]
        or tokenizer.bos_id() != expected["bos_id"]
        or tokenizer.eos_id() != expected["eos_id"]
    ):
        raise PreparationError("tokenizer_config_mismatch")
    ids = [tokenizer.bos_id()] + tokenizer.encode_as_ids(expected["text"])
    for fixture in expected["fixtures"]:
        count = len(fixture["token_ids"])
        if ids[:count] != fixture["token_ids"] or fixture["positions"] != list(range(count)):
            raise PreparationError(f"token_ids_mismatch: {fixture['name']}")
    return expected


def prepare(source_dir, output_dir, *, offline=False):
    manifest = prepare_sources(source_dir, offline=offline)
    import torch

    torch.set_num_threads(1)
    state, original_args = load_checkpoint(source_dir)
    inputs = verify_inputs(source_dir)
    tensors = {
        name: {
            "shape": list(tensor.shape),
            "dtype": "f32-le",
            "sha256": sha256(tensor.numpy().astype("<f4", copy=False).tobytes()),
        }
        for name, tensor in sorted(state.items())
    }
    info = {
        "schema_version": 1,
        "source": manifest,
        "config": CONFIG,
        "original_model_args": original_args,
        "config_defaults_source": {
            "norm_eps": "model.py RMSNorm/ModelArgs at reference_revision",
            "rope_theta": "model.py precompute_freqs_cis at reference_revision",
        },
        "name_transform": "remove exact _orig_mod. prefix",
        "aliases": {"output.weight": "tok_embeddings.weight"},
        "unique_parameters": sum(t.numel() for n, t in state.items() if n != "output.weight"),
        "tensors": tensors,
    }
    # No publicar metadata parcial si la validación de cualquier entrada falla.
    output_dir.mkdir(parents=True, exist_ok=True)
    hashes = {
        "model-info.json": write_json(output_dir / "model-info.json", info),
        "inputs.json": write_json(output_dir / "inputs.json", inputs),
        "requirements-tiny.lock": sha256((ROOT / "tools/requirements-tiny.lock").read_bytes()),
        "prepare_tiny.py": sha256(Path(__file__).read_bytes()),
    }
    write_json(output_dir / "environment.json", {
        "python": platform.python_version(),
        "platform": platform.platform(),
        "machine": platform.machine(),
        "packages": {dist.metadata["Name"]: dist.version for dist in importlib.metadata.distributions()},
    })
    write_json(output_dir / "hashes.json", hashes)
    return {"status": "ok", "unique_parameters": info["unique_parameters"], "hashes": hashes}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=ROOT / "target/tiny-model/sources")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/tiny-model/prepared")
    parser.add_argument("--offline", action="store_true", help="Exigir fuentes ya descargadas")
    args = parser.parse_args()
    try:
        result = prepare(args.source_dir, args.output_dir, offline=args.offline)
    except (PreparationError, OSError) as error:
        print(json.dumps({"status": "error", "stage": "prepare", "message": str(error)}), file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
