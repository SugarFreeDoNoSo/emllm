"""Ejecuta la puerta escalar sobre el corpus fijado, conservando fallos y ubicación."""

import argparse
import json
from pathlib import Path
import subprocess

from prepare_tiny import ROOT, sha256, write_json
from verify_reference import verify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", type=Path, default=ROOT / "target/tiny-model/reference")
    parser.add_argument("--package", type=Path, default=ROOT / "target/tiny-model/canonical/model.emltiny")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/analysis/scalar-synthesis")
    args = parser.parse_args()
    verified = verify(args.reference_dir, args.package)
    corpus = args.reference_dir / "corpus"
    scalars = json.loads((corpus / "scalars.json").read_text())
    command = ["cargo", "run", "--quiet", "--release", "--example", "verify_scalars", "--", str(corpus / "scalars.u32")]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    (args.output_dir / "stdout.json").write_text(result.stdout)
    (args.output_dir / "stderr.txt").write_text(result.stderr)
    outcome = json.loads(result.stdout)
    assert outcome["total"] == len(scalars["values"])
    if result.returncode:
        assert outcome["status"] == "failed"
        location = scalars["values"][outcome["index"]]
        assert location["bits"] == outcome["bits"]
        outcome["location"] = location
        outcome["bits_hex"] = f"{outcome['bits']:08x}"
        outcome["remaining_not_verified"] = outcome["total"] - outcome["index"] - 1
    else:
        assert outcome["status"] == "ok"
    report = {"schema_version": 1, "outcome": outcome, "exit_code": result.returncode,
              "corpus_sha256": sha256((corpus / "corpus.json").read_bytes()), "scalar_bits_sha256": scalars["sha256"],
              "reference_verification": verified, "command": command,
              "rust": subprocess.check_output(["rustc", "-Vv"], text=True),
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "sources": {str(path.relative_to(ROOT)): sha256(path.read_bytes()) for path in
                          [ROOT / "src/synth/scalar.rs", ROOT / "src/synth.rs", ROOT / "examples/verify_scalars.rs"]}}
    write_json(args.output_dir / "report.json", report)
    print(json.dumps({"outcome": outcome, "evidence": str(args.output_dir)}, indent=2))
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
