"""Synthetic checks for the study's gap denominators and reset boundaries."""
import base64
import gzip
import json
from pathlib import Path
import tempfile
import unittest

from analyze import capture


class AnalysisTests(unittest.TestCase):
    def test_burn_in_setup_and_connection_boundaries(self):
        def incoming(t, h, code=b'p'):
            return ['i',t,base64.b64encode(h.to_bytes(2,'big')+code).decode()]
        records = [
            {'type':'header'}, {'event':'connect'}, incoming(0,0,b'a'),
            incoming(4999,7), incoming(5000,7), incoming(5000.5,7),
            incoming(5020,14), incoming(6000,0,b'a'),
            incoming(11000,7), incoming(11007,7),
            {'event':'connect'}, incoming(12000,0,b'6'), incoming(12020,0,b'6'),
            incoming(12100,0,b'a'), incoming(17100,7), incoming(17107,7),
        ]
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'synthetic.jsonl.gz'
            with gzip.open(path,'wt') as f:
                for record in records:
                    f.write(json.dumps(record)+'\n')
            result=capture(path)
        self.assertEqual(result['setups'],3)
        self.assertEqual(result['receipt']['n'],4)
        self.assertEqual(result['receipt']['max'],19.5)
        self.assertEqual(result['under1'],1)
        self.assertEqual(result['over14'],1)
        self.assertEqual(result['burstPairs'],1)
        self.assertEqual(result['eligibleAdjacentPairs'],1)
        self.assertEqual(result['header']['p50'],7)


if __name__ == '__main__':
    unittest.main()
