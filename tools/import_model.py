"""Exporta stories260K a EMLTINY v1, preservando los bits F32 y alias."""

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import struct
import sys

from prepare_tiny import CONFIG, ROOT, PreparationError, load_checkpoint, prepare, sha256, write_json


MAGIC = b"EMLTINY\0"
VERSION = 1
NONE = 0xFFFFFFFF
MAX_BYTES = 16 * 1024 * 1024
MAX_TENSORS = 128
HEADER = struct.Struct("<8s10IQQ")
RECORD = struct.Struct("<4I4Q")
CONFIG_FIELDS = (
    "dim", "hidden_dim", "n_layers", "n_heads", "n_kv_heads",
    "vocab_size", "max_seq_len", "rope_positions",
)
LAYER_KINDS = {
    2: "attention_norm.weight", 3: "attention.wq.weight",
    4: "attention.wk.weight", 5: "attention.wv.weight",
    6: "attention.wo.weight", 7: "ffn_norm.weight",
    8: "feed_forward.w1.weight", 9: "feed_forward.w2.weight",
    10: "feed_forward.w3.weight",
}
GLOBAL_KINDS = {
    1: "tok_embeddings.weight", 11: "norm.weight", 12: "output.weight",
    13: "rope.cos", 14: "rope.sin", 15: "norm.epsilon", 16: "attention.scale",
}


class FormatError(ValueError):
    """Paquete inválido; no contiene un modelo acreditado."""


@dataclass(frozen=True)
class Tensor:
    kind: int
    layer: int
    shape: tuple
    data: bytes
    alias: int = NONE

    @property
    def name(self):
        if self.kind in LAYER_KINDS:
            return f"layers.{self.layer}.{LAYER_KINDS[self.kind]}"
        return GLOBAL_KINDS[self.kind]


def shape_bytes(shape):
    if len(shape) > 2 or any(d <= 0 or d > 0xFFFFFFFFFFFFFFFF for d in shape):
        raise FormatError("invalid_shape")
    size = 4
    for dimension in shape:
        size *= dimension
        if size > 0xFFFFFFFFFFFFFFFF:
            raise FormatError("size_overflow")
    return size


def decode_package(data, *, max_bytes, max_tensors):
    """Valida el contenedor; la política del modelo valida además su inventario."""
    if len(data) > max_bytes:
        raise FormatError("byte_limit")
    if len(data) < HEADER.size:
        raise FormatError("truncated_header")
    magic, version, count, *tail = HEADER.unpack_from(data)
    if magic != MAGIC or version != VERSION:
        raise FormatError("magic_or_version")
    if count == 0 or count > max_tensors:
        raise FormatError("tensor_limit")
    config = dict(zip(CONFIG_FIELDS, tail[:8]))
    directory, payload = tail[8:]
    if any(v == 0 for v in config.values()):
        raise FormatError("zero_dimension")
    if directory != HEADER.size or payload != HEADER.size + count * RECORD.size:
        raise FormatError("directory_layout")
    if payload > len(data):
        raise FormatError("truncated_directory")
    cursor = payload
    tensors, offsets, keys = [], [], set()
    for index in range(count):
        kind, layer, rank, alias, d0, d1, offset, size = RECORD.unpack_from(
            data, directory + index * RECORD.size
        )
        if kind in LAYER_KINDS:
            if layer >= config["n_layers"]:
                raise FormatError("layer_range")
        elif kind not in GLOBAL_KINDS or layer != NONE:
            raise FormatError("tensor_kind_or_layer")
        if (kind, layer) in keys:
            raise FormatError("duplicate_tensor")
        keys.add((kind, layer))
        if rank > 2 or (rank == 0 and d0 != 1) or (rank < 2 and d1 != 1):
            raise FormatError("rank_or_unused_dimension")
        shape = (d0, d1)[:rank]
        if shape_bytes(shape) != size:
            raise FormatError("tensor_size")
        if alias != NONE:
            if alias >= index or tensors[alias].alias != NONE:
                raise FormatError("alias_target")
            target = tensors[alias]
            if target.shape != shape or offsets[alias] != offset or len(target.data) != size:
                raise FormatError("alias_layout")
            body = target.data
        else:
            if offset != cursor or size > len(data) - cursor:
                raise FormatError("payload_range")
            body = data[cursor:cursor + size]
            cursor += size
            if any(bits & 0x7F800000 == 0x7F800000 for (bits,) in struct.iter_unpack("<I", body)):
                raise FormatError("nonfinite_scalar")
        tensors.append(Tensor(kind, layer, shape, body, alias))
        offsets.append(offset)
    if cursor != len(data):
        raise FormatError("trailing_bytes")
    return config, tensors


def encode_package(config, tensors):
    payload_offset = HEADER.size + len(tensors) * RECORD.size
    directory, payload, offsets = bytearray(), bytearray(), []
    for index, tensor in enumerate(tensors):
        if tensor.alias != NONE:
            if tensor.alias >= index:
                raise FormatError("alias_target")
            target = tensors[tensor.alias]
            if target.shape != tensor.shape or target.data != tensor.data:
                raise FormatError("alias_content")
            offset = offsets[tensor.alias]
        else:
            offset = payload_offset + len(payload)
            payload.extend(tensor.data)
        if len(tensor.data) != shape_bytes(tensor.shape):
            raise FormatError("tensor_size")
        dimensions = tensor.shape + (1,) * (2 - len(tensor.shape))
        directory.extend(RECORD.pack(
            tensor.kind, tensor.layer, len(tensor.shape), tensor.alias,
            *dimensions, offset, len(tensor.data),
        ))
        offsets.append(offset)
    header = HEADER.pack(
        MAGIC, VERSION, len(tensors), *(config[key] for key in CONFIG_FIELDS),
        HEADER.size, payload_offset,
    )
    result = header + directory + payload
    decode_package(result, max_bytes=MAX_BYTES, max_tensors=MAX_TENSORS)
    return bytes(result)


def tensor_bytes(tensor):
    return tensor.detach().cpu().contiguous().numpy().astype("<f4", copy=False).tobytes()


def verify_llama2c_weights(source_dir, state):
    names = ["tok_embeddings.weight"]
    for suffix in LAYER_KINDS.values():
        names.extend(f"layers.{layer}.{suffix}" for layer in range(5))
    names.append("norm.weight")
    original = (source_dir / "stories260K.bin").read_bytes()
    cursor = 28
    for name in names:
        expected = tensor_bytes(state[name])
        if original[cursor:cursor + len(expected)] != expected:
            raise PreparationError(f"binary_weight_mismatch: {name}")
        cursor += len(expected)
    # El binario original incluye dos tablas RoPE antiguas para todo el contexto.
    if len(original) - cursor != CONFIG["max_seq_len"] * CONFIG["head_dim"] * 4:
        raise PreparationError("binary_legacy_rope_size")


def model_tensors(state):
    import torch

    tensors = [Tensor(1, NONE, tuple(state["tok_embeddings.weight"].shape), tensor_bytes(state["tok_embeddings.weight"]))]
    for layer in range(5):
        for kind, suffix in LAYER_KINDS.items():
            tensor = state[f"layers.{layer}.{suffix}"]
            tensors.append(Tensor(kind, layer, tuple(tensor.shape), tensor_bytes(tensor)))
    tensors.append(Tensor(11, NONE, (64,), tensor_bytes(state["norm.weight"])))
    tensors.append(Tensor(12, NONE, (512, 64), tensor_bytes(state["output.weight"]), 0))
    # Misma fórmula F32 del model.py fijado, calculada fuera del runtime EML.
    indices = torch.arange(0, 8, 2, dtype=torch.float32)
    frequencies = 1.0 / (10000.0 ** (indices / 8))
    angles = torch.outer(torch.arange(16, dtype=torch.float32), frequencies)
    for kind, value in [(13, angles.cos()), (14, angles.sin())]:
        tensors.append(Tensor(kind, NONE, (16, 4), tensor_bytes(value)))
    tensors.append(Tensor(15, NONE, (), tensor_bytes(torch.tensor(1e-5, dtype=torch.float32))))
    tensors.append(Tensor(16, NONE, (), tensor_bytes(torch.tensor(8.0, dtype=torch.float32).rsqrt())))
    return tensors


def export(source_dir, output_dir, *, offline):
    prepare(source_dir, output_dir / "prepared", offline=offline)
    state, _ = load_checkpoint(source_dir)
    verify_llama2c_weights(source_dir, state)
    tensors = model_tensors(state)
    config = {**CONFIG, "rope_positions": 16}
    data = encode_package(config, tensors)
    _, decoded = decode_package(data, max_bytes=MAX_BYTES, max_tensors=MAX_TENSORS)
    for original, actual in zip(tensors, decoded, strict=True):
        if original != actual:
            raise FormatError(f"round_trip_mismatch: {original.name}")
    # La identidad se conserva por bytes, incluso para cero con signo.
    for tensor in decoded:
        if tensor.kind <= 12 and tensor.data != tensor_bytes(state[tensor.name]):
            raise FormatError(f"weight_round_trip_mismatch: {tensor.name}")
    (output_dir / "model.emltiny").write_bytes(data)
    metadata = {
        "schema_version": 1,
        "file": "model.emltiny",
        "sha256": sha256(data),
        "size": len(data),
        "source_info_sha256": sha256((output_dir / "prepared/model-info.json").read_bytes()),
        "input_sha256": sha256((output_dir / "prepared/inputs.json").read_bytes()),
        "exporter_sha256": sha256(Path(__file__).read_bytes()),
        "config": config,
        "constant_policy": {
            "rope": "Original model.py formula; CPU F32 frequencies, angles, cos and sin; 16 positions",
            "epsilon": "Nearest F32 to 1e-5, from reference norm_eps default",
            "attention_scale": "CPU F32 rsqrt(8); EML and F64 reference multiply this constant",
        },
        "tensors": [{
            "name": tensor.name, "kind": tensor.kind, "layer": tensor.layer,
            "shape": tensor.shape, "alias": None if tensor.alias == NONE else tensor.alias,
            "sha256": sha256(tensor.data),
        } for tensor in decoded],
        "all_weights_match_pt_and_bin": True,
    }
    write_json(output_dir / "manifest.json", metadata)
    return {"status": "ok", "size": len(data), "sha256": sha256(data), "tensors": len(tensors)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=ROOT / "target/tiny-model/sources")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/tiny-model/canonical")
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    try:
        result = export(args.source_dir, args.output_dir, offline=args.offline)
    except (PreparationError, FormatError, OSError) as error:
        print(json.dumps({"status": "error", "stage": "import", "message": str(error)}), file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
