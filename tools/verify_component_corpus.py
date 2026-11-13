"""Puerta de reducciones/componentes cerrados; conserva el primer fallo y su coste."""

import argparse
from collections import Counter
import json
import math
from pathlib import Path
import re
import struct
import subprocess
import sys

from prepare_tiny import ROOT, sha256, write_json
from reference_corpus import number
from verify_reference import verify


def component_cases(corpus, directory):
    cases = []
    for item in corpus["reductions"]:
        cases.append({**item, "aux": "-", "expected_f64": [item["expected_f64"]]})
    for item in corpus["components"]:
        assert item["operation"] in ("rmsnorm", "softmax")
        if item["operation"] == "softmax": assert item["pivot"] == 0
        cases.append({**item, "left": item["input"], "right": item.get("weight", []),
                      "aux": f"{item['epsilon']['bits']:08x}" if item["operation"] == "rmsnorm" else "0"})

    # Filas completas de gate/up ya fijadas, seleccionadas por las ubicaciones
    # de los productos gated del corpus original; no se consulta ningún resultado EML.
    rows = {}
    for item in corpus["operations"]:
        if item["operation"] == "mul" and item["family"].endswith(".gated"):
            ref = item["args"][0]
            rows[(ref["source"].removesuffix(".silu"), ref["index"] // 172)] = None
    sources = {}

    def refs(source, row):
        if source not in sources:
            fixture, mode, name = source.split("/", 2)
            root = directory / fixture / mode
            trace = json.loads((root / "trace.json").read_text())
            entry = trace["tensors"][name]
            payload = (root / "values.bin").read_bytes()
            body = payload[entry["offset"]:entry["offset"] + entry["bytes"]]
            assert entry["dtype"] == "<f4" and entry["shape"][-1] == 172
            assert sha256(body) == entry["sha256"]
            sources[source] = [bits for bits, in struct.iter_unpack("<I", body)]
        return [{"source": source, "index": i, "bits": sources[source][i]}
                for i in range(row * 172, (row + 1) * 172)]

    for source, row in rows:
        left, right = refs(source + ".gate", row), refs(source + ".up", row)
        expected = [number(gate) / (1.0 + math.exp(-number(gate))) * number(up)
                    for gate, up in zip(left, right, strict=True)]
        assert all(map(math.isfinite, expected))
        cases.append({"family": f"{source}.swiglu[{row}]", "operation": "swiglu",
                      "left": left, "right": right, "aux": "-", "expected_f64": expected})
    return cases


def encode(cases):
    lines = [f"EMLCOMP1 {len(cases)}"]
    for case in cases:
        lines.extend([f"{case['operation']} {len(case['left'])} {case['aux']}",
                      " ".join(f"{ref['bits']:08x}" for ref in case["left"]),
                      " ".join(f"{ref['bits']:08x}" for ref in case["right"]) or "-",
                      " ".join(map(repr, case["expected_f64"]))])
    return "\n".join(lines) + "\n"


def estimate(prepared):
    ratios = [record["nodes"] / record["length"] for record in prepared if record["operation"] == "dot"]
    ratio = max(ratios) if ratios else None
    caps = {"nodes": 10_000_000, "slots": 1_000_000, "process_bytes": 2 * 1024**3}
    fixtures = {}
    for tokens in [8, 16]:
        # Cinco bloques: Q/K/V/O y W1/W3/W2; atención Q.K y suma ponderada;
        # una proyección final de 512 logits. No incluye operaciones no lineales.
        macs = tokens * 5 * (64*64 + 2*64*32 + 64*64 + 3*64*172)
        macs += 5 * 2 * 8 * 8 * tokens*(tokens+1)//2 + 512*64
        nodes = math.ceil(macs * ratio) if ratio is not None else None
        fixtures[f"prefix{tokens}"] = {
            "dense_and_attention_macs": macs, "extrapolated_nodes": nodes,
            "extrapolated_pair_and_instruction_payload_bytes": 40*(nodes-1) if nodes else None,
            "within_node_cap": nodes <= caps["nodes"] if nodes else None,
            "within_process_cap_for_these_payloads": 40*(nodes-1) <= caps["process_bytes"] if nodes else None,
        }
    return {"status": "estimate_only", "full_program_caps": caps, "fixtures": fixtures,
            "max_observed_nodes_per_dot_term": ratio,
            "assumptions": "Extrapolate maximum observed closed-dot nodes/term to all MACs. Includes repeated scalar/constant cost; sharing may reduce it. Excludes nonlinear operations, allocator, arena/interning and compile/plan temporaries. Neither a lower/upper bound nor measured full-model RSS.",
            "within_estimated_caps": all(item["within_node_cap"] and item["within_process_cap_for_these_payloads"] for item in fixtures.values())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", type=Path, default=ROOT / "target/tiny-model/reference")
    parser.add_argument("--package", type=Path, default=ROOT / "target/tiny-model/canonical/model.emltiny")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/analysis/component-synthesis")
    args = parser.parse_args()
    verified = verify(args.reference_dir, args.package)
    corpus_bytes = (args.reference_dir / "corpus/corpus.json").read_bytes()
    cases = component_cases(json.loads(corpus_bytes), args.reference_dir)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    cases_hash = write_json(args.output_dir / "cases.json", cases)
    path = args.output_dir / "components.txt"
    path.write_text(encode(cases))
    subprocess.run(["cargo", "build", "--release", "--example", "verify_components"], cwd=ROOT, check=True)
    command = [str(ROOT / "target/release/examples/verify_components"), str(path.resolve())]
    timed = sys.platform == "darwin"
    if timed: command = ["/usr/bin/time", "-l", *command]
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    (args.output_dir / "stdout.jsonl").write_text(result.stdout)
    (args.output_dir / "stderr.txt").write_text(result.stderr)
    events = [json.loads(line) for line in result.stdout.splitlines()]
    prepared = [event for event in events if event["event"] == "prepared"]
    passed = [event for event in events if event["event"] == "verified"]
    final = events[-1]
    assert final["total"] == len(cases)
    outcome = {"status": "failed" if result.returncode else "ok", "total": len(cases),
               "verified": len(passed), "coverage": dict(Counter(cases[e["index"]]["operation"] for e in passed)),
               "max_threshold_fraction": max((e["max_threshold_fraction"] for e in passed), default=None),
               "max_differential_fraction": max((e["max_differential_fraction"] for e in passed), default=None)}
    if result.returncode:
        assert final["event"] == "failed" and final["index"] == len(passed)
        case = cases[final["index"]]
        outcome.update({"failure": {**final, "case": case}, "remaining_not_verified": len(cases)-len(passed)-1})
        (args.output_dir / "failure.txt").write_text(encode([case]))
    else:
        assert final["event"] == "complete" and len(passed) == len(cases)
    maxima = {key: max((event[key] for event in prepared), default=None)
              for key in ["nodes", "edges", "roots", "depth", "slots", "program_pair_capacity_bytes",
                          "plan_instruction_capacity_bytes", "required_original_value_bytes", "required_planned_value_bytes"]}
    memory = []
    successes = {event["index"]: event for event in passed}
    for event in prepared:
        original = sum(event[key] for key in ["program_pair_capacity_bytes", "program_root_capacity_bytes", "program_metadata_bytes"])
        planned = original + sum(event[key] for key in ["plan_instruction_capacity_bytes", "plan_root_capacity_bytes", "plan_metadata_bytes"])
        success = successes.get(event["index"])
        memory.append({"index": event["index"], "original_resident_capacity_bytes": original,
                       "planned_resident_capacity_bytes": planned,
                       "original_with_buffers_capacity_bytes": original + sum(success[key] for key in ["original_value_capacity_bytes", "original_output_capacity_bytes", "original_buffer_metadata_bytes"]) if success else None,
                       "planned_with_buffers_capacity_bytes": planned + sum(success[key] for key in ["planned_value_capacity_bytes", "planned_output_capacity_bytes", "planned_buffer_metadata_bytes"]) if success else None})
    match = re.search(r"(\d+)\s+maximum resident set size", result.stderr) if timed else None
    report = {"schema_version": 1, "outcome": outcome, "exit_code": result.returncode,
              "corpus_sha256": sha256(corpus_bytes), "derived_cases_sha256": cases_hash,
              "protocol_sha256": sha256(path.read_bytes()), "reference_verification": verified,
              "required_cases": dict(Counter(case["operation"] for case in cases)),
              "maxima_prepared_including_failure": maxima, "case_memory": memory,
              "memory_scope": "Real capacities + inline metadata of resident Program, optional plan and successful run buffers. Excludes allocator and construction/compile/plan temporaries; not RSS. Failed run buffer capacities unavailable.",
              "rss": {"status": "measured" if match else "unavailable", "bytes": int(match[1]) if match else None,
                      "method": "/usr/bin/time -l on macOS" if timed else "unavailable on this platform",
                      "scope": "Rust verifier process: load, all sequential construction/compilation/planning/evaluation; excludes Python and Rust compilation"},
              "growth": estimate(prepared), "command": command,
              "limits": {"load_bytes": 8*1024**2, "build_nodes": 1_000_000, "compile_nodes": 1_000_000,
                         "plan_nodes": 1_000_000, "plan_slots": 100_000, "evaluation_nodes": 1_000_000},
              "rust": subprocess.check_output(["rustc", "-Vv"], text=True),
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "sources": {str(p.relative_to(ROOT)): sha256(p.read_bytes()) for p in
                          [ROOT / "src/synth.rs", *sorted((ROOT / "src/synth").rglob("*.rs")),
                            ROOT / "examples/verify_components.rs", ROOT / "examples/support/components.rs", Path(__file__).resolve()]}}
    write_json(args.output_dir / "report.json", report)
    print(json.dumps({"outcome": {k: v for k, v in outcome.items() if k != "failure"},
                      "failure": {k: v for k, v in outcome.get("failure", {}).items() if k != "case"},
                      "maxima": maxima, "rss": report["rss"], "growth": report["growth"],
                      "evidence": str(args.output_dir)}, indent=2))
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
