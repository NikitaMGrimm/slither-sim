"""Persistent remote job ownership: reconnecting launch requests never repeat work."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def write(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value) + '\n')
    temporary.replace(path)


def status(root):
    if (root / 'exit.json').exists():
        return {'state': 'finished', **json.loads((root / 'exit.json').read_text())}
    if (root / 'running.json').exists():
        running = json.loads((root / 'running.json').read_text())
        try:
            os.kill(running['pid'], 0)
        except ProcessLookupError:
            command = json.loads((root / 'claim.json').read_text())['command']
            if len(command) > 3 and command[1].endswith('run_trial.py'):
                output = command[3]
                if (Path(output) / 'DONE').exists():
                    return {'state': 'finished', 'code': 0, 'recovered': True}
                # A dead supervisor does not prove its detached host is gone.
                # Wait for surviving trial processes rather than overlap holds.
                for proc in Path('/proc').glob('[0-9]*'):
                    try:
                        argv = (proc / 'cmdline').read_bytes().split(b'\0')
                        if output.encode() in argv:
                            return {'state': 'running-orphan'}
                    except (FileNotFoundError, ProcessLookupError, PermissionError):
                        pass
            return {'state': 'interrupted'}
        return {'state': 'running'}
    if (root / 'claim.json').exists():
        claimed = json.loads((root / 'claim.json').read_text())
        return {'state': 'starting' if time.time() - claimed['unix'] < 30 else 'interrupted'}
    return {'state': 'absent'}


def launch(root, command):
    root.mkdir(parents=True, exist_ok=True)
    try:
        # O_EXCL deduplicates even a retry after the launch response was lost.
        with (root / 'claim.json').open('x') as handle:
            json.dump({'unix': time.time(), 'command': command}, handle)
    except FileExistsError:
        return status(root)
    with (root / 'supervisor.log').open('w') as log:
        subprocess.Popen([sys.executable, str(Path(__file__).resolve()), 'execute', str(root), *command],
                         stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                         start_new_session=True)
    return {'state': 'starting'}


def execute(root, command):
    write(root / 'running.json', {'pid': os.getpid(), 'unix': time.time()})
    try:
        with (root / 'command.log').open('w') as log:
            result = subprocess.run(command, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                                    # The autonomous sequence includes builds and
                                    # seven holds; stay below the 100-minute boot guard.
                                    timeout=90 * 60)
        write(root / 'exit.json', {'code': result.returncode, 'unix': time.time()})
    except BaseException as error:
        write(root / 'exit.json', {'code': 125, 'unix': time.time(), 'errorType': type(error).__name__})
        raise


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['launch', 'status', 'execute'])
    parser.add_argument('root', type=Path)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.action == 'execute':
        execute(args.root, args.command)
    else:
        print(json.dumps(launch(args.root, args.command) if args.action == 'launch' else status(args.root)))
