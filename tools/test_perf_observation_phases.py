"""Preparation-only regressions. No builds, Rust imports, workloads or network.

Parent may run: python -B -m unittest discover -s tools -p test_perf_observation_phases.py
These checks do not establish compiled instrumentation or trace parity.
"""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import perf_observation_phases as phases


PINNED = Path(__file__).resolve().parent / "fixtures" / "observation-phases"


def PINNED_SOURCE(name):
    """Vendored copy of one guarded source blob."""
    return PINNED / ("Cargo.toml.pinned" if name == "Cargo.toml" else name)


class PinnedFixtureTests(unittest.TestCase):
    def test_vendored_sources_are_the_guarded_blobs(self):
        for name, blob in phases.SHAPE.items():
            vendored = phases.git(phases.ROOT, "hash-object", "--no-filters", str(PINNED_SOURCE(name)))
            self.assertEqual(vendored.decode().strip(), blob, name)


class PreparationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # CI has a shallow checkout, and local pre-squash commits need not exist
        # on the remote. Preparation tests own their Git objects and blob guards;
        # production source guards and retained public-pin parity stay unchanged.
        temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(temporary.cleanup)
        cls.root = Path(temporary.name) / "source"
        cls.root.mkdir()
        files = [phases.ROOT / name for name in ("Cargo.toml", "Cargo.lock", "README.md", phases.HARNESS)]
        files.extend((phases.ROOT / "src").rglob("*.rs"))
        for source in files:
            target = cls.root / source.relative_to(phases.ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(source.read_bytes())
        # The recipe instruments only the reviewed shape, which current sources
        # have since left; overlay the vendored pinned blobs over the copy.
        for name in phases.SHAPE:
            (cls.root / name).write_bytes(PINNED_SOURCE(name).read_bytes())
        phases.git(cls.root, "init", "--quiet", "--template=")
        phases.git(cls.root, "add", ".")
        cls.commit("fixture")
        cls.base = phases.git(cls.root, "rev-parse", "HEAD").decode().strip()
        cls.tree = phases.git(cls.root, "rev-parse", "HEAD^{tree}").decode().strip()
        guards = {name: phases.git(cls.root, "rev-parse", f"HEAD:{name}").decode().strip()
                  for name in phases.SHAPE}
        replacement = patch.dict(phases.SHAPE, guards, clear=True)
        replacement.start()
        cls.addClassCleanup(replacement.stop)

    @classmethod
    def commit(cls, message):
        phases.git(cls.root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                   "-c", "core.hooksPath=", "commit", "--quiet", "--allow-empty", "-m", message)

    def test_equivalent_commits_accept_identical_source(self):
        self.commit("same tree")
        other = phases.git(self.root, "rev-parse", "HEAD").decode().strip()
        self.assertNotEqual(other, self.base)
        self.assertEqual(phases.source_files(self.root, other), phases.source_files(self.root, self.base))

    def test_prepare_exact_source_and_retained_recipe(self):
        original, tree = phases.source_files(self.root, self.base)
        self.assertEqual(tree, self.tree)
        with tempfile.TemporaryDirectory() as temporary:
            output = phases.prepare(self.root, self.base, Path(temporary) / "phase-prep")
            manifest = json.loads((output / "manifest.json").read_bytes())
            self.assertEqual(manifest["baseCommit"], self.base)
            self.assertEqual(manifest["baseTree"], self.tree)
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
                phases.prepare(self.root, self.base, output)

    def test_exact_commit_not_ref_and_unknown_shape(self):
        with self.assertRaises(ValueError):
            phases.source_files(self.root, "HEAD")
        with patch.dict(phases.SHAPE, {phases.OBSERVATION: "0" * 40}):
            with self.assertRaises(ValueError):
                phases.source_files(self.root, self.base)
        original, _ = phases.source_files(self.root, self.base)
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
            phases.safe_output(self.root, Path("relative-output"))
        with self.assertRaises(ValueError):
            phases.safe_output(self.root, self.root / "new-phase-output")
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(ValueError):
                phases.safe_output(self.root, Path(temporary) / ".." / "escape")

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
