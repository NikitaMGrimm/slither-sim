"""One-second all-process CPU and complete proc/stat, on the host's sole CPU."""
import json
from pathlib import Path
import sys
import time


def process(pid):
    text = (Path('/proc') / pid / 'stat').read_text()
    fields = text.rsplit(')', 1)[1].split()
    return {'pid': int(pid), 'start': int(fields[19]),
            'userTicks': int(fields[11]), 'systemTicks': int(fields[12]),
            'comm': text[text.index('(') + 1:text.rindex(')')]}


def main(root):
    import os
    started = time.time()
    (root / 'diagnostic-start.json').write_text(json.dumps({'unix': started,
        'clockTicksPerSecond': os.sysconf('SC_CLK_TCK'), 'pid': os.getpid()}) + '\n')
    with (root / 'background.jsonl').open('x') as output:
        next_sample = time.monotonic()
        while not (root / 'diagnostic-stop').exists():
            began = time.time()
            stat = Path('/proc/stat').read_text()
            rows, unavailable = [], []
            for item in Path('/proc').iterdir():
                if item.name.isdecimal():
                    try:
                        rows.append(process(item.name))
                    except (OSError, ValueError):
                        unavailable.append(int(item.name))
            ended = time.time()
            output.write(json.dumps({'unix': began, 'endUnix': ended,
                'procStat': stat, 'processes': rows, 'unavailablePids': unavailable}) + '\n')
            output.flush()
            next_sample += 1
            time.sleep(max(0, next_sample - time.monotonic()))


if __name__ == '__main__':
    main(Path(sys.argv[1]))
