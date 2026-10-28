import importlib.util
import math
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('wall_analysis',Path(__file__).with_name('analyze.py'))
a=importlib.util.module_from_spec(spec);spec.loader.exec_module(a)


class WallAnalysisTests(unittest.TestCase):
    def test_interior_rules_agree_and_hard_boundary_preserves_lethal_pose(self):
        state=(.4,5.79,7,8000.)
        for model in a.MODELS:
            p,dead=a.step((100.,0.),state,.007,model)
            self.assertFalse(dead)
            self.assertAlmostEqual(p[0],100+5.79/.032*.007*math.cos(.4))
        p,dead=a.step((7999.,0.),(0.,14.,2,8000.),.007,'H')
        self.assertTrue(dead);self.assertGreater(p[0],8000)

    def test_shallow_projection_and_damping_are_distinct(self):
        h=math.pi/2-.6
        state=(h,5.79,7,8000.)
        p=(7990.,0.)
        current,dead=a.step(p,state,.007,'P16')
        damped,dead_d=a.step(p,state,.007,'D90')
        self.assertFalse(dead or dead_d)
        self.assertLess(damped[1],current[1])
        tip=(current[0]+16*a.scale(7)*math.cos(h),current[1]+16*a.scale(7)*math.sin(h))
        self.assertAlmostEqual(math.hypot(*tip),8000.,places=9)

    def test_packet_order_and_event_shift_do_not_reinitialize_position(self):
        start=dict(server=0,order=1,xy=(100.,0.))
        end=dict(server=100,order=3)
        states=[(0,1,0.,3.2,2,8000.,0),(50,2,math.pi/2,3.2,2,8000.,50)]
        p=a.replay(start,end,states,'P16')['xy']
        self.assertAlmostEqual(p[0],105);self.assertAlmostEqual(p[1],5)
        p=a.replay(start,end,states,'P16',shift=.025)['xy']
        self.assertAlmostEqual(p[0],107.5);self.assertAlmostEqual(p[1],2.5)

    def test_count_event_is_not_hidden_by_matching_endpoints(self):
        row=dict(server=0,order=1,gap=10,alpha=.6,speed=5.79,nearest=None,direction=1,count=7)
        end=dict(row,server=300,order=4)
        self.assertFalse(a.eligible(row,end,[(100,2,1.,5.79,8,8000.,0),(200,3,1.,5.79,7,8000.,0)]))

    def test_empty_exposure_is_null_and_lethal_rows_are_separate(self):
        result=a.aggregate([])
        self.assertEqual(result['intervals'],0)
        self.assertIsNone(result['models']['P16']['aliveEndpoint']['median'])

    def test_power_does_not_average_systematic_uncertainty(self):
        result=a.power()
        self.assertEqual(result['designs'],1176)
        self.assertLess(result['P15MaximumCeilingOverBudget'],1)
        self.assertLess(result['T16MaximumCeilingOverBudget'],1)
        self.assertEqual(result['D90StalePassingDesigns'],0)


if __name__=='__main__':unittest.main()
