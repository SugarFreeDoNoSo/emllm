"""Expande y verifica operaciones del corpus fijado; pausa ante el primer fallo."""

import argparse
import json
from pathlib import Path
import subprocess

from prepare_tiny import ROOT, sha256, write_json
from verify_reference import verify


def encode(cases):
    lines = [f"EMLOPS1 {len(cases)}"]
    for case in cases:
        args = [f"{arg['bits']:08x}" for arg in case["args"]]
        if len(args) == 1: args.append("-")
        lines.append(" ".join([case["operation"], *args, repr(case["expected_f64"])]))
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", type=Path, default=ROOT / "target/tiny-model/reference")
    parser.add_argument("--package", type=Path, default=ROOT / "target/tiny-model/canonical/model.emltiny")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/analysis/operation-synthesis")
    args = parser.parse_args()
    verified = verify(args.reference_dir, args.package)
    source = (args.reference_dir / "corpus/corpus.json").read_bytes()
    cases = json.loads(source)["operations"]
    args.output_dir.mkdir(parents=True, exist_ok=True)
    path = args.output_dir / "operations.txt"
    path.write_text(encode(cases))
    command = ["cargo", "run", "--quiet", "--release", "--example", "verify_operations", "--", str(path)]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    (args.output_dir / "stdout.json").write_text(result.stdout)
    (args.output_dir / "stderr.txt").write_text(result.stderr)
    outcome = json.loads(result.stdout)
    assert outcome["total"] == len(cases)
    if result.returncode:
        assert outcome["status"] == "failed"
        case = cases[outcome["index"]]
        assert outcome["bits"] == [arg["bits"] for arg in case["args"]]
        outcome["case"] = case
        outcome["remaining_not_verified"] = len(cases) - outcome["index"] - 1
        (args.output_dir / "failure.txt").write_text(encode([case]))
    else:
        assert outcome["status"] == "ok"
    report = {"schema_version": 1, "outcome": outcome, "exit_code": result.returncode,
              "corpus_sha256": sha256(source), "operations_sha256": sha256(path.read_bytes()),
              "reference_verification": verified, "command": command,
              "limits": {"load_bytes": 2097152, "build_nodes": 100000, "compile_nodes": 100000,
                         "plan_nodes": 100000, "plan_slots": 100000, "evaluation_nodes": 100000},
              "rust": subprocess.check_output(["rustc", "-Vv"], text=True),
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "sources": {str(path.relative_to(ROOT)): sha256(path.read_bytes()) for path in
                          [ROOT / "src/synth/model.rs", ROOT / "src/synth/scalar.rs", ROOT / "src/synth.rs", ROOT / "examples/verify_operations.rs"]}}
    write_json(args.output_dir / "report.json", report)
    print(json.dumps({"outcome": outcome, "evidence": str(args.output_dir)}, indent=2))
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
