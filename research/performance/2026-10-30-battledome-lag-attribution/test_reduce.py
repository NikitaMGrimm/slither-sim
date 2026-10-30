"""Synthetic scheduling/accounting regressions, run remotely only."""
from pathlib import Path
import struct
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import reduce


class ReductionTests(unittest.TestCase):
    def test_interval_union_and_boundary_intersections(self):
        value=reduce.Intervals([(0,5),(3,7),(10,15),(15,20)])
        self.assertEqual(value.values,[[0,7],[10,20]])
        for a,b,expected in [(0,20,17),(2,12,7),(7,10,0),(5,15,7),(20,21,0),(-1,2,2)]:
            self.assertEqual(value.overlap(a,b),expected)

    def test_episode_recovery_and_censoring(self):
        ticks=[{"lag":x*reduce.MS} for x in [0]*5+[30,40,3,4,3,2,1,30,0,0]]
        rows=reduce.episodes(ticks)
        self.assertEqual(len(rows),2)
        self.assertFalse(rows[0]["leftCensored"])
        self.assertFalse(rows[0]["rightCensored"])
        self.assertEqual(rows[0]["peak"],6)
        self.assertTrue(rows[1]["rightCensored"])
        self.assertTrue(reduce.episodes([{"lag":30*reduce.MS}])[0]["leftCensored"])

    def test_trace_socket_wait_irq_and_wakeup_dispatch(self):
        text=""" sim 100 [000] 1.000000000: sched:sched_switch: idle:0 [120] R ==> sim:100 [120]
 sim 100 [000] 1.001000000: sched:sched_switch: sim:100 [120] R ==> socket:200 [120]
 socket 200 [000] 1.001500000: irq:irq_handler_entry: irq=1 name=timer
 socket 200 [000] 1.001700000: irq:irq_handler_exit: irq=1 ret=handled
 socket 200 [000] 1.003000000: sched:sched_switch: socket:200 [120] S ==> sim:100 [120]
 sim 100 [000] 1.003500000: sched:sched_switch: sim:100 [120] S ==> idle:0 [120]
 idle 0 [000] 1.004600000: sched:sched_wakeup: sim:100 [120] CPU:000
 idle 0 [000] 1.004900000: sched:sched_switch: idle:0 [120] R ==> sim:100 [120]
 sim 100 [000] 1.005000000: sched:sched_switch: sim:100 [120] S ==> idle:0 [120]
"""
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/"trace.data";path.touch()
            def run(*args,**kwargs):kwargs["stdout"].write(text);return SimpleNamespace(returncode=0)
            with patch("reduce.subprocess.run",side_effect=run):value=reduce.Trace(path,100,200)
            self.assertTrue(value.usable)
            parts=value.wait_parts(1_000_000_000,1_005_000_000)
            self.assertEqual(parts["socket"],1_800_000)
            self.assertEqual(parts["irq"],200_000)
            self.assertEqual(parts["idleOrSteal"],300_000)
            self.assertEqual(value.wakes,[1_004_600_000])
            self.assertIn(1_004_900_000,value.dispatch)
            before=[0]*16;after=[0]*16
            before[2]=1_003_490_000;before[3]=1_004_500_000;after[2]=1_004_950_000
            wake=reduce.wake_details(before,after,value)
            self.assertAlmostEqual(wake["readyDelayMs"],.1)
            self.assertAlmostEqual(wake["dispatchDelayMs"],.3)
            self.assertAlmostEqual(wake["overshootMs"],.45)

    def test_trace_loss_cannot_be_qualified(self):
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/"trace.data";path.touch()
            def run(*args,**kwargs):kwargs["stderr"].write("lost 5 events");return SimpleNamespace(returncode=0)
            with patch("reduce.subprocess.run",side_effect=run):value=reduce.Trace(path,1,2)
            self.assertFalse(value.usable)

    def test_tick_cpu_wait_and_truncated_record(self):
        before=[0,1,100_000_000,90_000_000,1,0,50_000_000,50_000_000,20_000_000,30_000_000,0,1,1,100,200,0]
        after=[1,2,110_000_000,90_000_000,1,0,54_000_000,54_000_000,26_000_000,36_000_000,0,2,2,100,200,0]
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/"ticks.bin";path.write_bytes(struct.pack("<16Q",*before)+struct.pack("<16Q",*after))
            ticks,_,_=reduce.read_ticks(path,0,200_000_000)
            self.assertEqual(ticks[0]["cpu"],4_000_000)
            self.assertEqual(ticks[0]["wait"],6_000_000)
            self.assertEqual(ticks[0]["socket"],6_000_000)
            path.write_bytes(path.read_bytes()+b"x")
            with self.assertRaises(AssertionError):reduce.read_ticks(path,0,200_000_000)

    def test_frame_phase_bunching_and_sparse_residual(self):
        records=[{"type":"frame","life":1,"playing":True,"wasPlaying":True,
                  "unix":x,"at":x,"headerMs":h} for x,h in [(1.,0),(1.098,98),(1.0985,105),(1.13,112)]]
        episodes=[{"startUnix":1.09,"peakUnix":1.10,"endUnix":1.12}]
        result=reduce.phase_frames(records,episodes,{"startUnix":1.,"endUnix":2.})
        self.assertEqual(result["growth"]["bunchedUnder1Ms"],1)
        self.assertEqual(result["growth"]["residualStalls"]["20"],0)
        self.assertEqual(result["control"]["residualStalls"]["20"],1)

    def test_small_catchup_requires_no_pacing_wait(self):
        def tick(a,b):
            first=[0]*16;last=[0]*16;first[2]=a;last[2]=b
            return {"a":first,"b":last}
        ticks=[tick(1_000_000_000,1_007_500_000),tick(1_007_600_000,1_010_100_000)]
        records=[{"type":"frame","life":1,"playing":True,"wasPlaying":True,
                  "unix":x,"at":x,"headerMs":h} for x,h in [(1.,0),(1.0077,7),(1.0103,14),(1.05,56)]]
        result=reduce.catchup_diagnostics(records,ticks,[],0,{"startUnix":1.,"endUnix":2.})
        self.assertEqual(result["pairs"],1)
        self.assertAlmostEqual(result["completionSpacingMs"]["p50"],2.6)
        self.assertEqual(result["adjacentFrames"]["gapMs"]["n"],2)
        sleep=[0]*16;sleep[2]=1_004_000_000
        result=reduce.catchup_diagnostics(records,ticks,[(sleep,sleep)],0,{"startUnix":1.,"endUnix":2.})
        self.assertEqual(result["pairs"],0)
        self.assertEqual(result["adjacentFrames"]["gapMs"]["n"],0)


if __name__=="__main__":unittest.main()
