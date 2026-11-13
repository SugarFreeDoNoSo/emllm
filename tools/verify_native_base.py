"""Prepara la base nativa del checkpoint fijado, supervisa y verifica cada constante."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess

from import_model import MAX_BYTES, MAX_TENSORS, NONE, decode_package
from native_profile import DEFAULT_PROFILE, load_profile
from native_resources import supervise
from prepare_tiny import ROOT, prepare_sources, sha256, write_json

PACKAGE_SHA256 = "434011a9e90db0bd15b39b80f11676c9b238b27ca8c526a9b0b87efc5b02ecd5"
EXPANSION_BITS = {0, 0x3f800000, 0xbf800000, 0x40000000, 0x43000000,
                  0x3c800000, 0x3e000000, 0x45800000, 0x44000000, 0x3f000000}


def checked_package(path):
    data = path.read_bytes()
    if sha256(data) != PACKAGE_SHA256:
        raise ValueError("native package hash mismatch")
    return decode_package(data, max_bytes=MAX_BYTES, max_tensors=MAX_TENSORS)[1]


def verify_dump(output, tensors, events):
    constants = {}
    for line in (output / "constants.tsv").read_text().splitlines():
        bit_text, root_text, original_text, planned_text = line.split()
        bits, root, a, b = int(bit_text, 16), int(root_text), float(original_text), float(planned_text)
        reference, = struct.unpack("<f", struct.pack("<I", bits))
        assert bits not in constants and root >= 0
        assert all(map(math.isfinite, [a, b, reference]))
        assert max(abs(a-reference), abs(b-reference)) <= 1e-9 + 1e-7*abs(reference), (bits, root)
        assert abs(a-b) <= 1e-12 + 1e-12*abs(a), (bits, root)
        constants[bits] = root
    vocabulary_bits = [struct.unpack("<I", struct.pack("<f", id))[0] for id in range(512)]
    expected_bits = {bits for tensor in tensors for bits, in struct.iter_unpack("<I", tensor.data)}
    assert set(constants) == expected_bits | EXPANSION_BITS | set(vocabulary_bits)
    tensor_events = [event for event in events if event["event"] == "tensor"]
    assert len(tensor_events) == len(tensors)
    coordinates = 0
    with (output / "tensors.tsv").open() as rows:
        for i, (tensor, event) in enumerate(zip(tensors, tensor_events, strict=True)):
            assert event == {"event": "tensor", "index": i, "kind": tensor.kind,
                             "layer": None if tensor.layer == NONE else tensor.layer,
                             "shape": list(tensor.shape), "alias": None if tensor.alias == NONE else tensor.alias,
                             "scalars": len(tensor.data)//4}
            for index, (expected,) in enumerate(struct.iter_unpack("<I", tensor.data)):
                t, j, bits, root = next(rows).split()
                assert (int(t), int(j), int(bits, 16), int(root)) == (i, index, expected, constants[expected])
                coordinates += 1
        assert rows.read() == ""
    vocabulary = [tuple(map(int, line.split())) for line in (output / "vocabulary.tsv").read_text().splitlines()]
    assert vocabulary == [(i, constants[bits]) for i, bits in enumerate(vocabulary_bits)]
    prepared, = [e for e in events if e["event"] == "base-prepared"]
    verified, = [e for e in events if e["event"] == "base-verified"]
    assert prepared["constants"] == verified["constants"] == len(constants)
    assert prepared["vocabulary_symbols"] == 512 and prepared["tensors"] == len(tensors)
    assert all(root < prepared["nodes"] for root in constants.values())
    return {"constants": len(constants), "tensor_coordinates_including_aliases": coordinates,
            "vocabulary_symbols": len(vocabulary), "prepared": prepared, "verified": verified}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, default=ROOT / "target/tiny-model/canonical/model.emltiny")
    parser.add_argument("--profile", type=Path, default=DEFAULT_PROFILE)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    profile = load_profile(args.profile)
    tensors = checked_package(args.package)
    sources = prepare_sources(ROOT / "target/tiny-model/sources", offline=True)
    subprocess.run(["cargo", "build", "--release", "--example", "prepare_native_base"], cwd=ROOT, check=True)
    command = [str(ROOT / "target/release/examples/prepare_native_base"), str(args.package.resolve()),
               str((args.output_dir / "dump").resolve()), *[f"{k}={v}" for k, v in profile["limits"].items()]]
    supervision = supervise(command, args.output_dir, profile, cwd=ROOT)
    events = [json.loads(line) for line in (args.output_dir / "stdout.log").read_text().splitlines() if line.startswith("{")]
    report = {"schema_version": 1, "status": supervision["status"], "package_sha256": PACKAGE_SHA256,
              "source": sources, "supervision": supervision, "events": events,
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "sources": {str(p.relative_to(ROOT)): sha256(p.read_bytes()) for p in
                          [*sorted((ROOT / "src").rglob("*.rs")), ROOT / "examples/prepare_native_base.rs", Path(__file__).resolve()]}}
    if supervision["status"] == "ok":
        try:
            report["verification"] = verify_dump(args.output_dir / "dump", tensors, events)
            report["dump_sha256"] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                                     for p in sorted((args.output_dir / "dump").iterdir())}
        except (AssertionError, ValueError, OSError, StopIteration) as error:
            report["status"] = "invalid"
            report["verification_error"] = repr(error)
    write_json(args.output_dir / "verification.json", report)
    print(json.dumps({"status": report["status"], "supervision_failure": supervision["failure"],
                      "verification": report.get("verification"), "evidence": str(args.output_dir)}, indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
