"""Synthetic controller failures; no simulator, network or measurement work."""
import json
from pathlib import Path
import subprocess
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import count_censored
import finish_collection
import run
import run_botstorm


class ArchiveGate(unittest.TestCase):
    def test_each_failed_check_retains_status_and_prevents_archive(self):
        for failed in range(3):
            with self.subTest(failed=failed), tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp)/'execution';root.mkdir()
                study=Path(tmp)/'tools';study.mkdir()
                (root/'after-primary.pid').write_text('999999999')
                (root/'after-primary.log').write_text('{"event":"post-count-complete"}\n')
                codes=[int(i==failed) for i in range(3)]
                with patch.object(finish_collection.os,'sched_setaffinity',create=True), \
                     patch.object(finish_collection.subprocess,'run',side_effect=[SimpleNamespace(returncode=c) for c in codes]), \
                     patch.object(finish_collection,'emit'), \
                     patch.object(finish_collection.tarfile,'open') as archive:
                    with self.assertRaisesRegex(RuntimeError,'do not archive'):
                        finish_collection.execute(root,study,Path(tmp)/'source.tar')
                self.assertEqual([c['exit'] for c in json.loads((root/'reduction-status.json').read_text())],codes)
                self.assertEqual(len(list(root.glob('*.log'))),4)
                archive.assert_not_called()
                self.assertFalse((root/'archive-ready.json').exists())
                self.assertFalse((root/'archive-manifest.json').exists())


class WarmupDeadline(unittest.TestCase):
    def test_silent_stalled_child_is_killed_and_cannot_qualify(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'botstorm-build.json').write_text('{"binarySha256":"pin"}')
            killed=threading.Event();child=Mock()
            def silent_stderr():
                if not killed.wait(2):raise RuntimeError('Watchdog did not unblock stderr')
                yield from ()
            child.stderr=silent_stderr();child.kill.side_effect=killed.set
            child.wait.return_value=-9;child.poll.return_value=-9
            real_timer=threading.Timer;timers=[]
            def quick_timer(seconds,callback):
                self.assertEqual(seconds,7800)
                timer=real_timer(.01,callback);timers.append(timer);return timer
            with patch.object(run_botstorm,'digest',return_value='pin'), \
                 patch.object(run_botstorm.subprocess,'Popen',return_value=child), \
                 patch.object(run_botstorm.threading,'Timer',side_effect=quick_timer), \
                 patch.object(run_botstorm,'emit'):
                with self.assertRaisesRegex(AssertionError,'warmup failed'):
                    run_botstorm.execute(root)
            child.kill.assert_called_once()
            self.assertFalse(timers[0].is_alive())
            self.assertFalse((root/'botstorm-results/12000/qualification.json').exists())

    def test_successful_warmup_cancels_watchdog_before_counting(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'botstorm-build.json').write_text('{"binarySha256":"pin"}')
            child=Mock(stderr=iter(()));child.wait.return_value=0;child.poll.return_value=0
            with patch.object(run_botstorm,'digest',return_value='pin'), \
                 patch.object(run_botstorm.subprocess,'Popen',return_value=child), \
                 patch.object(run_botstorm.threading,'Timer') as timer, \
                 patch.object(run_botstorm.subprocess,'run',side_effect=RuntimeError('stop before counting')), \
                 patch.object(run_botstorm,'emit'):
                with self.assertRaisesRegex(RuntimeError,'stop before counting'):
                    run_botstorm.execute(root)
            timer.return_value.start.assert_called_once()
            timer.return_value.cancel.assert_called_once()
            timer.return_value.join.assert_called_once()
            child.kill.assert_not_called()


class MeasurementDeadline(unittest.TestCase):
    def test_timeout_propagates_in_each_phase_of_both_controllers(self):
        for controller in [run,count_censored]:
            for phase in ['verify','plain','profile']:
                with self.subTest(controller=controller.__name__,phase=phase), tempfile.TemporaryDirectory() as tmp:
                    root=Path(tmp)
                    (root/'measurement-pins.json').write_text('{"binarySha256":"pin"}')
                    if controller is count_censored:
                        directory=root/'results/10000';(directory/'fixture').mkdir(parents=True)
                        (directory/'warm-qualification.json').write_text('{"stabilized":false}')
                    def warm_child(command,**kwargs):
                        Path(command[2]).mkdir(parents=True)
                        child=Mock(stderr=iter(['{"kind":"warm","tick":1000,"census":{"meanLength":1,"world":{"food":1}}}\n']))
                        child.wait.return_value=0;child.poll.return_value=0
                        return child
                    def measure(command,**kwargs):
                        self.assertEqual(kwargs['timeout'],3600)
                        if command[5]==phase:
                            raise subprocess.TimeoutExpired(command,kwargs['timeout'])
                        output=Path(command[-1])
                        output.write_text('{"actionSha256":"actions","finalStateSha256":"state"}')
                        output.with_suffix('.tape').write_bytes(b'actions')
                    with patch.object(controller,'digest',return_value='pin'), \
                         patch.object(controller,'emit') as emit, \
                         patch.object(controller.subprocess,'Popen',side_effect=warm_child), \
                         patch.object(controller.subprocess,'run',side_effect=measure), \
                         patch.object(run,'stable',return_value={'screen':True}):
                        with self.assertRaises(subprocess.TimeoutExpired):
                            if controller is run:controller.execute(root,[1000])
                            else:controller.execute(root,root)
                    self.assertFalse(any(call.args[0] in ['population-complete','complete','guarded-counting-complete']
                                         for call in emit.call_args_list))
                    self.assertFalse((root/'results/10000/censored-counts.json').exists())


if __name__=='__main__':unittest.main()
