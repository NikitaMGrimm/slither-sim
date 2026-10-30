import unittest
from classify_diagnostic import operation_phase


class StackClassification(unittest.TestCase):
    def test_demangled_tick_and_nested_admission(self):
        self.assertEqual(operation_phase('slither_sim::simulator::Simulator::respawn\n'
            'internal_scaling_debug::Scene::admission\ninternal_scaling_debug::Scene::tick'),'admission')

    def test_demangled_observation_and_policy(self):
        self.assertEqual(operation_phase('slither_sim::observation::Observer::visit\n'
            'internal_scaling_debug::Scene::tick'),'observation')
        self.assertEqual(operation_phase('slither_sim::bot::kernel::choose\n'
            'internal_scaling_debug::Scene::tick'),'A4')

    def test_legacy_symbols_and_outside_tick(self):
        self.assertEqual(operation_phase('_ZN11slither_sim11observation8Observer5visit\n'
            '_ZN22internal_scaling_debug5Scene4tick'),'observation')
        self.assertIsNone(operation_phase('slither_sim::observation::Observer::visit\n'
            'internal_scaling_debug::Scene::prime'))


if __name__=='__main__':unittest.main()
