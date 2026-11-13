"""Contrato binario pequeño y corrupción; independiente del checkpoint externo."""

from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from import_model import CONFIG_FIELDS, FormatError, NONE, Tensor, decode_package, encode_package


GOLDEN = Path(__file__).resolve().parents[2] / "tests/fixtures/tiny-package-v1.hex"


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.data = bytes.fromhex(GOLDEN.read_text())

    def decode(self, data):
        return decode_package(data, max_bytes=1024, max_tensors=8)

    def test_golden_layout_bits_and_alias(self):
        config, tensors = self.decode(self.data)
        self.assertEqual(tuple(config[key] for key in CONFIG_FIELDS), (2, 2, 1, 1, 1, 2, 2, 2))
        self.assertEqual([t.name for t in tensors], ["tok_embeddings.weight", "output.weight"])
        self.assertEqual(tensors[0].shape, (2, 2))
        self.assertEqual(struct.unpack("<4I", tensors[0].data), (0x80000000, 0x3F800000, 0xBF000000, 0x40000000))
        self.assertIs(tensors[0].data, tensors[1].data)
        self.assertEqual(tensors[1].alias, 0)
        self.assertEqual(encode_package(config, tensors), self.data)

    def test_scalar_rank_and_alias_content(self):
        config, tensors = self.decode(self.data)
        tensors.append(Tensor(15, NONE, (), struct.pack("<f", 1e-5)))
        _, decoded = self.decode(encode_package(config, tensors))
        self.assertEqual(decoded[2].shape, ())
        tensors[1] = Tensor(12, NONE, (2, 2), bytes(16), 0)
        with self.assertRaisesRegex(FormatError, "alias_content"):
            encode_package(config, tensors)

    def test_structural_corruption(self):
        cases = [
            (8, "<I", 2, "magic_or_version"),
            (56, "<Q", 164, "directory_layout"),
            (64, "<I", 999, "tensor_kind_or_layer"),
            (72, "<I", 3, "rank_or_unused_dimension"),
            (80, "<Q", 1 << 63, "size_overflow"),
            (96, "<Q", 164, "payload_range"),
            (124, "<I", 1, "alias_target"),
            (144, "<Q", 164, "alias_layout"),
            (160, "<I", 0x7FC00000, "nonfinite_scalar"),
        ]
        for offset, fmt, value, error in cases:
            with self.subTest(error=error):
                data = bytearray(self.data)
                struct.pack_into(fmt, data, offset, value)
                with self.assertRaisesRegex(FormatError, error):
                    self.decode(data)

    def test_truncation_trailing_data_and_limits(self):
        for size in (0, 63, 100, 175):
            with self.assertRaises(FormatError):
                self.decode(self.data[:size])
        with self.assertRaisesRegex(FormatError, "trailing_bytes"):
            self.decode(self.data + b"\0")
        with self.assertRaisesRegex(FormatError, "byte_limit"):
            decode_package(self.data, max_bytes=175, max_tensors=8)
        with self.assertRaisesRegex(FormatError, "tensor_limit"):
            decode_package(self.data, max_bytes=1024, max_tensors=1)


if __name__ == "__main__":
    unittest.main()
