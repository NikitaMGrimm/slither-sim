"""Bounded acquisition coordinator. Only this command contacts game servers."""
import ctypes
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = Path(r'C:\Users\Nikita\.t3\scratch\official-capture-20261025')


def append(name, row):
    row['utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (ROOT / name).open('a', encoding='utf-8', newline='\n') as f:
        f.write(json.dumps(row, allow_nan=False) + '\n')
        f.flush()
        os.fsync(f.fileno())


def load():
    command = """$p=Get-CimInstance Win32_Process; $busy=@($p | Where-Object {$_.Name -match '^(host_load|slither-sim)(\\.exe)?$'} | Select-Object ProcessId,Name,CommandLine); $top=@(Get-Process | Sort-Object CPU -Descending | Select-Object -First 12 Id,ProcessName,CPU,WorkingSet64); [pscustomobject]@{busy=$busy; top=$top; cpu=@(Get-CimInstance Win32_Processor | Select-Object LoadPercentage)} | ConvertTo-Json -Depth 5 -Compress"""
    return json.loads(subprocess.check_output(['powershell', '-NoProfile', '-Command', command], text=True))


def main():
    if not ctypes.windll.kernel32.SetProcessAffinityMask(ctypes.windll.kernel32.GetCurrentProcess(), 48):
        raise OSError('cannot pin coordinator/children to CPUs 4-5')
    os.environ['UV_THREADPOOL_SIZE'] = '1'
    plan = json.loads((HERE / 'plan.json').read_text())
    plan_sha = hashlib.sha256((HERE / 'plan.json').read_bytes()).hexdigest()
    lock = ROOT / 'session.lock'
    with lock.open('x', encoding='utf-8') as f:
        f.write(str(os.getpid()))
    try:
        for batch in plan['batches']:
            while True:
                state = load()
                append('machine.jsonl', {'batch': batch['id'], 'phase': 'before', 'state': state})
                if not state['busy']:
                    break
                time.sleep(30)
            directory = ROOT / 'raw' / batch['id']
            append('attempts.jsonl', {'batch': batch, 'phase': 'reserved', 'planSha256': plan_sha,
                                     'affinityMask': 48, 'connectionCeiling': 2})
            started = time.monotonic()
            log = ROOT / (batch['id'] + '.log')
            with log.open('x', encoding='utf-8') as output:
                child = subprocess.Popen(['node', str(HERE / 'capture.mjs'), str(HERE / 'plan.json'),
                                          str(directory), batch['id']], stdout=output, stderr=subprocess.STDOUT)
                append('attempts.jsonl', {'batchId': batch['id'], 'phase': 'started', 'pid': child.pid})
                while child.poll() is None:
                    time.sleep(30)
                    append('machine.jsonl', {'batch': batch['id'], 'phase': 'during', 'state': load()})
                    if time.monotonic() - started > batch['seconds'] + 60:
                        child.kill()
                        append('attempts.jsonl', {'batchId': batch['id'], 'phase': 'timeout'})
                code = child.wait()
            append('attempts.jsonl', {'batchId': batch['id'], 'phase': 'exited', 'exitCode': code,
                                     'wallSeconds': time.monotonic() - started})
            print(batch['id'], code, flush=True)
            time.sleep(30)
    finally:
        lock.unlink()


if __name__ == '__main__':
    main()
