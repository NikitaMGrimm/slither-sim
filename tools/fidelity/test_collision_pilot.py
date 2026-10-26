import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import collect
import evidence
import prepare_collision_pilot as pilot


def selection():
    return {"serverListSha256": "a" * 64, "candidates": [
        {"listedIndex": i, "population": population, "archiveSha256": "b" * 64,
         "server": {"ip": "127.0.0.1", "port": 8766 + i}}
        for i, population in enumerate([None, 90, 20, 20, 120])]}


class CollisionPilotTests(unittest.TestCase):
    def test_second_pilot_feasibility_gate_preserves_zero_cells_and_contamination(self):
        def row(kind, qualified=True, reasons=()):
            return {"spec": {"kind": kind}, "qualified": qualified,
                    "reasons": list(reasons), "initial": [{}, {}]}
        with self.assertRaisesRegex(ValueError, "zero qualified ladder"):
            collect.check_collision_feasibility([])
        with self.assertRaisesRegex(ValueError, "zero qualified head-on"):
            collect.check_collision_feasibility([row("ladder")])
        good = [row("ladder"), row("head-on")]
        collect.check_collision_feasibility(good)
        with self.assertRaisesRegex(ValueError, "over half"):
            collect.check_collision_feasibility(good + [row("ladder", False, ["third_party"])] * 3)

    def test_population_ranking_and_decoded_order_tie_break(self):
        self.assertEqual(pilot.select(selection())["port"], 8768)
        data = selection()
        for row in data["candidates"]:
            row["population"] = None
        with self.assertRaisesRegex(ValueError, "no eligible"):
            pilot.select(data)

    @patch("collect.shutil.which", return_value="node")
    def test_binding_is_offline_frozen_and_dispatches_two_client_driver(self, _which):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "selection.json"
            source.write_text(json.dumps(selection()), encoding="utf-8")
            study = pilot.prepare(source, root / "study")
            plan = evidence.read_plan(study)
            template = evidence.read_json(pilot.TEMPLATE)
            self.assertEqual({k: v for k, v in plan.items() if k != "capture"}, template)
            self.assertEqual(plan["capture"]["origin"], "http://slither.io")
            bare = pilot.prepare(source, root / "bare", origin=None)
            self.assertNotIn("origin", evidence.read_plan(bare)["capture"])
            self.assertEqual((study / "pilot-template.json").read_bytes(), pilot.TEMPLATE.read_bytes())
            self.assertEqual((study / "attempts.jsonl").read_bytes(), b"")
            command = collect.capture_command(study, root / "unused")
            self.assertEqual(Path(command[1]).name, "slither-paired-collision.mjs")
            self.assertEqual(command[command.index("--allow-outside") + 1], "yes")
            self.assertEqual(command[command.index("--origin") + 1], "http://slither.io")
            self.assertNotIn("--mode", command)
            self.assertNotIn("--bots", command)
            plan["capture"]["bots"] = 3
            with self.assertRaises(ValueError):
                evidence.validate(plan, "study-plan")
