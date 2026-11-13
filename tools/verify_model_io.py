"""Comprueba todos los bits y metadatos del lector Rust contra el exportador."""

import argparse
import json
from pathlib import Path
import struct
import subprocess

from import_model import MAX_BYTES, MAX_TENSORS, NONE, decode_package
from prepare_tiny import ROOT, sha256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, default=ROOT / "target/tiny-model/canonical/model.emltiny")
    args = parser.parse_args()
    data = args.package.read_bytes()
    _, expected = decode_package(data, max_bytes=MAX_BYTES, max_tensors=MAX_TENSORS)
    command = ["cargo", "run", "--quiet", "--release", "--example", "inspect_tiny", "--", str(args.package), "--bits"]
    result = subprocess.run(command, cwd=ROOT, check=True, capture_output=True, text=True)
    actual = json.loads(result.stdout)
    assert actual["bytes"] == len(data)
    assert len(actual["tensors"]) == len(expected)
    for tensor, decoded in zip(expected, actual["tensors"], strict=True):
        assert decoded == {
            "kind": tensor.kind,
            "layer": None if tensor.layer == NONE else tensor.layer,
            "shape": list(tensor.shape),
            "alias": None if tensor.alias == NONE else tensor.alias,
            "scalars": len(tensor.data) // 4,
            "bits": [bits for (bits,) in struct.iter_unpack("<I", tensor.data)],
        }, tensor.name
    print(json.dumps({
        "status": "ok", "sha256": sha256(data), "bytes": len(data),
        "tensors": len(expected), "scalars_including_aliases": sum(len(t.data) // 4 for t in expected),
        "all_bits_and_metadata_identical": True, "command": command,
    }, indent=2))


if __name__ == "__main__":
    main()
