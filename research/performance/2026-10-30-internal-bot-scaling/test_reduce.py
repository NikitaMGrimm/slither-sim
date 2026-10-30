import importlib.util
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('scaling_reduce',Path(__file__).with_name('reduce.py'))
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CpuAttribution(unittest.TestCase):
    def row(self, cpu, wall, phase):
        fields=['movementSeconds','collisionSeconds','deathSeconds','foodSeconds','preySeconds','ambientSeconds']
        profile={key:phase if key=='foodSeconds' else 0 for key in fields}
        profile.update(ticks=1,bodyPairs=2,segmentTests=3,foodPairs=4,pickupTests=5)
        return {'samples':[{'worldCpu':cpu,'worldWall':wall,'profile':profile}]}

    def test_on_cpu_phase_has_point_bound(self):
        row=self.row(1,1,.4)
        out=module.phase_summary(row,[row])['phases']['foodSeconds']
        self.assertEqual(out['cpuMsPerTickBounds'],[400,400])

    def test_preemption_is_not_attributed_to_cpu(self):
        row=self.row(.6,1,.4)
        out=module.phase_summary(row,[row])['phases']['foodSeconds']
        self.assertEqual(out['cpuMsPerTickBounds'],[0,400])

    def test_bounds_include_all_possible_preemption_placements(self):
        row=self.row(.8,1,.4)
        lower,upper=module.phase_summary(row,[row])['phases']['foodSeconds']['cpuMsPerTickBounds']
        self.assertAlmostEqual(lower,200)
        self.assertAlmostEqual(upper,400)


if __name__=='__main__':unittest.main()
