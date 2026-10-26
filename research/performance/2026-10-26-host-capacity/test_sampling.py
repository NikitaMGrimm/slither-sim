"""Linux-only /proc race regression; use Python 3.12+ on the measurement host."""
import os
import unittest
from unittest.mock import MagicMock, patch

if os.name == 'posix':
    import run_trial

@unittest.skipUnless(os.name == 'posix', 'native Linux sampler')
class SamplingTests(unittest.TestCase):
    def test_exit_permission_race_keeps_cpu_and_rss(self):
        stat = ['0']*23
        stat[11], stat[12], stat[21] = '100', '50', '3'
        stat_path, fd_path = MagicMock(), MagicMock()
        stat_path.read_text.return_value = '42 (host) ' + ' '.join(stat)
        fd_path.iterdir.side_effect = PermissionError('exiting process')
        with patch.object(run_trial, 'Path', side_effect=[stat_path,fd_path]):
            result = run_trial.proc(42)
        self.assertEqual(result['cpuSeconds'],150/run_trial.HZ)
        self.assertEqual(result['rssBytes'],3*os.sysconf('SC_PAGE_SIZE'))
        self.assertIsNone(result['fds'])

    def test_thread_uses_task_stat_not_thread_group_stat(self):
        path = MagicMock()
        path.read_text.return_value = '42 (thread) ' + ' '.join(['0']*23)
        path.iterdir.return_value = []
        with patch.object(run_trial, 'Path', return_value=path) as mocked:
            run_trial.proc(42, 43)
        self.assertEqual(mocked.call_args_list[0].args[0], '/proc/42/task/43/stat')

if __name__=='__main__': unittest.main()
