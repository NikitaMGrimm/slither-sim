import unittest
from classify_diagnostic import operation_phase, header, world_phase


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

    def test_header_uses_perf_monotonic_timestamp_and_period(self):
        self.assertEqual(header('internal_scalin 123 1245.123456789: 2004008 cpu-clock:u:\n'),
                         (1245.123456789,2004008))
        self.assertIsNone(header('PERF_RECORD_LOST'))

    def test_world_source_context_and_admission(self):
        ranges={'movement':(100,200),'food pickup/cache':(300,400),'pickup grid construction':(300,320)}
        self.assertEqual(world_phase('/work/src/simulator.rs:305','world',ranges),'pickup grid construction')
        self.assertEqual(world_phase('/work/src/simulator.rs:340','world',ranges),'food pickup/cache')
        self.assertEqual(world_phase('unknown','admission',ranges),'spawn/admission')
        self.assertEqual(world_phase('unknown','world',ranges),'unresolved world / clock brackets')


if __name__=='__main__':unittest.main()
