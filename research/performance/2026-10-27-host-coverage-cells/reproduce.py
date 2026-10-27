"""Frozen fifteen-trace and paired user-instruction study; Linux x86-64."""
import sys, tarfile
from pathlib import Path
if not __debug__:
    raise SystemExit('run without -O: evidence checks require asserts')
if len(sys.argv) < 4:
    raise SystemExit('MODE ARCHIVES OUT [--perf PERF]')
archive = Path(sys.argv[2]) / 'base.tar'
with tarfile.open(archive) as t:
    shared = t.extractfile('research/performance/2026-10-27-host-full-map-sharing/reproduce.py').read().decode()
    frames = t.extractfile('research/performance/2026-10-27-host-owned-legacy-frames/reproduce.py').read().decode()
# Six native option/protocol traces; default stock/half13/all13 socket holds.
start, end = "if a.mode == 'sockets':", "elif a.mode == 'pairs':"
socket = frames[frames.index(start):frames.index(end)].replace('18794', '18796')
socket = socket.replace('text = driver.read_text()', "text = driver.read_text().replace('os.sched_setaffinity(0,{0})', 'os.sched_setaffinity(0,{1})')")
shared = shared[:shared.index(start)] + socket + shared[shared.index(end):]
if sys.argv[1] == 'sockets':
    shared = shared.replace('os.sched_setaffinity(0, {0})', 'os.sched_setaffinity(0, {1})')
assert shared.count('src/client/encode13.rs') == 1
shared = shared.replace('src/client/encode13.rs', 'src/client/simulation/snakes.rs')
shared = shared.replace('encodeSha256', 'snakesSha256')
exec(compile(shared, str(archive) + '-frozen-driver', 'exec'))
