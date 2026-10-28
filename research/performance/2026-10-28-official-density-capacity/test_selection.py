"""Independent arithmetic checks for the research CPU/tick reporting seam."""
import copy
import unittest
import importlib.util
from pathlib import Path

spec=importlib.util.spec_from_file_location('capacity_selection',Path(__file__).with_name('select_results.py'))
selection=importlib.util.module_from_spec(spec);spec.loader.exec_module(selection)

def row(pid,user,system,role):
    return {'pid':pid,'start':'1','user':user,'system':system,'comm':role,'cpus':[0],'peakKiB':102400}

class Arithmetic(unittest.TestCase):
    def test_interpolation_uses_cumulative_cpu(self):
        samples=[{'server':{'unix':at,'process':row(10,user,system,'host'),
                            'threads':[row(10,user,system,'simulation')]}}
                 for at,user,system in [(100.,2.,1.),(101.,2.4,1.2),(102.,2.8,1.4)]]
        point,edge=selection.cpu_at(samples,100.25)
        self.assertAlmostEqual(point['process']['user'],2.1)
        self.assertAlmostEqual(point['process']['system'],1.05)
        self.assertAlmostEqual(edge['fraction'],.25)

    def test_process_and_disjoint_thread_cpu_agree(self):
        start={'unix':100.,'process':row(10,1.,.5,'host'),
               'threads':[row(10,0.,0.,'main'),row(11,.5,.2,'simulation'),row(12,.5,.3,'bots')]}
        end={'unix':110.,'process':row(10,9.,2.5,'host'),
             'threads':[row(10,0.,0.,'main'),row(11,4.5,1.2,'simulation'),row(12,4.5,1.3,'bots')]}
        result=selection.cpu_delta(start,end)
        self.assertAlmostEqual(result['corePercent'],100.)
        self.assertAlmostEqual(result['userSeconds'],8.)
        self.assertAlmostEqual(result['kernelSeconds'],2.)
        self.assertEqual(result['peakRssMiB'],100.)
        corrupt=copy.deepcopy(end)
        for thread in corrupt['threads']:
            thread['user']=9.;thread['system']=2.5
        with self.assertRaisesRegex(AssertionError,'thread totals'):
            selection.cpu_delta(start,corrupt)
        moved=copy.deepcopy(end);moved['threads'][1]['cpus']=[1]
        with self.assertRaises(AssertionError):selection.cpu_delta(start,moved)

    def test_tail_order_statistics_keep_the_extreme(self):
        values=[100.,1.,2.,3.,4.]
        self.assertEqual(selection.quantile(values,.5),3.)
        self.assertEqual(selection.quantile(values,.99),100.)
        self.assertEqual(selection.quantile(values,1.),100.)

if __name__=='__main__':unittest.main()
