"""Count host syscall types inside a diagnostic hold; never emit process identities."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def main(pid, out, start, end, method, perf):
    out = Path(out)
    while time.time() < start + .25:
        time.sleep(min(.01, max(.001, start + .25 - time.time())))
    tids = sorted(int(p.name) for p in Path(f'/proc/{pid}/task').iterdir())
    names = {str(t): Path(f'/proc/{pid}/task/{t}/comm').read_text().strip() for t in tids}
    if not {'slither-sockets', 'slither-simulat'}.issubset(names.values()):
        raise RuntimeError('Host threads missing before count window')
    duration = end - time.time() - .25
    if duration < 1:
        raise RuntimeError('Missed syscall-counting window')
    if method == 'perf':
        command = ['sudo', perf, 'stat', '--no-inherit', '--per-thread', '-t',
                   ','.join(map(str, tids)), '-e', 'syscalls:sys_enter_*', '-x', ';',
                   '-o', str(out/'syscalls.csv'), '--timeout', str(int(duration*1000))]
    else:
        command = ['sudo', 'timeout', '-s', 'INT', str(duration), 'strace', '-c', '-f',
                   '-p', str(pid), '-o', str(out/'syscalls-strace.txt')]
    before = time.time()
    result = subprocess.run(command, capture_output=True, text=True, timeout=duration+20)
    after = time.time()
    (out/'syscall-observer.log').write_text(result.stdout+'\n'+result.stderr)
    subprocess.run(['sudo', 'chown', '-R', f'{os.getuid()}:{os.getgid()}', str(out)], check=True)
    (out/'syscall-window.json').write_text(json.dumps({
        'method': method, 'beforeUnix': before, 'afterUnix': after,
        'configuredSeconds': duration, 'threads': names, 'exit': result.returncode,
        'denominator': 'tick starts in observer wrapper bounds; includes tool startup/teardown uncertainty'
    })+'\n')
    if result.returncode not in ([0] if method == 'perf' else [0, 124, 130]):
        raise RuntimeError('Syscall observer failed; private logs retained')


if __name__ == '__main__':
    main(int(sys.argv[1]), sys.argv[2], float(sys.argv[3]), float(sys.argv[4]), sys.argv[5], sys.argv[6])
