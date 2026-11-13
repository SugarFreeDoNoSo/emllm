from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from import_model import NONE, Tensor
from verify_native_base import EXPANSION_BITS, checked_package, verify_dump


class NativeBaseVerifierTests(unittest.TestCase):
    def fixture(self):
        directory = tempfile.TemporaryDirectory(prefix="native-base-")
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        bits = sorted(EXPANSION_BITS | {struct.unpack("<I", struct.pack("<f", i))[0] for i in range(512)})
        constants = {b: i for i, b in enumerate(bits)}
        rows = []
        for b, index in constants.items():
            value, = struct.unpack("<f", struct.pack("<I", b))
            rows.append(f"{b:08x}\t{index}\t{value}\t{value}\n")
        (root / "constants.tsv").write_text("".join(rows))
        (root / "tensors.tsv").write_text(f"0\t0\t3f800000\t{constants[0x3f800000]}\n"
                                          f"1\t0\t3f800000\t{constants[0x3f800000]}\n")
        (root / "vocabulary.tsv").write_text("".join(f"{i}\t{constants[struct.unpack('<I', struct.pack('<f', i))[0]]}\n"
                                                     for i in range(512)))
        tensors = [Tensor(1, NONE, (1, 1), struct.pack("<f", 1)),
                   Tensor(12, NONE, (1, 1), struct.pack("<f", 1), 0)]
        events = [{"event": "tensor", "index": i, "kind": t.kind, "layer": None,
                   "shape": [1, 1], "alias": None if i == 0 else 0, "scalars": 1} for i, t in enumerate(tensors)]
        events += [{"event": "base-prepared", "constants": len(bits), "nodes": len(bits),
                    "tensors": 2, "vocabulary_symbols": 512}, {"event": "base-verified", "constants": len(bits)}]
        return root, tensors, events

    def test_every_coordinate_alias_and_vocabulary_root_are_checked(self):
        root, tensors, events = self.fixture()
        result = verify_dump(root, tensors, events)
        self.assertEqual(result["tensor_coordinates_including_aliases"], 2)
        (root / "tensors.tsv").write_text("0\t0\t3f800000\t999999\n")
        with self.assertRaises(AssertionError):
            verify_dump(root, tensors, events)

    def test_invalid_numeric_value_is_rejected_even_with_matching_structure(self):
        root, tensors, events = self.fixture()
        text = (root / "constants.tsv").read_text().splitlines()
        fields = text[0].split()
        fields[2] = "nan"
        text[0] = "\t".join(fields)
        (root / "constants.tsv").write_text("\n".join(text) + "\n")
        with self.assertRaises(AssertionError):
            verify_dump(root, tensors, events)

    def test_hash_rejected_before_reading_or_preparing_an_incompatible_package(self):
        root, _, _ = self.fixture()
        package = root / "model.emltiny"
        package.write_bytes(b"EMLTINY\x00")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            checked_package(package)


if __name__ == "__main__":
    unittest.main()
