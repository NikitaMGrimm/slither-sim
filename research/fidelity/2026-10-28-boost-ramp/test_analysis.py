"""Synthetic arithmetic, selection, lifecycle and uncertainty regressions."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

spec = importlib.util.spec_from_file_location("boost_analysis", Path(__file__).with_name("analyze.py"))
a = importlib.util.module_from_spec(spec)
spec.loader.exec_module(a)


def row(n=37, up=True, events=None):
    points = [[i*42, 8 + (1 if up else -1)*i*.5, n, i+1] for i in range(8)]
    return dict(source="synthetic", conn=1, sid=1, generation=1, up=up, points=points, events=events or [])


class AnalysisTests(unittest.TestCase):
    def test_units_and_ratio(self):
        self.assertAlmostEqual(a.rate(2, True), 15.39375)
        self.assertAlmostEqual(a.rate(2, False)/a.rate(2, True), -4/3)
        self.assertEqual(a.base(532), a.base(1000))

    def test_fixed_size_identity(self):
        for n in (2,37,343,500):
            for up in (True,False):
                r = row(n, up)
                for p, v in zip(a.trajectory(r,"P305"), a.trajectory(r,"V305")):
                    self.assertAlmostEqual(p,v, places=12)

    def test_growth_retains_progress_and_direct_continuity(self):
        r = row(events=[[100,137,4]])
        p, v = a.trajectory(r,"P305"), a.trajectory(r,"V305")
        self.assertGreater(p[3],v[3])
        self.assertGreater(p[-1],v[-1])

    def test_single_growth_is_sub_byte(self):
        r = row(events=[[100,38,4]])
        self.assertLess(max(abs(p-v) for p,v in zip(a.trajectory(r,"P305"),a.trajectory(r,"V305"))),1/18)

    def test_fall_ratio_candidate(self):
        r = row(up=False)
        self.assertGreater(a.trajectory(r,"P305_equal")[1], a.trajectory(r,"P305")[1])

    def test_delay_changes_phase_without_reinitializing(self):
        r = row(events=[[100,100,4]])
        self.assertNotEqual(a.trajectory(r,"P305",7),a.trajectory(r,"P305"))

    def test_trajectory_count_before_same_timestamp_report(self):
        for model in a.MODELS[:-1]:
            for up in (True, False):
                for nn in (36, 38):
                    with self.subTest(model=model, up=up, count=nn):
                        r = dict(up=up, points=[[0,8,37,1],[42,8,nn,4],[84,8,nn,6]],
                                 events=[[42,nn,3]])
                        q = (8-a.base(37))/(14-a.base(37)) + a.rate(37,up,model)/(14-a.base(37))*.042
                        expected = 8+a.rate(37,up,model)*.042 if model == "V305" else a.base(nn)+(14-a.base(nn))*q
                        predicted = a.trajectory(r,model)
                        self.assertAlmostEqual(predicted[1],expected, places=12)
                        self.assertAlmostEqual(predicted[2],expected+a.rate(nn,up,model)*.042, places=12)

    def test_trajectory_count_after_same_timestamp_report(self):
        for model in a.MODELS[:-1]:
            for up in (True, False):
                for nn in (36, 38):
                    with self.subTest(model=model, up=up, count=nn):
                        r = dict(up=up, points=[[0,8,37,1],[42,8,37,4],[42,8,nn,6],[84,8,nn,8]],
                                 events=[[42,nn,5]])
                        old = 8+a.rate(37,up,model)*.042
                        q = (old-a.base(37))/(14-a.base(37))
                        new = old if model == "V305" else a.base(nn)+(14-a.base(nn))*q
                        predicted = a.trajectory(r,model)
                        self.assertAlmostEqual(predicted[1],old, places=12)
                        self.assertAlmostEqual(predicted[2],new, places=12)
                        self.assertAlmostEqual(predicted[3],new+a.rate(nn,up,model)*.042, places=12)

    def test_trajectory_shifted_count_tie_keeps_packet_ordinal(self):
        for ordinal, count in ((3,38),(5,37)):
            with self.subTest(ordinal=ordinal):
                r = dict(up=True, points=[[0,8,37,1],[42,8,count,4],[84,8,38,6]],
                         events=[[35,38,ordinal]])
                q = (8-a.base(37))/(14-a.base(37)) + a.rate(37,True)/(14-a.base(37))*.042
                self.assertAlmostEqual(a.trajectory(r,"P305",7)[1],a.base(count)+(14-a.base(count))*q, places=12)

    def test_trajectory_zero_elapsed_count_preserves_idle_and_full_endpoints(self):
        for up in (True,False):
            for initial in (a.base(37)-.1,14):
                r = dict(up=up, points=[[0,initial,37,1],[0,initial,38,3]], events=[[0,38,2]])
                for model in a.MODELS[:-1]:
                    with self.subTest(up=up,initial=initial,model=model):
                        self.assertEqual(a.trajectory(r,model)[1],initial)

    def test_selector_trims_both_endpoints(self):
        selected = list(a.runs(row()["points"]))
        self.assertEqual(len(selected),1)
        self.assertEqual(selected[0][1],row()["points"][1:-1])
        self.assertEqual(list(a.runs(row()["points"][:5])),[])

    def test_selector_breaks_gaps_equal_values_and_reversals(self):
        p = [[i*42,float(i)] for i in range(7)]
        q = [[400+i*42,float(7-i)] for i in range(7)]
        self.assertEqual(len(list(a.runs(p+q))),2)
        p[3][1] = p[2][1]
        self.assertEqual(list(a.runs(p)),[])

    def test_ols_known_slope_and_quantization_bound(self):
        points = [[i*42,2+i*.42] for i in range(8)]
        f = a.fit(points)
        self.assertAlmostEqual(f["slope"],10)
        changed = [[t,v+(1/36 if i<4 else -1/36)] for i,(t,v) in enumerate(points)]
        self.assertLessEqual(abs(a.fit(changed)["slope"]-10),f["weight_l1"]/36+1e-12)

    def test_fit_rejects_nonfinite_and_duplicate_clock(self):
        for points in ([[0,1]]*4, [[i,float("nan")] for i in range(4)]):
            with self.assertRaises(ValueError):
                a.fit(points)

    def test_event_guard_can_leave_no_exposure(self):
        r = row(events=[[100,38,4],[200,39,6]])
        scored = a.cross_score(r)
        self.assertLess(scored["models"]["P305"]["gates"]["42"]["points"],len(r["points"])-1)

    def test_wrong_archive_hash_fails_before_read(self):
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp)/"wrong.gz"
            p.write_bytes(b"not a capture")
            with patch.object(a.sp,"read",side_effect=AssertionError("must not read")):
                with self.assertRaisesRegex(ValueError,"hash mismatch"):
                    a.extract(p,next(iter(a.PINS)))

    def test_extractor_preserves_endpoint_event_and_life_boundaries(self):
        class World:
            def __init__(self, **kwargs):
                self.snakes, self.events = {}, []

            def feed(self, packet, time):
                code = packet[:1]
                if code == b"s":
                    self.snakes[1] = SimpleNamespace(sct=37, sp=6.)
                    self.events.append(("spawn",time,1))
                elif code == b"x":
                    self.events.append(("remove",time,1))
                    del self.snakes[1]
                elif code == b"n":
                    self.snakes[1].sct += 1
                    self.events.append(("anchor",time,1))
                elif code == b"v":
                    snake=self.snakes[1]
                    before=snake.sp
                    snake.sp=packet[1]/18
                    self.events.append(("rot",time,1,(0,0,before),(0,0,snake.sp)))

        setup=bytearray(25)
        setup[0]=ord("a")
        setup[23]=19
        for index,value in ((11,539),(13,40),(15,1400)):
            setup[index:index+2]=value.to_bytes(2,"big")
        frames=[a.sp.Frame(1,0,0,[bytes(setup),b"s"])]
        for generation in range(2):
            offset=generation*1000
            if generation:
                frames.append(a.sp.Frame(1,offset,offset,[b"x",b"s"]))
            for i in range(8):
                packets=[b"v"+bytes([120+i*8])]
                if i in (1,3):
                    packets.append(b"n")
                frames.append(a.sp.Frame(1,offset+i*42,offset+i*42,packets))
        frames.append({"event":"capture-end"})
        source=next(iter(a.PINS))
        with patch.object(a,"sha",return_value=a.PINS[source]), patch.object(a.sp,"read",return_value=iter(frames)), patch.object(a.sp,"World",World):
            _,cross,_=a.extract(Path("synthetic.gz"),source)
        self.assertEqual(len(cross),2)
        self.assertEqual([r["generation"] for r in cross],[1,2])
        for r in cross:
            self.assertEqual(len(r["events"]),2)
            self.assertEqual(r["events"][0][0],r["points"][0][0])
            self.assertEqual(r["points"][0][2],37)
            self.assertEqual(r["events"][0][1],38)

    def test_output_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/"result.json"
            a.dump(p,{"a":1})
            with self.assertRaises(FileExistsError):
                a.dump(p,{"a":2})

    def test_size_phase_is_below_one_byte(self):
        p=a.power()
        self.assertLess(p["phase_one_event_bound"],p["speed_byte"])
        self.assertTrue(all(r["monotone_growth_refresh_bound"]<2/18 for r in p["fixed"]))

    def test_cycles_do_not_average_away_input_uncertainty(self):
        p=a.power_cycle(37,.2,.4,seconds=3)
        self.assertGreater(p["switches"],1)
        self.assertGreater(p["input_timing_budget"]["14"],p["input_timing_budget"]["7"])
        self.assertFalse(p["separates"]["7"])


if __name__ == "__main__":
    unittest.main()
