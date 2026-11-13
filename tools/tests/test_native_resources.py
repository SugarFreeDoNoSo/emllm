import json
from pathlib import Path
import signal
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_profile import load_profile
from native_resources import supervise

ROOT = Path(__file__).resolve().parents[2]
PRELUDE = "import os, signal, sys, time; sys.path.insert(0, 'tools'); from native_resources import emit_phase\n"


class NativeResourceTests(unittest.TestCase):
    def run_child(self, body, *, timeout=3, rss=1024**3, reader=None):
        profile = load_profile()
        profile["supervision"] = {"max_rss_bytes": rss, "phase_timeout_seconds": timeout}
        temporary = tempfile.TemporaryDirectory(prefix="native-resources-")
        self.addCleanup(temporary.cleanup)
        output = Path(temporary.name) / "run"
        kwargs = {} if reader is None else {"rss_reader": reader}
        report = supervise([sys.executable, "-u", "-c", PRELUDE + body], output, profile,
                           cwd=ROOT, interval=0.02, grace=0.1, **kwargs)
        self.assertEqual(report, json.loads((output / "report.json").read_text()))
        return report, output

    def test_completed_phases_stream_output_and_rss_have_declared_scopes(self):
        report, output = self.run_child("emit_phase('start', 'base')\n"
                                       "print('partial evidence', flush=True)\n"
                                       "time.sleep(0.15)\nemit_phase('end', 'base')\n"
                                       "emit_phase('start', 'compose')\ntime.sleep(0.15)\nemit_phase('end', 'compose')")
        self.assertEqual(report["status"], "ok")
        self.assertEqual(report["returncode"], 0)
        self.assertEqual(report["last_phase"], "compose")
        self.assertGreater(report["sampled_peak_rss_bytes"], 0)
        self.assertGreater(report["wait4_peak_rss_bytes"], 0)
        self.assertTrue(all(p["end_seconds"] >= p["start_seconds"] for p in report["phases"]))
        self.assertEqual(len((output / "events.jsonl").read_text().splitlines()), 4)
        self.assertIn("partial evidence", (output / "stdout.log").read_text())
        self.assertIn("group", report["rss_scope"])

    def test_timeout_kills_even_a_child_ignoring_sigterm_and_keeps_last_phase(self):
        report, output = self.run_child("signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
                                       "emit_phase('start', 'base')\nprint('checkpoint', flush=True)\ntime.sleep(10)", timeout=0.3)
        self.assertEqual(report["failure"]["kind"], "timeout")
        self.assertEqual(report["failure"]["phase"], "base")
        self.assertEqual(report["returncode"], -signal.SIGKILL)
        self.assertLess(report["elapsed_seconds"], 3)
        self.assertIn("checkpoint", (output / "stdout.log").read_text())
        self.assertIsNone(report["phases"][0]["end_seconds"])

    def test_observed_rss_excess_terminates_memory_allocating_process(self):
        report, output = self.run_child("emit_phase('start', 'base')\ntime.sleep(0.1)\n"
                                       "data = bytearray(96 * 1024**2)\ntime.sleep(10)", rss=64 * 1024**2)
        self.assertEqual(report["failure"]["kind"], "rss-limit")
        self.assertEqual(report["last_phase"], "base")
        self.assertGreater(report["failure"]["observed_bytes"], 64 * 1024**2)
        self.assertTrue((output / "rss.jsonl").stat().st_size)

    def test_unexpected_signal_preserves_partial_events_and_stderr(self):
        report, output = self.run_child("emit_phase('start', 'template')\n"
                                       "print('diagnostic', file=sys.stderr, flush=True)\ntime.sleep(0.1)\n"
                                       "os.kill(os.getpid(), signal.SIGKILL)")
        self.assertEqual(report["failure"]["kind"], "command-exit")
        self.assertEqual(report["returncode"], -signal.SIGKILL)
        self.assertEqual(report["failure"]["phase"], "template")
        self.assertEqual((output / "stderr.log").read_text(), "diagnostic\n")

    def test_rss_scope_includes_a_child_in_the_same_process_group(self):
        report, output = self.run_child("import subprocess\nemit_phase('start', 'base')\n"
                                       "child = subprocess.Popen([sys.executable, '-c', "
                                       "'import time; data=bytearray(96*1024**2); time.sleep(10)'])\n"
                                       "child.wait()\nemit_phase('end', 'base')", rss=64 * 1024**2)
        self.assertEqual(report["failure"]["kind"], "rss-limit")
        observations = [json.loads(line) for line in (output / "rss.jsonl").read_text().splitlines()]
        self.assertTrue(any(len(o.get("members", [])) >= 2 and o["bytes"] > 64 * 1024**2
                            for o in observations))

    def test_missing_metrics_are_unverified_instead_of_zero_rss(self):
        report, _ = self.run_child("emit_phase('start', 'base')\ntime.sleep(0.1)\nemit_phase('end', 'base')",
                                  reader=lambda _: {"bytes": None, "reason": "controlled unavailable metric"})
        self.assertEqual(report["status"], "unverified")
        self.assertIsNone(report["sampled_peak_rss_bytes"])
        self.assertFalse(report["rss_budget_observed"])
        self.assertIn("controlled unavailable metric", report["rss_unavailable_reasons"])

    def test_open_mismatched_or_missing_phase_invalidates_successful_exit(self):
        for body in ["emit_phase('start', 'base')", "emit_phase('end', 'base')", "print('no events')",
                     "emit_phase('start', 'base'); emit_phase('start', 'compose')"]:
            with self.subTest(body=body):
                report, _ = self.run_child(body)
                self.assertEqual(report["status"], "error")
                self.assertEqual(report["failure"]["kind"], "protocol")

    def test_timeout_applies_before_first_phase(self):
        report, _ = self.run_child("time.sleep(10)", timeout=0.15)
        self.assertEqual(report["failure"]["kind"], "timeout")
        self.assertIsNone(report["last_phase"])

    def test_wait4_peak_is_checked_when_sampler_missed_a_short_peak(self):
        with patch("native_resources.wait4_peak_bytes", return_value=2048):
            report, _ = self.run_child("emit_phase('start', 'base'); emit_phase('end', 'base')", rss=1024,
                                      reader=lambda _: {"bytes": 1, "members": []})
        self.assertEqual(report["failure"]["kind"], "rss-limit")
        self.assertEqual(report["failure"]["observed_bytes"], 2048)


if __name__ == "__main__":
    unittest.main()
