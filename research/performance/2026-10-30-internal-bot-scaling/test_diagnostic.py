import unittest
import contextlib
import io
import json
from pathlib import Path
import tempfile
from classify_diagnostic import operation_phase, header, world_phase, classify


class StackClassification(unittest.TestCase):
    def test_demangled_tick_and_nested_admission(self):
        self.assertEqual(operation_phase('slither_sim::simulator::Simulator::respawn\n'
            'internal_scaling_debug::Scene::admission\ninternal_scaling_debug::Scene::tick'),'admission')

    def test_demangled_observation_and_policy(self):
        self.assertEqual(operation_phase('slither_sim::observation::Observer::visit\n'
            'internal_scaling_debug::Scene::tick'),'observation')
        self.assertEqual(operation_phase('slither_sim::bot::kernel::choose\n'
            'internal_scaling_debug::Scene::tick'),'A4')
        self.assertEqual(operation_phase('internal_scaling_debug::pickup_guard_after_step\n'
            'internal_scaling_debug::Scene::tick'),'guard audit outside world brackets')

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

    def test_observation_bucket_cannot_replace_window_label(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);(root/'src/observation').mkdir(parents=True);(root/'diagnostic').mkdir()
            (root/'src/observation/mod.rs').write_text('\n'.join([
                'for row in grid.row(b.min.y)','// Ascending bit order restores',
                'for (word, bits) in marks.iter_mut()','for (i, p) in w.prey.iter()',
                'let visible_opponents =','for i in visible_opponents {']))
            (root/'src/simulator.rs').write_text('\n'.join(['fn advance<const PROFILE: bool>',
                'profile.movement_seconds +=','profile.collision_seconds +=','profile.death_seconds +=',
                'let dense = self.config.food_index','// Eating changes only mass, which no pickup test reads',
                'profile.food_seconds +=','profile.prey_seconds +=','profile.ambient_seconds +=']))
            labels=['1000','10000','botstorm-12000']
            (root/'diagnostic/pins.json').write_text('{}')
            (root/'diagnostic/overhead.json').write_text(json.dumps([{'label':label} for label in labels]))
            for label in labels:
                samples=[];stacks=[]
                for tick in range(6):
                    profile={key:0 for key in ['ticks','bodyPairs','segmentTests','foodPairs','pickupTests',
                        'movementSeconds','collisionSeconds','deathSeconds','foodSeconds','preySeconds','ambientSeconds']}
                    profile.update(ticks=1,movementSeconds=.0005)
                    samples.append({'tick':tick,'worldCpu':.001,'worldWall':.001,'admissionCpu':.00001,
                        'observations':1,'decisions':1,'observationCpu':.00002,'policyLoopCpu':.00001,
                        'worldStartMonotonic':tick+.01,'worldEndMonotonic':tick+.011,
                        'admissionStartMonotonic':tick,'admissionEndMonotonic':tick+.00001,'profile':profile,
                        'pickupGuard':{'path':'dense','boxCells':100,'foodLen':1000,'coverArea':25,'circles':6,
                            'foodGuardLimit':4096,'regularGeometry':True,'declineFixedCells':False,
                            'declineFoodLength':False,'declineCoverArea':False}})
                    stacks.extend([f'worker 1 {tick+.0105:.9f}: 2004008 cpu-clock:u:\n'
                        'slither_sim::simulator::Simulator::advance /fixture/src/simulator.rs:1\n'
                        'internal_scaling_debug::Scene::tick\n\n',
                        f'worker 1 {tick+.005:.9f}: 2004008 cpu-clock:u:\n'
                        'slither_sim::observation::Observer::new\ninternal_scaling_debug::Scene::tick\n\n'])
                row={'ticks':6,'samples':samples,'initial':{},'final':{},'wallSeconds':1,'finalStateSha256':'same'}
                for kind in ['ordinary','sampled','phases']:
                    (root/f'diagnostic/{label}-{kind}.json').write_text(json.dumps(row))
                (root/f'diagnostic/{label}-stacks.txt').write_text(''.join(stacks))
                (root/f'diagnostic/{label}-report.txt').write_text('Total Lost Samples: 0')
            with contextlib.redirect_stdout(io.StringIO()):classify(root)
            result=json.loads((root/'diagnostic/selected.json').read_text())
            self.assertEqual([row['label'] for row in result['windows']],labels)
            self.assertEqual(result['windows'][0]['worldTickAttribution']['all world ticks']['samples'],6)


if __name__=='__main__':unittest.main()
