"""Synthetic transport failures: retries may not repeat started workload commands."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from aws_run import Study
from remote_job import execute, launch, status


class TransportTests(unittest.TestCase):
    def study(self, root):
        study = Study.__new__(Study)
        study.root = Path(root)
        study.state = {'public': 'example.invalid'}
        study.log = lambda message: None
        return study

    def test_transient_setup_retried_with_bounded_backoff(self):
        with tempfile.TemporaryDirectory() as tmp:
            study = self.study(tmp)
            results = [subprocess.CompletedProcess([], 255, '', 'kex_exchange_identification: Connection reset'),
                       subprocess.CompletedProcess([], 0, 'ok', '')]
            with patch('aws_run.subprocess.run', side_effect=results) as run, patch('aws_run.time.sleep') as sleep:
                self.assertEqual(study.ssh('status'), 'ok')
                self.assertEqual(run.call_count, 2)
                sleep.assert_called_once_with(2)
            with patch('aws_run.subprocess.run', return_value=results[0]) as run, patch('aws_run.time.sleep') as sleep:
                with self.assertRaises(RuntimeError):
                    study.ssh('status')
                self.assertEqual(run.call_count, 5)
                self.assertEqual([c.args[0] for c in sleep.call_args_list], [2, 4, 8, 16])

    def test_remote_command_failure_not_retried(self):
        with tempfile.TemporaryDirectory() as tmp:
            study = self.study(tmp)
            with patch('aws_run.subprocess.run', return_value=subprocess.CompletedProcess([], 1, '', 'failed')) as run:
                with self.assertRaises(RuntimeError):
                    study.ssh('work')
                self.assertEqual(run.call_count, 1)

    def test_lost_launch_response_cannot_repeat_job(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'job'
            with patch('remote_job.subprocess.Popen') as child:
                self.assertEqual(launch(root, ['fake-work'])['state'], 'starting')
                self.assertEqual(launch(root, ['fake-work'])['state'], 'starting')
                self.assertEqual(child.call_count, 1)
                (root / 'exit.json').write_text(json.dumps({'code': 0}))
                self.assertEqual(launch(root, ['fake-work'])['code'], 0)
                self.assertEqual(child.call_count, 1)

    def test_confirmed_interruption_has_no_implicit_relaunch(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'claim.json').write_text(json.dumps({'unix': 0, 'command': ['fake-work']}))
            with patch('remote_job.subprocess.Popen') as child:
                self.assertEqual(status(root)['state'], 'interrupted')
                self.assertEqual(launch(root, ['fake-work'])['state'], 'interrupted')
                child.assert_not_called()

    def test_supervisor_bounds_whole_sequence_not_only_build(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with patch('remote_job.subprocess.run', return_value=subprocess.CompletedProcess([], 0)) as run:
                execute(root, ['fake-sequence'])
                self.assertEqual(run.call_args.kwargs['timeout'], 90 * 60)
                self.assertEqual(json.loads((root / 'exit.json').read_text())['code'], 0)


if __name__ == '__main__':
    unittest.main()
