"""Contratos de los oráculos; las trazas reales se verifican explícitamente."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from reference import ReferenceError, Trace, compare, pack_causal
from reference_corpus import balanced_sum, selected_indices
import torch


class ReferenceTests(unittest.TestCase):
    def test_dtype_audit_and_nonfinite_are_not_silently_cast_or_filtered(self):
        trace = Trace(torch.float64)
        with self.assertRaisesRegex(ReferenceError, "dtype_or_device"):
            trace.put("norm", torch.tensor([1.0], dtype=torch.float32))
        for value in [float("nan"), float("inf")]:
            with self.assertRaisesRegex(ReferenceError, "nonfinite"):
                trace.put("norm", torch.tensor([value], dtype=torch.float64))
        trace.put("norm", torch.tensor([1.0], dtype=torch.float64))
        with self.assertRaisesRegex(ReferenceError, "duplicate_stage"):
            trace.put("norm", torch.tensor([2.0], dtype=torch.float64))

    def test_comparison_checks_all_coordinates_with_fixed_bounds(self):
        ref = torch.tensor([0.0, 10.0], dtype=torch.float64)
        result = compare(torch.tensor([0.00011, 10.0005], dtype=torch.float64), ref)
        self.assertEqual(result["count"], 2)
        self.assertEqual(result["failures"], 1)
        self.assertAlmostEqual(result["max_tolerance_ratio"], 1.1)
        with self.assertRaisesRegex(ReferenceError, "nonfinite"):
            compare(torch.tensor([float("nan"), 10.0]), ref)

    def test_causal_packing_excludes_only_future_positions(self):
        values = torch.tensor([[[[0.0, -torch.inf, -torch.inf],
                                 [10.0, 11.0, -torch.inf],
                                 [20.0, 21.0, 22.0]]]])
        self.assertEqual(pack_causal(values).tolist(), [[0, 10, 11, 20, 21, 22]])

    def test_corpus_selection_ties_and_balanced_reduction(self):
        self.assertEqual(selected_indices([2, 3, 1, 0, -8, 12, 0.01, 0.01]), list(range(7)))
        self.assertEqual(balanced_sum([1e16, 1.0, -1e16, 1.0, 3.0]), 3.0)
        with self.assertRaisesRegex(ValueError, "nonfinite"):
            selected_indices([1.0, float("nan")])


if __name__ == "__main__":
    unittest.main()
