import json
import unittest
import analyze_second_continuation as c


class Tests(unittest.TestCase):
    def test_original_questions_metrics_stall_gauge_gates_and_split(self):
        plan=c.read_plan()
        original=json.loads((c.HERE/'plan.json').read_text(encoding='utf-8'))
        for key in ('questions','network','scheduler','telemetry','batches'):
            self.assertEqual(plan[key],original[key])


if __name__=='__main__': unittest.main()
