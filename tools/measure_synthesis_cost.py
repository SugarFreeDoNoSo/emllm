"""Coste compartido del checkpoint y estimación explícita de todas las familias."""

import argparse
import json
from pathlib import Path
import re
import struct
import subprocess
import sys

from prepare_tiny import ROOT, sha256, write_json
from import_model import NONE, decode_package
from verify_reference import verify


def estimate(marginal, weights, inventory, components, caps):
    mul, add = marginal["mul_max"], marginal["add_max"]
    left = marginal["left_warm_max"] - mul
    right = marginal["right_warm_max"] - mul
    fixtures = {}
    for tokens in [8, 16]:
        triangular = tokens * (tokens + 1) // 2
        dense_macs = 5 * tokens * (64*64 + 2*64*32 + 64*64 + 3*64*172) + 512*64
        macs = dense_macs + 5*2*8*8*triangular
        additions = dense_macs - (5*tokens*600 + 512) + 5*8*7*triangular + 5*8*8*(triangular-tokens)
        core = macs*mul + additions*add
        nonlinear = {}
        for operation, rows in [("rmsnorm", 10*tokens+1), ("softmax", 40*tokens), ("swiglu", 5*tokens)]:
            samples = [e["nodes"] for e in components if e["event"] == "prepared" and e["operation"] == operation
                       and (operation != "softmax" or e["length"] <= tokens)]
            nonlinear[operation] = {"rows": rows, "closed_case_max_nodes": max(samples), "estimated_nodes": rows*max(samples)}
        overhead = {
            "all_weight_operand_preparations": inventory["distinct"]*right,
            "dense_input_preparations": (5*tokens*(3*64+172)+64)*left,
            "attention_operand_preparations": 5*(tokens*64*left + tokens*64*right + 8*triangular*left),
            "rope": 5*tokens*(192*mul + 96*add + 96*left),
            "score_scaling": 5*8*triangular*(mul+left),
            "residual_additions": 5*2*tokens*64*add,
            "auxiliary_scalar_and_operand_budget": (2*tokens*4+2)*(829+right),
            "kernel_priming": marginal["priming_nodes"],
        }
        without_weights = core + sum(overhead.values()) + sum(e["estimated_nodes"] for e in nonlinear.values())
        core_with_weights = core + weights["nodes"] if weights else None
        total = without_weights + weights["nodes"] if weights else None
        fixtures[f"prefix{tokens}"] = {
            "macs": macs, "balanced_additions": additions, "core_operation_nodes": core,
            "core_plus_shared_weights_nodes": core_with_weights,
            "nonlinear": nonlinear, "overhead": overhead,
            "estimated_nodes_excluding_weight_synthesis": without_weights,
            "inventory_estimated_nodes": total,
            "pair_and_instruction_payload_excluding_weight_synthesis_bytes": 40*(without_weights-1),
            "pair_and_instruction_payload_bytes": 40*(total-1) if weights else None,
            "within_node_cap": total <= caps["nodes"] if weights else False if without_weights > caps["nodes"] else None,
            "payload_within_process_cap": 40*(total-1) <= caps["process_bytes"] if weights else False if 40*(without_weights-1) > caps["process_bytes"] else None,
        }
    return {"fixtures": fixtures, "left_preparation_nodes": left, "right_preparation_nodes": right,
            "caps": caps, "within_estimated_caps": all(e["within_node_cap"] and e["payload_within_process_cap"] for e in fixtures.values()),
            "scope": "Conditional structural estimate, not a bound or full-model RSS. Distinct binary occurrences use warmed measured costs; exact repeated pairs would share. Weights synthesized once; full synthesis count unavailable on budget failure, never replaced with zero. Nonlinear rows use closed-case maxima (overcount imported activation synthesis and shared weights). Includes RoPE, residuals, score scaling and auxiliary constants; excludes allocator/capacity/compile-plan temporaries, whose memory is additional. No full-model slot estimate."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", type=Path, default=ROOT / "target/tiny-model/reference")
    parser.add_argument("--package", type=Path, default=ROOT / "target/tiny-model/canonical/model.emltiny")
    parser.add_argument("--components-dir", type=Path, default=ROOT / "target/analysis/component-synthesis-log-domain")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/analysis/synthesis-shared-cost")
    args = parser.parse_args()
    verified = verify(args.reference_dir, args.package)
    _, tensors = decode_package(args.package.read_bytes(), max_bytes=2097152, max_tensors=64)
    owned_weights = [t for t in tensors if t.kind <= 12 and t.alias == NONE]
    weight_bits = [bits for t in owned_weights for bits, in struct.iter_unpack("<I", t.data)]
    distinct_bits = list(dict.fromkeys(weight_bits))
    inventory = {"tensors": len(owned_weights), "scalars": len(weight_bits), "distinct": len(distinct_bits)}
    component_bytes = (args.components_dir / "report.json").read_bytes()
    component_report = json.loads(component_bytes)
    assert component_report["outcome"]["status"] == "ok"
    assert component_report["reference_verification"] == verified
    for relative in ["src/synth/model.rs", "src/synth/scalar.rs", "src/synth.rs"]:
        assert component_report["sources"][relative] == sha256((ROOT / relative).read_bytes())
    events = [json.loads(line) for line in (args.components_dir / "stdout.jsonl").read_text().splitlines()]
    assert len([e for e in events if e["event"] == "verified"]) == component_report["outcome"]["total"]
    args.output_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run(["cargo", "build", "--release", "--example", "synthesis_cost"], cwd=ROOT, check=True)
    command = [str(ROOT / "target/release/examples/synthesis_cost"), str(args.package.resolve())]
    if sys.platform == "darwin": command = ["/usr/bin/time", "-l", *command]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    (args.output_dir / "stdout.jsonl").write_text(result.stdout)
    (args.output_dir / "stderr.txt").write_text(result.stderr)
    measurements = [json.loads(line) for line in result.stdout.splitlines()]
    marginal = next(e for e in measurements if e["event"] == "marginal")
    assert marginal["sample_bits"] == distinct_bits[:128]
    weights = next((e for e in measurements if e["event"] == "weights"), None)
    failed_weights = next((e for e in measurements if e["event"] == "weights_failed"), None)
    if weights: assert {key: weights[key] for key in inventory} == inventory
    growth = estimate(marginal, weights, inventory, events, component_report["growth"]["full_program_caps"])
    status = "blocked_weight_construction" if failed_weights else "failed_measurement" if result.returncode else "within_estimated_caps" if growth["within_estimated_caps"] else "blocked_resource_estimate"
    match = re.search(r"(\d+)\s+maximum resident set size", result.stderr) if sys.platform == "darwin" else None
    report = {"schema_version": 1, "status": status, "measurement_exit_code": result.returncode,
              "measurements": measurements, "growth": growth, "reference_verification": verified,
              "weight_inventory": inventory,
              "component_report_sha256": sha256(component_bytes), "package_sha256": sha256(args.package.read_bytes()),
              "sample_selection": "First 128 distinct weight bit patterns in canonical directory/payload order; first 64 left, next 64 right. Warm first row/column; measure other 63x63 pairs, then their balanced reduction. No numeric evaluation or value-based selection.",
              "limits": {"load_bytes": 2097152, "weight_nodes": 10_000_000, "weight_plan_slots": 1_000_000, "marginal_nodes": 1_000_000},
              "rss": {"status": "measured" if match else "unavailable", "bytes": int(match[1]) if match else None,
                      "method": "/usr/bin/time -l on macOS" if sys.platform == "darwin" else "unavailable",
                      "scope": "Rust load, marginal calibration and shared-weight build attempt; compile/plan only when construction succeeds. No evaluation, Python or Rust compilation."},
              "command": command, "rust": subprocess.check_output(["rustc", "-Vv"], text=True),
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "sources": {str(p.relative_to(ROOT)): sha256(p.read_bytes()) for p in
                          [ROOT / "examples/synthesis_cost.rs", ROOT / "src/synth/model.rs", ROOT / "src/synth/scalar.rs", ROOT / "src/synth.rs", Path(__file__).resolve()]}}
    write_json(args.output_dir / "report.json", report)
    print(json.dumps({"status": status, "weight_inventory": inventory, "measurements": [{k:v for k,v in e.items() if k != "sample_bits"} for e in measurements],
                      "growth": growth, "rss": report["rss"], "evidence": str(args.output_dir)}, indent=2))
    return result.returncode or (0 if growth["within_estimated_caps"] else 2)


if __name__ == "__main__":
    raise SystemExit(main())
