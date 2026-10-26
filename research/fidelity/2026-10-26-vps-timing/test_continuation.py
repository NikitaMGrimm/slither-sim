import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import analyze as a
import analyze_continuation as c
import witnesses


class Tests(unittest.TestCase):
    def test_questions_metrics_stall_gauge_and_gates_are_unchanged(self):
        plan=c.read_plan()
        original=json.loads((c.HERE/'plan.json').read_text())
        for key in ('questions','network','scheduler','telemetry'):
            self.assertEqual(plan[key],original[key])
        self.assertEqual(plan['batches'],original['batches'])

    def test_validation_seal_detects_changed_development(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); (root/'development.json').write_text('{}')
            (root/'analysis-seal.json').write_text(json.dumps(dict(developmentSha256=a.digest(root/'development.json'),sourcePins=[])))
            c.check_seal(root,[])
            (root/'development.json').write_text('{"changed":true}')
            with self.assertRaisesRegex(ValueError,'changed'): c.check_seal(root,[])

    def test_receipt_compression_is_separate_from_server_exception(self):
        class World:
            def __init__(self,**kw): self.events=[]
            def feed(self,p,s): self.events.append(('rot',s,1,(),()))
        rows=[a.sp.Frame(1,100,0,[b'a']),a.sp.Frame(1,101,42,[b'e']),a.sp.Frame(1,120,70,[b'e'])]
        # The test only isolates the clock comparison; the real protocol tracker is used in collection analysis.
        with patch.object(a.sp,'World',World),patch.object(a.sp,'read',return_value=iter(rows)),patch.object(a.Clean,'interval',return_value=True):
            result=witnesses.collect(None)
        self.assertEqual(result['counts']['motion-clean']['receiptOnlyBelowThreshold'],1)
        self.assertEqual(result['counts']['motion-clean']['serverBelowThreshold'],1)


if __name__=='__main__': unittest.main()
