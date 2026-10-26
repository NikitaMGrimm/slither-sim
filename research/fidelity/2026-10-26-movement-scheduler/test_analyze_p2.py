import base64
import gzip
import json
import math
from pathlib import Path
import sys
import unittest
import tempfile
import analyze_p2 as a

SOURCE=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(SOURCE/'tools/fidelity'))
import slither_protocol as sp

def incoming(t,*packets,elapsed=10):
    payload=packets[0] if len(packets)==1 else b''.join(len(p).to_bytes(2,'big')+p for p in packets)
    return ['i',t,base64.b64encode(elapsed.to_bytes(2,'big')+payload).decode()]

def outgoing(t,byte):return ['o',t,base64.b64encode(bytes([byte])).decode()]

def setup():
    p=bytearray(30);p[0]=97;p[23]=19;p[24]=42;p[6:8]=(300).to_bytes(2,'big');return bytes(p)

def snake(sid=1):
    p=bytearray(33);p[0]=115;p[1:3]=sid.to_bytes(2,'big');p[6]=48;p[10:12]=(5000).to_bytes(2,'big');return bytes(p)

def header():return dict(type='header',schema=1,t=0)
def connect(t=1,life=1):return dict(type='meta',event='connect',t=t,life=life)
def end(t=1000):return dict(type='meta',event='capture-end',t=t,collectorTiming=dict(lateTicks=0),integrity=dict(backpressureWrites=0))

class Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='scheduler-synthetic-')
        cls.addClassCleanup(cls.temp.cleanup)
        cls.output = Path(cls.temp.name)

    def analyze(self,rows):
        path=self.output/(self.id().split('.')[-1]+'.jsonl.gz')
        path.write_bytes(gzip.compress(('\n'.join(json.dumps(r) for r in rows)+'\n').encode()))
        return a.analyze(path,sp)

    def test_nearest_rank(self):
        self.assertEqual(a.quantile([1,2,3,4],.5),2)
        self.assertIsNone(a.quantile([],.95))

    def test_wrap_distance(self):self.assertEqual(a.angular_distance(255,1),2)

    def test_malformed_motion_forms_are_not_reinterpreted(self):
        for packet in (b'5\0',b'd\0\0',b'7\0\0'):
            with self.assertRaises(ValueError):a.motion_form(packet,1)

    def test_all_motion_masks_and_direction(self):
        forms=[(b'd\1\2\3','hws',1),(b'7\1\2\3','hws',2),
            (b'e\0\2\1\2\3','hws',1),(b'E\0\2\1\2\3','hws',2),
            (b'e\1\2','hs',None),(b'E\1\2','ws',1),(b'4\1\2','ws',2),
            (b'3\1\2','hw',1),(b'5\1\2','hw',2),
            (b'e\1','h',None),(b'E\1','w',1),(b'4\1','w',2),(b'3\1','s',None)]
        for packet,mask,direction in forms:
            form=a.motion_form(packet,1)
            self.assertEqual((form['mask'],form['direction']),(mask,direction))

    def test_exact_reflection_and_hold(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),outgoing(30,100),
            incoming(50,b'E'+bytes([101]),elapsed=40),outgoing(230,150),end(300)])
        first=result['headings'][0]
        self.assertEqual(first['targetWantedByte'],101)
        self.assertTrue(first['isolatedQualified'])
        self.assertEqual(first['heldMs'],200)

    def test_wrong_quantization_does_not_match(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),outgoing(30,100),
            incoming(50,b'E'+bytes([102]),elapsed=40),outgoing(230,150),end(300)])
        self.assertFalse(result['headings'][0]['isolatedQualified'])
        self.assertEqual(result['headings'][0]['status'],'superseded-before-reflection')

    def test_short_hold_not_qualified(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),outgoing(30,100),
            incoming(50,b'E'+bytes([101]),elapsed=40),outgoing(100,150),end(200)])
        self.assertFalse(result['headings'][0]['isolatedQualified'])

    def test_no_reflection_borrowed_after_supersession(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),outgoing(30,100),
            outgoing(60,150),incoming(80,b'E'+bytes([101]),elapsed=70),end(400)])
        self.assertIsNone(result['headings'][0]['firstReflection'])
        self.assertIsNone(result['headings'][1]['firstReflection'])

    def test_pickup_pairs_follow_packet_order(self):
        food=b'F\1\1\0\2\3\x19';pickup=b'<\1\1\2\3\0\1';mass=b'h\0\1\0\0\1'
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),food,elapsed=0),
            incoming(20,mass,pickup),end(200)])
        self.assertEqual(result['coverage']['creditedPickupMassPairs'],0)
        self.assertEqual(result['coverage']['rightCensoredPickups'],1)
        result2=self.analyze_distinct([header(),connect(),incoming(10,setup(),snake(),food,elapsed=0),
            incoming(20,pickup,mass),end(200)],'pickup-order-second')
        self.assertEqual(result2['coverage']['creditedPickupMassPairs'],1)

    def analyze_distinct(self,rows,name):
        path=self.output/(name+'.jsonl.gz')
        path.write_bytes(gzip.compress(('\n'.join(json.dumps(r) for r in rows)+'\n').encode()))
        return a.analyze(path,sp)

    def test_motion_gap_resets_on_connection(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),incoming(20,b'e\1'),
            dict(type='meta',event='close',t=30),connect(40,2),incoming(50,setup(),snake(),elapsed=0),
            incoming(60,b'e\2'),end(100)])
        self.assertEqual(result['gaps'].get('motion/own',{}).get('n',0),0)

    def test_direction_change_not_unchanged_direction(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),incoming(20,b'E\0'),incoming(30,b'4\0'),end(100)])
        self.assertEqual(result['counts']['unchanged-tuple/direction-changed'],2)

    def test_directional_boost_reflections(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),outgoing(20,253),
            incoming(40,b'3\x6c'),outgoing(60,254),incoming(80,b'3\x5a'),end(100)])
        self.assertEqual(result['coverage']['confirmedDirectionalBoostReflections'],2)

    def test_growth_confounds_boost_reflection(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),outgoing(20,253),
            incoming(30,b'N\0\0\0\0\1'),incoming(40,b'3\x6c'),end(100)])
        self.assertEqual(result['coverage']['confirmedDirectionalBoostReflections'],0)
        self.assertEqual(result['coverage']['directionalSpeedResponsesIncludingConfounds'],1)

    def test_avoidance_confounds_boost_reflection(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),outgoing(20,253),
            dict(type='meta',event='movement-control',t=21,interruption='avoidance'),incoming(40,b'3\x6c'),end(100)])
        self.assertEqual(result['coverage']['confirmedDirectionalBoostReflections'],0)

    def test_tail_without_mass_does_not_complete_pickup_pair(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),b'F\1\1\0\2\3\x19',elapsed=0),
            incoming(20,b'<\1\1\2\3\0\1',b'r\0\1'),end(100)])
        self.assertEqual(result['coverage']['creditedPickupMassPairs'],0)

    def test_clock_bounds_are_interval_and_ignore_challenge_clock(self):
        result=self.analyze([header(),connect(),incoming(5,b'6x',elapsed=0),incoming(10,setup(),snake(),elapsed=0),
            outgoing(100,251),incoming(110,b'p',elapsed=100),outgoing(120,100),
            incoming(150,b'E\x65',elapsed=40),incoming(5010,b'p',elapsed=4860),end(6000)])
        bounds=result['headings'][0]['clockBounds']
        self.assertEqual((bounds['reflectionDelayLower'],bounds['reflectionDelayUpper']),(20,30))

    def test_missing_drift_fit_leaves_bounds_missing(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),outgoing(20,251),
            incoming(30,b'p'),outgoing(40,100),incoming(60,b'E\x65'),end(300)])
        self.assertIsNone(result['headings'][0]['clockBounds'])

    def test_phase_residuals_do_not_select_from_validation(self):
        self.assertEqual(a.best_phase([0,42,84],42),0)
        self.assertEqual(a.phase_residual(42,42,21),21)

    def test_missing_telemetry_and_late_intervals_masked(self):
        windows=a.telemetry_mask([header(),dict(type='meta',event='collector-timing',t=1000,lateTicks=1),end(2000)])
        self.assertTrue(any(lo<=0 and hi>=1100 for lo,hi in windows))

    def test_future_dirty_times_not_fabricated(self):
        result=self.analyze([header(),connect(),incoming(10,setup(),snake(),elapsed=0),end(100)])
        self.assertTrue(all(c['packetStateMismatch'] is None for c in result['candidates'].values()))

if __name__=='__main__':unittest.main()
