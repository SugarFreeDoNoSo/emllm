"""Pruebas de integridad locales; no descargan ni cargan el checkpoint."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from prepare_tiny import PreparationError, verify_blob


class SourceIntegrityTests(unittest.TestCase):
    def setUp(self):
        self.entry = {
            "name": "fixture.bin",
            "size": 3,
            "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        }

    def test_original_and_same_length_corruption(self):
        verify_blob(b"abc", self.entry)
        with self.assertRaisesRegex(PreparationError, "hash_mismatch"):
            verify_blob(b"abd", self.entry)

    def test_truncation_and_extra_data(self):
        for data in (b"ab", b"abcd"):
            with self.assertRaisesRegex(PreparationError, "size_mismatch"):
                verify_blob(data, self.entry)


if __name__ == "__main__":
    unittest.main()
