"""Parent-requested total-load gate around the frozen continuation coordinator."""
import importlib.util
import json
from pathlib import Path
import subprocess

path = Path(__file__).with_name('run-origin.py')
spec = importlib.util.spec_from_file_location('coordinator', path)
coordinator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coordinator)
original = coordinator.load


def load():
    script = """$v=@(1..10 | ForEach-Object {(Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'").PercentProcessorTime; Start-Sleep -Seconds 1}); [pscustomobject]@{samples=$v; mean=($v | Measure-Object -Average).Average; max=($v | Measure-Object -Maximum).Maximum} | ConvertTo-Json -Compress"""
    sample = json.loads(subprocess.check_output(['powershell', '-NoProfile', '-Command', script], text=True))
    state = original()
    state['totalCpuTenSeconds'] = sample
    # The same gate runs during acquisition for contamination logging; a high
    # sample never aborts or replaces an ongoing batch.
    if sample['mean'] >= 30:
        state['busy'].append({'Name': 'total_cpu_at_least_30_percent', 'mean': sample['mean']})
    return state


coordinator.load = load
coordinator.main()
