"""One idle preflight before the fixed series; never excludes a timed window."""
from pathlib import Path
import os,pwd,time,json,subprocess

r = Path(__file__).resolve().parent
assert (r / 'MATURE-PROOFS-PASS').exists() and (r / 'SHORT-PROOFS-PASS').exists()
assert (r / 'target-cleanup.json').exists()
assert not Path('/tmp/slither-retained-layout-target').exists()
assert time.time() - json.loads((r / 'primary-timing-notice.json').read_text())['unix'] >= 60
pins = json.loads((r / 'confirmation-build.json').read_text())
assert pins['allCopiedBinariesAndGeneratedSourcesVerified']
account = pwd.getpwuid(os.getuid()).pw_name
def inventory(label):
    commands = subprocess.check_output(['ps', '-eo', 'pid,ppid,stat,pcpu,args', '-ww'], text=True)
    (r / (label + '-processes.txt')).write_text(commands.replace(account, '<redacted-account>'))
def snapshot():
    result = {}
    for path in Path('/proc').iterdir():
        if not path.name.isdigit():
            continue
        try:
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
            result[(int(path.name), int(fields[19]))] = int(fields[11]) + int(fields[12])
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            pass
    return result
inventory('confirmation-idle-before')
before = snapshot(); started = time.monotonic(); time.sleep(2); after = snapshot()
inventory('confirmation-idle-after')
busy = [{'pid': key[0], 'cpuQuanta': value - before[key]}
        for key, value in after.items()
        if key in before and value - before[key] > 2 and key[0] != os.getpid()]
receipt = {'elapsedSeconds': time.monotonic() - started, 'logicalCpus': os.cpu_count(),
           'processesOverTwoCpuQuanta': busy, 'targetAbsent': True,
           'counterQuantumMs': 1000 / os.sysconf('SC_CLK_TCK'),
           'timedWindowExclusionRule': False}
with (r / 'confirmation-idle-preflight.json').open('x') as out:
    out.write(json.dumps(receipt, indent=2) + '\n')
assert not busy, busy
print('Confirmation idle preflight passes; every timed window will be retained.', flush=True)
