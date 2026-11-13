"""Verifica hashes, dtypes, entradas, logits y procedencia de un corpus preparado."""

import argparse
import json
from pathlib import Path
import struct

import numpy as np

from import_model import MAX_BYTES, MAX_TENSORS, decode_package
from prepare_tiny import INPUTS, ROOT, sha256


def verify(directory, package):
    report = json.loads((directory / "report.json").read_text())
    inputs = json.loads(INPUTS.read_text())
    assert report["inputs_sha256"] == sha256(INPUTS.read_bytes())
    data = package.read_bytes()
    assert report["package_sha256"] == sha256(data)
    _, tensors = decode_package(data, max_bytes=MAX_BYTES, max_tensors=MAX_TENSORS)
    sources = {"package/" + t.name: np.frombuffer(t.data, dtype="<u4") for t in tensors}
    stages = 0
    for fixture in inputs["fixtures"]:
        name = fixture["name"]
        result = report["fixtures"][name]
        assert result["token_ids"] == fixture["token_ids"]
        assert result["positions"] == fixture["positions"]
        for mode, expected_dtype in [("original_f32", "<f4"), ("explicit_f32", "<f4"), ("explicit_f64", "<f8")]:
            root = directory / name / mode
            manifest_bytes = (root / "trace.json").read_bytes()
            assert sha256(manifest_bytes) == result["traces"][mode]
            trace = json.loads(manifest_bytes)
            payload = (root / "values.bin").read_bytes()
            assert sha256(payload) == trace["sha256"]
            cursor = 0
            for tensor, entry in trace["tensors"].items():
                start, size = entry["offset"], entry["bytes"]
                body = payload[start:start + size]
                assert len(body) == size and sha256(body) == entry["sha256"]
                assert entry["dtype"] == expected_dtype
                values = np.frombuffer(body, dtype=expected_dtype).reshape(entry["shape"])
                assert np.isfinite(values).all()
                if tensor == "logits":
                    assert values.shape == (512,)
                    assert values.tolist() == result["logits"][mode]
                    assert int(values.argmax()) == result["top1"][mode]
                if mode == "explicit_f32":
                    sources[f"{name}/{mode}/{tensor}"] = np.frombuffer(body, dtype="<u4")
                cursor += size
                stages += 1
            assert cursor == len(payload)
        for comparison in [*result["explicit_f32_vs_original"].values(), *result["explicit_f64_vs_f32"].values(), result["f64_logits_vs_original"]]:
            assert comparison["failures"] == 0
    corpus = json.loads((directory / "corpus/corpus.json").read_text())
    assert sha256((directory / "corpus/corpus.json").read_bytes()) == report["corpus"]["sha256"]
    scalar_bytes = (directory / "corpus/scalars.json").read_bytes()
    assert sha256(scalar_bytes) == corpus["scalar_manifest_sha256"]
    scalars = json.loads(scalar_bytes)
    binary = (directory / "corpus/scalars.u32").read_bytes()
    assert sha256(binary) == scalars["sha256"] == report["corpus"]["scalars_sha256"]
    bits = [value for (value,) in struct.iter_unpack("<I", binary)]
    assert bits == sorted(set(bits)) == [v["bits"] for v in scalars["values"]]
    assert len(bits) == report["corpus"]["distinct_scalars"]
    assert {int(v) for t in tensors for v in np.frombuffer(t.data, dtype="<u4")} <= set(bits)
    references = 0

    def visit(value):
        nonlocal references
        if isinstance(value, dict):
            if "bits" in value:
                assert int(sources[value["source"]][value["index"]]) == value["bits"], value
                references += 1
            else:
                for item in value.values(): visit(item)
        elif isinstance(value, list):
            for item in value: visit(item)

    visit(scalars["values"])
    visit(corpus)
    return {"status": "ok", "stages": stages, "provenance_references": references,
            "distinct_scalars": len(bits), "report_sha256": sha256((directory / "report.json").read_bytes())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", type=Path, default=ROOT / "target/tiny-model/reference")
    parser.add_argument("--package", type=Path, default=ROOT / "target/tiny-model/canonical/model.emltiny")
    parser.add_argument("--compare-dir", type=Path)
    args = parser.parse_args()
    result = verify(args.reference_dir, args.package)
    if args.compare_dir:
        assert verify(args.compare_dir, args.package) == result
        expected = {p.relative_to(args.reference_dir) for p in args.reference_dir.rglob("*") if p.is_file() and p.name != "environment.json"}
        actual = {p.relative_to(args.compare_dir) for p in args.compare_dir.rglob("*") if p.is_file() and p.name != "environment.json"}
        assert expected == actual
        for path in sorted(expected):
            assert (args.reference_dir / path).read_bytes() == (args.compare_dir / path).read_bytes(), path
        result["identical_files_excluding_environment"] = len(expected)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
