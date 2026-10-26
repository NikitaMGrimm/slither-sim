"""Keep published shared-policy parameter identities tied to the frozen bytes."""
import hashlib
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]


class BotMetadata(unittest.TestCase):
    def test_runtime_and_workload_hash_the_complete_parameter_manifest(self):
        expected = hashlib.sha256((ROOT / "src/bot/data/resolved.json").read_bytes()).hexdigest()
        source = (ROOT / "src/bot/mod.rs").read_text(encoding="utf-8")
        runtime = re.search(r'PARAMETER_MANIFEST_SHA256: &str =\s*"([a-f0-9]{64})"', source)
        self.assertIsNotNone(runtime)
        self.assertEqual(runtime.group(1), expected)
        catalog = (ROOT / "examples/perf_baseline.rs").read_text(encoding="utf-8")
        workload = re.search(r'"parameterManifestSha256": "([a-f0-9]{64})"', catalog)
        self.assertIsNotNone(workload)
        self.assertEqual(workload.group(1), expected)
