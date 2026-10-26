import tempfile
from pathlib import Path
import unittest
import profile as study_profile

class ProfileTests(unittest.TestCase):
    def test_leaf_first_and_nonadditive_inclusive_counts(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'perf.stacks'
            path.write_text('host 42 1.00: cpu-clock:\n  123 malloc+0x1 (/lib/libc.so)\n  456 slither_sim::simulator::Simulator::step+0x2 (/study/bin/host_probe)\n\nhost 42 1.01: cpu-clock:\n  456 slither_sim::simulator::Simulator::step+0x2 (/study/bin/host_probe)\n\n')
            result=study_profile.reduce(path)
            self.assertEqual(result['samples'],2)
            self.assertEqual(result['topInclusive'][0]['percent'],100)
            self.assertEqual(result['categories']['allocation']['percent'],50)
            self.assertEqual(result['categories']['physics']['percent'],50)
    def test_empty_profile_is_not_success(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'empty';path.write_text('')
            with self.assertRaises(ValueError): study_profile.reduce(path)

    def test_period_weighting(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'perf-period.stacks'
            path.write_text('host 42/43 1.00: 900 cpu-clock:\n  123 malloc (/lib/libc.so)\n\nhost 42/43 1.01: 100 cpu-clock:\n  456 slither_sim::simulator::Simulator::step (/study/bin/host_probe)\n\n')
            result=study_profile.reduce(path)
            self.assertEqual(result['samples'],2)
            self.assertEqual(result['categories']['allocation']['percent'],90)
            filtered=study_profile.reduce(path,[1.005,2])
            self.assertEqual(filtered['samples'],1)
            self.assertEqual(filtered['categories']['physics']['percent'],100)

    def test_hardware_periods(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'perf-period.stacks'
            path.write_text('host 42/43 1.00: 900 cycles:\n  123 malloc (/lib/libc.so)\n\n')
            result=study_profile.reduce(path)
            self.assertEqual(result['event'],'cycles')
            self.assertEqual(result['sampleWeight'],'hardware cycles')

    def test_missing_callchain_stays_in_denominator(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'perf-period.stacks'
            path.write_text('host 42/43 1.00: 900 cycles:\n\nhost 42/43 1.01: 100 cycles:\n  123 malloc (/lib/libc.so)\n\n')
            result=study_profile.reduce(path)
            self.assertEqual(result['samples'],2)
            self.assertEqual(result['categories']['other/unknown']['percent'],90)

if __name__=='__main__': unittest.main()
