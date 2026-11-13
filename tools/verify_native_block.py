"""Puerta del primer bloque, ambos prefijos originales y todas las trazas F64."""
import argparse
import json
from pathlib import Path
import subprocess
import time
import numpy as np
from native_profile import DEFAULT_PROFILE, load_profile
from native_resources import supervise
from prepare_tiny import INPUTS, ROOT, sha256, write_json
from verify_native_base import checked_package
from verify_reference import verify


def compare_dump(directory, reference):
    metadata = json.loads((reference / "trace.json").read_text())
    payload = (reference / "values.bin").read_bytes()
    assert sha256(payload) == metadata["sha256"]
    expected_names = {n for n in metadata["tensors"] if n == "embedding" or n.startswith("layers.0.")}
    results = {}
    residual = None
    for variant in ["diagnostic", "residual"]:
        stages = [json.loads(line) for line in (directory / f"{variant}.stages.jsonl").read_text().splitlines()]
        assert {s["name"] for s in stages} == (expected_names if variant == "diagnostic" else {"layers.0.residual"})
        assert len(stages) == len({s["name"] for s in stages})
        original = np.fromfile(directory / f"{variant}.original.f64", dtype="<f8")
        planned = np.fromfile(directory / f"{variant}.planned.f64", dtype="<f8")
        roots = np.fromfile(directory / f"{variant}.roots.u64", dtype="<u8")
        assert original.shape == planned.shape == roots.shape
        assert np.isfinite(original).all() and np.isfinite(planned).all()
        assert np.all(np.abs(planned-original) <= 1e-12+1e-12*np.abs(original))
        count = 0
        reports = []
        for stage in stages:
            entry = metadata["tensors"][stage["name"]]
            assert entry["dtype"] == "<f8" and stage["shape"] == entry["shape"]
            assert stage["start"] == count and stage["count"] == int(np.prod(entry["shape"]))
            expected = np.frombuffer(payload, dtype="<f8", count=stage["count"], offset=entry["offset"])
            a = original[count:count+stage["count"]]
            b = planned[count:count+stage["count"]]
            error = np.maximum(np.abs(a-expected), np.abs(b-expected))
            bound = 1e-4+1e-4*np.abs(expected)
            failed = np.flatnonzero(error > bound)
            if len(failed):
                i = int(failed[0]); coordinates = list(map(int,np.unravel_index(i,stage["shape"])))
                location = {"stage":stage["name"],"coordinates":coordinates,"root":count+i,
                            "closed_node":int(roots[count+i]),"original":float(a[i]),"planned":float(b[i]),
                            "expected":float(expected[i]),"absolute_error":float(error[i]),"bound":float(bound[i])}
                if stage["layout"] == "Causal":
                    position, key = 0, coordinates[1]
                    while key > position:
                        key -= position+1
                        position += 1
                    location.update(position=position,key_position=key)
                else:
                    location["position"] = coordinates[0]
                raise ValueError(location)
            reports.append({"name":stage["name"],"coordinates":stage["count"],"max_threshold_fraction":float(np.max(error/bound))})
            if stage["name"] == "layers.0.residual":
                if residual is None: residual = a.copy()
                else: assert np.all(np.abs(a-residual) <= 1e-12+1e-12*np.abs(residual))
            count += stage["count"]
        assert count == len(original)
        results[variant] = {"stages":reports,"roots":count}
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package",type=Path,default=ROOT / "target/tiny-model/canonical/model.emltiny")
    parser.add_argument("--reference-dir",type=Path,default=ROOT / "target/tiny-model/reference")
    parser.add_argument("--profile",type=Path,default=DEFAULT_PROFILE)
    parser.add_argument("--output-dir",type=Path,required=True)
    args = parser.parse_args()
    checked_package(args.package)
    verified = verify(args.reference_dir,args.package)
    fixtures = json.loads(INPUTS.read_text())["fixtures"]
    profile = load_profile(args.profile)
    args.output_dir.mkdir(parents=True,exist_ok=False)
    write_json(args.output_dir / "profile.json",profile)
    report = {"status":"running","reference_verification":verified,"fixtures":{},"profile":profile,
              "revision":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
              "sources":{str(p.relative_to(ROOT)):sha256(p.read_bytes()) for p in [*sorted((ROOT / "src").rglob("*.rs")),
                ROOT / "examples/verify_native_block.rs",Path(__file__).resolve()]}}
    write_json(args.output_dir / "report.json",report)
    subprocess.run(["cargo","build","--release","--example","verify_native_block"],cwd=ROOT,check=True)
    for fixture in fixtures:
        path = args.output_dir / fixture["name"]
        command = [str(ROOT / "target/release/examples/verify_native_block"),str(args.package.resolve()),str((path / "dump").resolve()),
                   ",".join(map(str,fixture["token_ids"])),*[f"{k}={v}" for k,v in profile["limits"].items()]]
        supervision = supervise(command,path,profile,cwd=ROOT)
        events = [json.loads(line) for line in (path / "stdout.log").read_text().splitlines() if line.startswith("{")]
        entry = {"status":supervision["status"],"supervision":supervision,"events":events}
        failures = [event for event in events if event["event"] == "failed"]
        if failures:
            entry["failure"] = failures[0]
        diagnostics = [event for event in events if event["event"] == "failure-diagnostic"]
        if diagnostics:
            entry["failure_diagnostic"] = diagnostics[0]
        report["fixtures"][fixture["name"]] = entry
        if entry["status"] == "ok":
            start = time.monotonic()
            try:
                entry["comparison"] = compare_dump(path / "dump",args.reference_dir / fixture["name"] / "explicit_f64")
            except (ValueError,AssertionError,OSError) as error:
                entry.update(status="invalid",failure=error.args[0] if error.args else repr(error))
            entry["external_comparison_seconds"] = time.monotonic()-start
        write_json(args.output_dir / "report.json",report)
        if entry["status"] != "ok":
            report["status"] = "failed"
            break
    else:
        report["status"] = "ok"
    write_json(args.output_dir / "report.json",report)
    print(json.dumps({"status":report["status"],"fixtures":{k:{"status":v["status"],"failure":v.get("failure",v["supervision"]["failure"]),
                      "last_phase":v["supervision"]["last_phase"]} for k,v in report["fixtures"].items()}},indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
