"""Verifica todos los casos del corpus correspondientes a componentes nativos."""
import argparse
from collections import Counter
import json
from pathlib import Path
import subprocess
from native_profile import load_profile
from native_resources import supervise
from prepare_tiny import ROOT, sha256, write_json
from verify_component_corpus import component_cases, encode
from verify_reference import verify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", type=Path, default=ROOT / "target/tiny-model/reference")
    parser.add_argument("--package", type=Path, default=ROOT / "target/tiny-model/canonical/model.emltiny")
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    verified = verify(args.reference_dir, args.package)
    corpus = (args.reference_dir / "corpus/corpus.json").read_bytes()
    cases = [c for c in component_cases(json.loads(corpus), args.reference_dir)
             if c["operation"] in ("dot", "rmsnorm", "swiglu")]
    assert Counter(c["operation"] for c in cases) == {"dot": 631, "rmsnorm": 48, "swiglu": 62}
    args.output_dir.mkdir(parents=True, exist_ok=False)
    write_json(args.output_dir / "cases.json", cases)
    path = args.output_dir / "components.txt"
    path.write_text(encode(cases))
    profile = load_profile()
    profile["name"] = "native-components-v1"
    profile["limits"].update(base_nodes=1_000_000, closed_nodes=1_000_000,
                              reachable_nodes=1_000_000, numeric_slots=100_000)
    write_json(args.output_dir / "profile.json", profile)
    subprocess.run(["cargo", "build", "--release", "--example", "verify_native_components"], cwd=ROOT, check=True)
    command = [str(ROOT / "target/release/examples/verify_native_components"), str(path.resolve())]
    supervision = supervise(command, args.output_dir / "process", profile, cwd=ROOT)
    events = [json.loads(line) for line in (args.output_dir / "process/stdout.log").read_text().splitlines() if line.startswith("{")]
    passed = [event for event in events if event["event"] == "verified"]
    report = {"status": supervision["status"], "supervision": supervision,
              "required_cases": dict(Counter(c["operation"] for c in cases)),
              "verified": len(passed), "events": events, "reference_verification": verified,
              "corpus_sha256": sha256(corpus), "protocol_sha256": sha256(path.read_bytes()),
              "sources": {str(p.relative_to(ROOT)): sha256(p.read_bytes()) for p in
                          [*sorted((ROOT / "src").rglob("*.rs")), ROOT / "examples/verify_native_components.rs",
                           ROOT / "examples/support/components.rs", Path(__file__).resolve()]}}
    if len(passed) != len(cases) or not events or events[-1] != {"event": "complete", "total": len(cases)}:
        report["status"] = "failed"
    if report["status"] == "ok":
        assert [e["index"] for e in passed] == list(range(len(cases)))
        assert all(e["operation"] == c["operation"] for e, c in zip(passed, cases, strict=True))
        report["max_threshold_fraction"] = max(e["max_threshold_fraction"] for e in passed)
        report["max_differential_fraction"] = max(e["max_differential_fraction"] for e in passed)
    else:
        failed = [e for e in events if e["event"] == "failed"]
        report["failure"] = failed[-1] if failed else supervision["failure"]
        if failed:
            write_json(args.output_dir / "failure-case.json", cases[failed[-1]["index"]])
    write_json(args.output_dir / "report.json", report)
    print(json.dumps({k: v for k, v in report.items() if k in ("status", "required_cases", "verified", "failure", "max_threshold_fraction", "max_differential_fraction")}, indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
