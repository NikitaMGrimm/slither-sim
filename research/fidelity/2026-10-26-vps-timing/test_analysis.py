"""Synthetic offline contracts; never connect to a game server."""
import unittest
from unittest.mock import patch
import numpy as np
import analyze as a


def telemetry(start, end, late=0, writes=0):
    return dict(since=start, until=end, timerLateMaxMs=late, loop={'maxMs':1.2},
                receiveMaxMs=0, gc=[], backpressureWrites=writes)


class Tests(unittest.TestCase):
    def test_linux_mask_and_unknown_edges(self):
        clean = a.Clean([telemetry(0,1000),telemetry(1000,2000,6),telemetry(2000,3000)])
        self.assertTrue(clean.interval(100,800))
        self.assertFalse(clean.interval(800,2200))
        self.assertTrue(clean.interval(2200,2800))
        self.assertFalse(clean.interval(-1,100))
        self.assertFalse(clean.interval(2999,3001))

    def test_backpressure_is_incremental(self):
        clean = a.Clean([telemetry(0,1000,writes=1),telemetry(1000,2000,writes=1)])
        self.assertFalse(clean.interval(500,500))
        self.assertTrue(clean.interval(1500,1500))

    def test_floor_matches_brute_force(self):
        from timing import rolling_min
        t = np.array([0,4,11,18,22,42.])
        x = np.array([8,5,7,3,9,4.])
        expected = [min(x[abs(t-ti)<=10]) for ti in t]
        np.testing.assert_equal(rolling_min(t,x,10),expected)

    def test_known_clock_slope(self):
        t = np.arange(0,60000,7.)
        s = t - 500 - .0001*t
        slope,linear,excess = a.baseline(t,s)
        self.assertAlmostEqual(slope,.0001,places=10)
        self.assertLess(max(excess),1e-8)

    def test_tail_singleton_and_censoring(self):
        t=np.arange(5)*7.; e=np.array([0,15,0,30,0.])
        r=a.tails(t,e,np.ones(5,bool),10)
        self.assertEqual(r['episodes'],2)
        self.assertEqual(r['durationsMs']['maximum'],0)
        self.assertEqual(r['bracketsMs']['p50'],14)
        r=a.tails(t,e,np.array([False,True,True,True,True]),10)
        self.assertEqual(r['censored'],1)

    def test_scheduler_uses_server_clock_and_resets_appearances(self):
        class World:
            self_id=1
            def __init__(self,**kwargs): self.events=[]
            def feed(self,p,s):
                event={b'e':('rot',s,1,(),()),b'h':('fam',s,1,0),b's':('spawn',s,1,None)}[p]
                self.events.append(event)
        frames=[a.sp.Frame(1,100,0,[b'e',b'h']),a.sp.Frame(1,101,42,[b'e',b'h']),
                a.sp.Frame(1,200,70,[b'e']),a.sp.Frame(1,205,75,[b's',b'e'])]
        with patch.object(a.sp,'World',World):
            r=a.scheduler(frames,a.Clean([telemetry(0,1000)]))
        self.assertEqual(r['gaps']['motion']['clean']['serverMs']['n'],2)
        self.assertEqual(r['gaps']['motion']['clean']['lt35'],1)
        self.assertEqual(r['gaps']['mass']['clean']['lt100'],1)
        self.assertEqual(r['ordering']['motionBeforeMassClean'],2)

    def test_model_is_ordered_and_deterministic(self):
        from timing import simulate
        s=np.arange(0,60000,7.)
        params=[1.901395726025303,.506615071647472,.17706847130783426,1.8374055509510452,1.6462607728944314,13.9335646104394]
        t,e=simulate(s,params,26)
        np.testing.assert_equal(t,simulate(s,params,26)[0])
        self.assertTrue(np.all(np.diff(t)>=0))
        self.assertTrue(np.all(e>=0))


if __name__ == '__main__': unittest.main()
