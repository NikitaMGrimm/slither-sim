"""Synthetic stop-gate witnesses; no game connections or raw archives required."""
import unittest

from analyze_pilot import SLOTS, failed_slot_gate


def slots(completed):
    return [{"scheduledSlot": i, "lane": lane, "direction": direction,
             "split": "calibration" if i <= 4 else "holdout",
             "status": "completed" if i <= completed else "missing"}
            for i, (lane, direction) in enumerate(SLOTS, 1)]


class PilotGateTests(unittest.TestCase):
    def test_three_completed_slots_stop_without_fabricating_other_denominators(self):
        rows = slots(3)
        rows[3]["status"] = "aborted"
        gate = failed_slot_gate(rows)
        self.assertEqual(gate["verdict"], "fail")
        self.assertEqual(gate["completedSlots"], 3)
        self.assertIsNone(gate["queryGate"]["eligible200msQueries"])
        self.assertEqual(gate["candidatesScored"], 0)
        self.assertEqual(rows[4]["status"], "missing")

    def test_four_completed_slots_cannot_be_declared_a_passing_pilot(self):
        with self.assertRaisesRegex(ValueError, "evaluate the full frozen gate"):
            failed_slot_gate(slots(4))

    def test_missing_or_relabelled_slots_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "all eight"):
            failed_slot_gate(slots(3)[:4])
        rows = slots(3)
        rows[4]["split"] = "calibration"
        with self.assertRaisesRegex(ValueError, "differs from frozen"):
            failed_slot_gate(rows)


if __name__ == "__main__":
    unittest.main()
