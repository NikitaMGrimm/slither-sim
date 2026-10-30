import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import collect
import evidence


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

    @patch("collect.shutil.which", return_value="node")
    def test_local_collision_plan_dispatches_two_client_driver(self, _which):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plan = evidence.read_json(evidence.ROOT / "evidence/templates/movement-plan.json")
            plan["capture"].update(scenario="paired-collision", bots=2, seconds=600)
            source = root / "plan.json"
            source.write_text(json.dumps(plan), encoding="utf-8")
            study = evidence.create_study(source, root / "study")
            command = collect.capture_command(study, root / "unused")
            self.assertEqual(Path(command[1]).name, "slither-paired-collision.mjs")
            self.assertNotIn("--allow-outside", command)
            self.assertNotIn("--mode", command)
            self.assertNotIn("--bots", command)
            plan["capture"]["bots"] = 3
            with self.assertRaises(ValueError):
                evidence.validate(plan, "study-plan")
