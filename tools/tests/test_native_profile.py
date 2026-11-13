import copy
import json
from pathlib import Path
import subprocess
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_profile import ProfileError, load_profile, validate_profile


class NativeProfileTests(unittest.TestCase):
    def test_rust_default_and_tool_profile_agree(self):
        result = subprocess.run(["cargo", "run", "--quiet", "--release", "--example", "native_profile"],
                                cwd=Path(__file__).resolve().parents[2], capture_output=True, text=True, check=True)
        self.assertEqual(json.loads(result.stdout), load_profile()["limits"])

    def test_zero_structural_budgets_are_finite_but_supervision_is_positive(self):
        profile = load_profile()
        profile["limits"] = dict.fromkeys(profile["limits"], 0)
        self.assertEqual(validate_profile(profile), profile)
        for field in profile["supervision"]:
            invalid = copy.deepcopy(profile)
            invalid["supervision"][field] = 0
            with self.assertRaises(ProfileError):
                validate_profile(invalid)

    def test_rejects_missing_unknown_overflow_boolean_and_nonfinite_limits(self):
        for field, value in [("ports", -1), ("roots", True), ("base_nodes", sys.maxsize + 1),
                             ("numeric_slots", 1.0), ("unknown", 1)]:
            profile = load_profile()
            profile["limits"][field] = value
            with self.assertRaises(ProfileError):
                validate_profile(profile)
        for value in [float("inf"), float("nan"), -1, True, 10**1000]:
            profile = load_profile()
            profile["supervision"]["phase_timeout_seconds"] = value
            with self.assertRaises(ProfileError):
                validate_profile(profile)
        profile = load_profile()
        del profile["limits"]["ports"]
        with self.assertRaises(ProfileError):
            validate_profile(profile)


if __name__ == "__main__":
    unittest.main()
