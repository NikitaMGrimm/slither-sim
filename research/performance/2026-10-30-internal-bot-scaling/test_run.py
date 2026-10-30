import importlib.util
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('scaling_run',Path(__file__).with_name('run.py'))
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def trajectory(food=lambda tick:10000, length=lambda tick:60, living=1000):
    return [{'kind':'warm','tick':tick,'census':{'meanLength':length(tick),
             'p90Length':150,'world':{'food':food(tick),'living':living}}}
            for tick in range(1000,46000,1000)]


class Stability(unittest.TestCase):
    def test_requires_minimum_warmup_and_complete_bins(self):
        self.assertIsNone(module.stable(trajectory()[:20],1000))
        self.assertIsNone(module.stable(trajectory()[-3:],1000))

    def test_stationary_census_passes(self):
        self.assertIsNotNone(module.stable(trajectory(),1000))

    def test_drifting_food_and_lengths_do_not_pass(self):
        self.assertIsNone(module.stable(trajectory(food=lambda t:10000+t),1000))
        self.assertIsNone(module.stable(trajectory(length=lambda t:10+t*.01),1000))

    def test_population_depletion_does_not_pass(self):
        self.assertIsNone(module.stable(trajectory(living=980),1000))


if __name__=='__main__':unittest.main()
