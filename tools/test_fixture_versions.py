"""Historical fixture labels, independent of the decoder's current defaults."""

import json
import unittest
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[1]


class FixtureVersionsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads(
            (ROOT / "tests/fixtures/manifest.json").read_text(encoding="utf-8")
        )

    def fixture(self, path):
        entries = [row for row in self.manifest["fixtures"] if row["path"] == path]
        self.assertEqual(len(entries), 1, path)
        return entries[0], json.loads((ROOT / path).read_text(encoding="utf-8"))

    def assert_protocol19_setup(self, entry, hex_text):
        # Offset 23 belongs to these historical protocol-19 a packets. Do not
        # import PROTOCOL_VERSION: changing the default cannot relabel old bytes.
        packet = bytes.fromhex(hex_text)
        self.assertGreaterEqual(len(packet), 25)
        self.assertEqual(packet[0], ord("a"))
        self.assertEqual(packet[23], 19)
        self.assertEqual(entry["wireProtocol"], packet[23])

    def test_inventory_schema_and_unique_existing_paths(self):
        self.assertEqual(set(self.manifest), {"schemaVersion", "fixtures"})
        self.assertIs(type(self.manifest["schemaVersion"]), int)
        self.assertEqual(self.manifest["schemaVersion"], 1)
        self.assertIsInstance(self.manifest["fixtures"], list)
        self.assertTrue(self.manifest["fixtures"])
        seen = set()
        for entry in self.manifest["fixtures"]:
            with self.subTest(entry=entry):
                self.assertEqual(set(entry), {
                    "path", "wireProtocol", "clientContract",
                    "dataOrigin", "expectedOrigin",
                })
                self.assertIsInstance(entry["path"], str)
                path = PurePosixPath(entry["path"])
                self.assertFalse(path.is_absolute())
                self.assertNotIn("..", path.parts)
                self.assertEqual(path.as_posix(), entry["path"])
                self.assertTrue(
                    path.parts[:2] == ("tests", "fixtures")
                    or path.parts[:3] == ("tools", "fidelity", "fixtures")
                )
                resolved = (ROOT / entry["path"]).resolve()
                self.assertTrue(resolved.is_relative_to(ROOT))
                self.assertEqual(resolved.suffix, ".json")
                self.assertTrue(resolved.is_file(), entry["path"])
                self.assertNotIn(resolved, seen)
                seen.add(resolved)
                self.assertIs(type(entry["wireProtocol"]), int)
                self.assertTrue(0 <= entry["wireProtocol"] <= 255)
                contract = entry["clientContract"]
                self.assertTrue(contract is None or (
                    isinstance(contract, str) and bool(contract.strip())
                ))
                self.assertIn(
                    (entry["dataOrigin"], entry["expectedOrigin"]),
                    {("captured", "reference-decoder"), ("synthetic", "synthetic")},
                )

    def test_protocol19_paired_capture_and_embedded_setup(self):
        entry, fixture = self.fixture("tests/fixtures/client-protocol-paired-coil.json")
        self.assertEqual(entry["dataOrigin"], "captured")
        self.assertIsNone(entry["clientContract"])
        self.assertTrue(fixture["source"]["archive"])
        self.assertTrue(fixture["reference"])
        # Record 5 starts with the elapsed-ms header and a two-byte packet
        # length of 61; subsequent packets in this frame are unrelated.
        record = fixture["records"][5]
        self.assertEqual(record[0], "i")
        frame = bytes.fromhex(record[2])
        self.assertEqual(frame[2:4], bytes([0, 61]))
        self.assertGreaterEqual(len(frame), 65)
        self.assert_protocol19_setup(entry, frame[4:65].hex())
        setup = fixture["wslSetup"]
        self.assert_protocol19_setup(entry, setup["setupPrefixHex"])
        self.assertEqual(setup["expected"]["protocolVersion"], entry["wireProtocol"])

    def test_official_catalog_derivatives_keep_both_wire_labels(self):
        for version in (13, 19):
            entry, fixture = self.fixture(f"tests/fixtures/protocol-catalog-{version}.json")
            self.assertEqual(entry["wireProtocol"], version)
            self.assertEqual(fixture["wireProtocol"], version)
            self.assertIn("Derived captured", fixture["transformation"])
            self.assertTrue(fixture["sources"])
            self.assertTrue(fixture["records"])

    def test_captured_legacy_login_pins_unsupported_protocol13(self):
        entry, fixture = self.fixture("tests/fixtures/client-legacy-login.json")
        self.assertEqual(entry["wireProtocol"], 13)
        self.assertEqual(fixture["wireProtocol"], 13)
        self.assertEqual(entry["dataOrigin"], "captured")
        self.assertIsNone(entry["clientContract"])
        session = fixture["joinedSession"]
        self.assertEqual(session["request"]["hex"], "6304")
        prefix = bytes.fromhex(session["setup"]["commandPrefixHex"])
        self.assertEqual(len(prefix), 25)
        self.assertEqual(prefix[0], ord("a"))
        self.assertEqual(prefix[23], 13)
        self.assertEqual(session["setup"]["expected"]["protocol"], prefix[23])
        # The retained stock prefix is a separate observation, still tagged 19.
        _, stock = self.fixture("tools/fidelity/fixtures/protocol19-setup.json")
        stock_prefix = bytes.fromhex(stock["setupPrefixHex"])
        self.assertEqual(stock_prefix[23], 19)
        self.assertEqual(prefix[:23], stock_prefix[:23])
        self.assertEqual(prefix[24], stock_prefix[24])
        login = bytes.fromhex(session["login"]["hex"])
        self.assertEqual(session["login"]["dataOrigin"], "synthetic")
        self.assertEqual(login[:4], bytes([115, 12, 7, 11]))
        self.assertEqual(login[4:15], b"CompatProbe")
        self.assertEqual(login[15:], bytes([0, 255]))

    def test_protocol19_hud_capture(self):
        entry, fixture = self.fixture("tests/fixtures/client-hud-leaderboard.json")
        self.assertEqual(entry["dataOrigin"], "captured")
        self.assertIsNone(entry["clientContract"])
        self.assertTrue(fixture["source"]["archive"])
        self.assertTrue(fixture["reference"])
        self.assert_protocol19_setup(entry, fixture["setup"]["hex"])
        self.assertEqual(fixture["setup"]["expected"]["protocol"], entry["wireProtocol"])

    def test_protocol19_partial_setup_capture(self):
        entry, fixture = self.fixture("tools/fidelity/fixtures/protocol19-setup.json")
        self.assertEqual(entry["dataOrigin"], "captured")
        self.assertIsNone(entry["clientContract"])
        self.assertTrue(fixture["source"]["partialArchive"])
        self.assertTrue(fixture["source"]["archive"])
        self.assert_protocol19_setup(entry, fixture["setupPrefixHex"])
        self.assertEqual(fixture["expected"]["protocolVersion"], entry["wireProtocol"])

    def test_synthetic_v2_contract_is_separate_from_wire_version(self):
        entry, fixture = self.fixture("tests/fixtures/client-contract-v2.json")
        self.assertEqual(entry["dataOrigin"], fixture["dataOrigin"])
        self.assertEqual(entry["dataOrigin"], "synthetic")
        self.assertEqual(entry["clientContract"], "slither-client-v2")
        self.assertEqual(fixture["frame"]["contract"], entry["clientContract"])
        self.assertEqual(entry["wireProtocol"], 19)
        self.assertEqual(fixture["frame"]["settings"]["protocol"], entry["wireProtocol"])

    def test_synthetic_serve_v3_error_contract(self):
        fixture = json.loads((ROOT / "tests/fixtures/serve-errors-v3.json").read_text(encoding="utf-8"))
        self.assertIn("Synthetic", fixture["description"])
        for name, ended in (("rejectedReply", False), ("terminalReply", True)):
            reply = fixture[name]
            self.assertEqual(set(reply), {"protocol", "error", "sessionEnded"})
            self.assertEqual(reply["protocol"], "slither-sim-json-v3")
            self.assertIs(reply["sessionEnded"], ended)
            self.assertIsInstance(reply["error"], str)

    def test_synthetic_serve_v4_error_contract(self):
        fixture = json.loads((ROOT / "tests/fixtures/serve-errors-v4.json").read_text(encoding="utf-8"))
        self.assertIn("Synthetic", fixture["description"])
        for name, ended in (("rejectedReply", False), ("terminalReply", True)):
            reply = fixture[name]
            self.assertEqual(set(reply), {"protocol", "error", "sessionEnded"})
            self.assertEqual(reply["protocol"], "slither-sim-json-v4")
            self.assertIs(reply["sessionEnded"], ended)
            self.assertIsInstance(reply["error"], str)

    def test_synthetic_serve_v5_structured_errors_and_rules(self):
        fixture = json.loads((ROOT / "tests/fixtures/serve-errors-v5.json").read_text())
        for name, code, ended in [("rejectedReply", 11, False), ("terminalReply", 15, True)]:
            reply = fixture[name]
            self.assertEqual(reply["protocol"], "slither-sim-json-v5")
            self.assertIs(reply["sessionEnded"], ended)
            self.assertEqual(set(reply["error"]), {"code", "rule", "world", "slot", "field", "message"})
            self.assertEqual(reply["error"]["code"], code)
        rules = json.loads((ROOT / "tests/fixtures/serve-rules-v5.json").read_text())
        self.assertEqual(rules["protocol"], "slither-sim-json-v5")
        self.assertEqual(set(rules["rules"]), {"boundaryMode", "pitRadius", "minScore", "killScoreSeconds", "freeBoostAtFloor"})

    def test_serve_v6_copies_change_only_protocol(self):
        for name in ("serve-errors", "serve-rules", "serve-radius"):
            old = json.loads((ROOT / f"tests/fixtures/{name}-v5.json").read_text())
            new = json.loads((ROOT / f"tests/fixtures/{name}-v6.json").read_text())
            def migrate(value):
                if isinstance(value, dict):
                    return {k: migrate(v) for k, v in value.items()}
                if isinstance(value, list):
                    return [migrate(v) for v in value]
                return "slither-sim-json-v6" if value == "slither-sim-json-v5" else value
            self.assertEqual(new, migrate(old), name)

    def test_protocol19_food_interest_receipts(self):
        entry, fixture = self.fixture("tests/fixtures/client-food-interest.json")
        self.assertEqual(entry["wireProtocol"], 19)
        self.assertEqual(fixture["wireProtocol"], 19)
        self.assertEqual(entry["dataOrigin"], fixture["dataOrigin"])
        self.assertIsNone(entry["clientContract"])
        self.assertEqual(len(fixture["source"]["sha256"]), 64)
        self.assertTrue(fixture["entries"])

    def test_protocol19_reporting_cadence_is_source_pinned(self):
        entry, fixture = self.fixture("tests/fixtures/client-reporting-cadence.json")
        self.assertEqual(entry["wireProtocol"], fixture["wireProtocol"])
        self.assertEqual(entry["dataOrigin"], fixture["dataOrigin"])
        for key in ("archiveSha256", "decoderSha256", "censusSha256"):
            self.assertEqual(len(fixture["source"][key]), 64)
        self.assertEqual(fixture["preyUpdateGapsMs"]["n"], 144)
        self.assertEqual(fixture["leaderboardGapsMs"]["n"], 27)

    def test_clean_server_cadence_and_scheduler_labels(self):
        for name, contract in (("client-reporting-cadence-near-server.json", None),
                               ("scheduler-near-server-witnesses.json", "slither-client-v2")):
            entry, fixture = self.fixture("tests/fixtures/" + name)
            self.assertEqual(fixture["wireProtocol"], entry["wireProtocol"])
            self.assertEqual(fixture["clientContract"], contract)
            self.assertEqual(entry["clientContract"], contract)
            self.assertEqual(fixture["dataOrigin"], "captured")
            self.assertEqual(len(fixture["provenance"]["decoderSha256"]), 64)
        for case in fixture["cases"]:
            self.assert_protocol19_setup(entry, case["setupHex"])
            self.assertEqual(len(case["source"]["archiveSha256"]), 64)


if __name__ == "__main__":
    unittest.main()
