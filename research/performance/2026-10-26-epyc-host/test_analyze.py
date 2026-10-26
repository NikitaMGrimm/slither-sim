"""Focused synthetic regression for the new wait and operator-stat reductions."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('epyc', Path(__file__).with_name('analyze.py'))
epyc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(epyc)


class ReducerTest(unittest.TestCase):
    def test_hold_waits_censoring_and_raw_stats(self):
        with tempfile.TemporaryDirectory() as tmp:
            host, load = Path(tmp)/'host', Path(tmp)/'load'
            host.mkdir(); load.mkdir()
            (host/'telemetry.json').write_text(json.dumps({'rows':[{'processes':{'1':{'hwmBytes':42}}}]}))
            (host/'host.json').write_text(json.dumps({'holdStartUnix':10,'holdEndUnix':30}))
            lines = [{'unix':at,'line':f'stats: ticks/s=142.86 deferred_ticks=0 simulation_load={v}% players=8 bot_slots=0'}
                     for at,v in [(5,90),(10,80),(15,10),(20,20),(25,30),(30,40),(35,90)]]
            (host/'stats-lines.json').write_text(json.dumps(lines))
            (load/'load-0.json').write_text(json.dumps({
                'spawnWaits':[{'inHold':False,'respawn':False,'waitMs':9999},
                              {'inHold':True,'respawn':False,'waitMs':500},
                              {'inHold':True,'respawn':True,'waitMs':100}],
                'pendingRequestsAtEnd':[{'ageMs':3000}],
                'clients':{'final':{'playing':7,'login':1}}, 'closeReasons':{}}))
            base = {'hostResources':{'cpuPercentOneCore':50,'windowSeconds':20},'executedTicks':2000}
            with patch.object(epyc,'reduce_trial',return_value=base):
                r = epyc.reduce(host,load,8)
            self.assertEqual(r['cpuMsPerExecutedTick'],5)
            self.assertEqual(r['peakRssBytes'],42)
            self.assertEqual(r['spawnWaitMs']['all']['count'],2)
            self.assertEqual(r['spawnWaitMs']['all']['p95'],500)
            self.assertEqual(r['spawnWaitMs']['respawn']['p50'],100)
            self.assertEqual(r['initialSpawnWaitMsIncludingWarmup']['count'],2)
            self.assertEqual(r['initialSpawnWaitMsIncludingWarmup']['max'],9999)
            self.assertEqual(r['pendingRequestsAtEnd'],1)
            self.assertEqual(r['pendingAgeMs']['max'],3000)
            self.assertEqual(r['operatorStats']['completeIntervals'],4)
            self.assertEqual(r['operatorStats']['meanSimulationLoadPercent'],25)


if __name__ == '__main__': unittest.main()
