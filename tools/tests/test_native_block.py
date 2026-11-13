import json
from pathlib import Path
import sys
import tempfile
import unittest
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from prepare_tiny import sha256
from verify_native_block import compare_dump


class NativeBlockVerifierTests(unittest.TestCase):
    def fixture(self):
        temporary = tempfile.TemporaryDirectory(prefix="native-block-")
        self.addCleanup(temporary.cleanup)
        directory = Path(temporary.name)
        reference = directory / "reference"
        reference.mkdir()
        stages = [
            {"name":"embedding","shape":[1,2],"start":0,"count":2,"layout":"RowMajor"},
            {"name":"layers.0.scores","shape":[1,6],"start":2,"count":6,"layout":"Causal"},
            {"name":"layers.0.residual","shape":[1,2],"start":8,"count":2,"layout":"RowMajor"},
        ]
        values = np.zeros(10,dtype="<f8")
        payload = values.tobytes()
        (reference / "values.bin").write_bytes(payload)
        (reference / "trace.json").write_text(json.dumps({"sha256":sha256(payload),"tensors":{
            s["name"]:{"shape":s["shape"],"dtype":"<f8","offset":s["start"]*8,"bytes":s["count"]*8} for s in stages}}))
        for variant in ["diagnostic","residual"]:
            chosen = stages if variant == "diagnostic" else [{**stages[-1],"start":0}]
            (directory / f"{variant}.stages.jsonl").write_text("".join(json.dumps(s)+"\n" for s in chosen))
            vector = values if variant == "diagnostic" else values[-2:]
            for engine in ["original","planned"]:
                vector.tofile(directory / f"{variant}.{engine}.f64")
            np.arange(len(vector),dtype="<u8").tofile(directory / f"{variant}.roots.u64")
        return directory,reference

    def test_all_stages_both_readers_and_diagnostic_variant_are_required(self):
        directory,reference = self.fixture()
        result = compare_dump(directory,reference)
        self.assertEqual(result["diagnostic"]["roots"],10)
        self.assertEqual(result["residual"]["roots"],2)
        path = directory / "diagnostic.stages.jsonl"
        path.write_text("\n".join(path.read_text().splitlines()[1:])+"\n")
        with self.assertRaises(AssertionError):
            compare_dump(directory,reference)

    def test_mismatch_reports_exact_causal_position_and_root(self):
        directory,reference = self.fixture()
        for engine in ["original","planned"]:
            path = directory / f"diagnostic.{engine}.f64"
            values = np.fromfile(path,dtype="<f8")
            values[5] = 0.001
            values.tofile(path)
        with self.assertRaises(ValueError) as caught:
            compare_dump(directory,reference)
        failure = caught.exception.args[0]
        self.assertEqual((failure["stage"],failure["root"],failure["position"],failure["key_position"]),
                         ("layers.0.scores",5,2,0))

    def test_nonfinite_value_cannot_pass_comparison(self):
        directory,reference = self.fixture()
        path = directory / "diagnostic.planned.f64"
        values = np.fromfile(path,dtype="<f8")
        values[0] = np.nan
        values.tofile(path)
        with self.assertRaises(AssertionError):
            compare_dump(directory,reference)


if __name__ == "__main__":
    unittest.main()
