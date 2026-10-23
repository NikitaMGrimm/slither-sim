"""Preparation-only regressions. No builds, Rust imports, workloads or network.

Parent may run: python -B -m unittest discover -s tools -p test_perf_observation_phases.py
These checks do not establish compiled instrumentation or trace parity.
"""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import perf_observation_phases as phases


BASE = "e01bc5592083b8e10ee6404d5732f9a70acfac59"
PUBLIC = "20dd3aff5574af0270d8ad2c1e93f9001e8f1142"
TREE = "8e410d0f3246e07a2d73332aa15561ad865c1b99"


class PreparationTests(unittest.TestCase):
    def test_equivalent_public_pin_accepts_identical_source(self):
        self.assertEqual(phases.source_files(phases.ROOT, PUBLIC), phases.source_files(phases.ROOT, BASE))

    def test_prepare_exact_source_and_retained_recipe(self):
        original, tree = phases.source_files(phases.ROOT, BASE)
        self.assertEqual(tree, TREE)
        with tempfile.TemporaryDirectory() as temporary:
            output = phases.prepare(phases.ROOT, BASE, Path(temporary) / "phase-prep")
            manifest = json.loads((output / "manifest.json").read_bytes())
            self.assertEqual(manifest["baseCommit"], BASE)
            self.assertEqual(manifest["baseTree"], TREE)
            self.assertEqual(manifest["status"], "prepared-only-unexecuted")
            for variant in ("baseline", "instrumented"):
                files = {str(p.relative_to(output / variant)).replace("\\", "/"): p
                         for p in (output / variant).rglob("*") if p.is_file()}
                self.assertEqual(set(files), set(manifest[variant + "Sha256"]))
                for name, path in files.items():
                    self.assertEqual(phases.sha256(path.read_bytes()), manifest[variant + "Sha256"][name])
                self.assertEqual(files[phases.HARNESS].read_bytes(), original[phases.HARNESS])
                for name in original:
                    if name != "Cargo.toml" and not (variant == "instrumented" and name == phases.OBSERVATION):
                        self.assertEqual(files[name].read_bytes(), original[name])
                self.assertNotIn(phases.PORTABLE_TARGET.encode(), files["Cargo.toml"].read_bytes())
            self.assertFalse((output / "PREPARATION_INCOMPLETE").exists())
            self.assertEqual((output / "recipe/perf_observation_phases.rs").read_bytes(), phases.RESOURCE.read_bytes())
            commands = (output / "COMMANDS.txt").read_text()
            self.assertIn("--offline", commands)
            self.assertIn("timeout=600", commands)
            self.assertIn("timeout=120", commands)
            self.assertIn("compare-traces", commands)
            with self.assertRaises(ValueError):
                phases.prepare(phases.ROOT, BASE, output)

    def test_exact_commit_not_ref_and_unknown_shape(self):
        with self.assertRaises(ValueError):
            phases.source_files(phases.ROOT, "HEAD")
        original, _ = phases.source_files(phases.ROOT, BASE)
        with self.assertRaises(ValueError):
            phases.instrument(original[phases.OBSERVATION].replace(b"candidates.sort_unstable();", b"candidates.sort();"))
        # Insertion-only hooks leave exact visibility math and the early return intact.
        generated = phases.instrument(original[phases.OBSERVATION])
        self.assertIn(b"&& p.distance(me.head) <= vision", generated)
        self.assertIn(b"return Ok((0..self.snakes.len()).map(|_| None).collect());", generated)
        self.assertIn(b"candidates.sort_unstable();", generated)

    def test_output_guards(self):
        for name in ("CON", "aux.txt", "abc.", "..", "a:b", "a/b"):
            self.assertFalse(phases.safe_component(name))
        with self.assertRaises(ValueError):
            phases.safe_output(phases.ROOT, Path("relative-output"))
        with self.assertRaises(ValueError):
            phases.safe_output(phases.ROOT, phases.ROOT / "new-phase-output")
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(ValueError):
                phases.safe_output(phases.ROOT, Path(temporary) / ".." / "escape")

    def test_trace_comparison_is_complete_and_order_sensitive(self):
        # Synthetic envelopes only: verifies comparator behavior, not runtime parity.
        snapshot = {"world": {}, "pickupEvents": [], "foodSpawnEvents": [], "observations": []}
        records = [
            {"kind": "initial", "case": "observe-dense", "seed": 7, "snapshot": snapshot},
            {"kind": "observe", "iteration": 0, "snapshot": snapshot, "result": [None, {"food": [1, 2, 1]}]},
            {"kind": "report", "case": "observe-dense", "mode": "trace", "iterations": 1,
             "timing": None, "outputDrop": None},
        ]
        data = ("\n".join(json.dumps(r, sort_keys=True) for r in records) + "\n").encode()
        with tempfile.TemporaryDirectory() as temporary:
            left, right = Path(temporary) / "left.jsonl", Path(temporary) / "right.jsonl"
            left.write_bytes(data)
            right.write_bytes(data)
            self.assertEqual(phases.compare_traces(left, right), hashlib.sha256(data).hexdigest())
            right.write_bytes(data.replace(b"[1, 2, 1]", b"[1, 1, 2]"))
            with self.assertRaises(ValueError):
                phases.compare_traces(left, right)
            right.write_bytes(data.splitlines(keepends=True)[0])
            with self.assertRaises(ValueError):
                phases.compare_traces(left, right)


if __name__ == "__main__":
    unittest.main()
