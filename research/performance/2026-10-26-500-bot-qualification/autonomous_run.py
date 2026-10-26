"""Run the entire frozen sequence on EC2; progress/results use a private S3 handoff.

The temporary instance role can write only this study's two S3 object keys.
No desktop SSH connection or embedded credential is needed after launch.
"""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import time

import boto3

ROOT = Path('/home/ubuntu/study')
sys.path.insert(0, str(ROOT / 'scripts'))
from analyze import reduce_trial


def main():
    os.chdir(ROOT)
    handoff = json.loads((ROOT / 'handoff.json').read_text())
    s3 = boto3.client('s3', region_name='eu-central-1')
    progress = {'state': 'running', 'phase': 'build', 'completedSlots': {}, 'trialAttempts': []}

    def publish():
        progress['updatedUtc'] = dt.datetime.now(dt.timezone.utc).isoformat()
        payload = json.dumps(progress, indent=2).encode()
        temporary = ROOT / 'progress.tmp'
        temporary.write_bytes(payload)
        temporary.replace(ROOT / 'progress.json')
        # Progress upload failure must not interrupt measurement.
        for attempt in range(3):
            try:
                s3.put_object(Bucket=handoff['bucket'], Key='progress.json', Body=payload)
                return
            except Exception:
                time.sleep(2 ** attempt)

    def command(argv, log, timeout):
        with (ROOT / log).open('w') as handle:
            return subprocess.run(argv, stdout=handle, stderr=subprocess.STDOUT,
                                  stdin=subprocess.DEVNULL, timeout=timeout).returncode

    def trial(slot, threads=2, warmup=30, hold=300):
        progress['phase'] = slot
        publish()
        for attempt in range(2):
            name = slot if attempt == 0 else slot + '-retry1'
            code = command(['python3', str(ROOT / 'scripts/run_trial.py'),
                            str(ROOT / slot[0] / 'target/release/examples/host_qualification'),
                            str(ROOT / 'raw' / name), '--threads', str(threads),
                            '--warmup', str(warmup), '--hold', str(hold)], name + '-worker.log', warmup + hold + 90)
            progress['trialAttempts'].append({'slot': slot, 'rawDirectory': name, 'exitCode': code})
            if code:
                publish()
                continue
            summary = reduce_trial(ROOT / 'raw' / name)
            (ROOT / 'raw' / name / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
            if not summary['valid']:
                raise RuntimeError('Completed hold failed predeclared validity')
            progress['completedSlots'][slot] = name
            # Full summaries are bounded; raw frames remain in the final archive.
            progress.setdefault('summaries', {})[slot] = summary
            publish()
            return
        raise RuntimeError('Hold failed twice in its original slot')

    started = time.monotonic()
    try:
        publish()
        build = ('set -e; cd /home/ubuntu/study; '
                 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.99.0; '
                 'source "$HOME/.cargo/env"; rustc -Vv > toolchain.txt; '
                 'for R in A B; do cd /home/ubuntu/study/"$R"; '
                 'CARGO_BUILD_JOBS=4 cargo build --release --locked --example host_qualification; done; '
                 'cd /home/ubuntu/study; sha256sum A/target/release/examples/host_qualification B/target/release/examples/host_qualification > binaries.sha256')
        if command(['bash', '-lc', build], 'build.log', 1500):
            raise RuntimeError('Release build failed')
        trial('Asmoke', warmup=3, hold=8)
        plan = json.loads((ROOT / 'scripts/plan.json').read_text())
        for slot in plan['trialOrder']:
            if time.monotonic() - started > 80 * 60:
                raise RuntimeError('Sequence exceeds 80-minute collection budget')
            trial(slot)
        if time.monotonic() - started < 65 * 60:
            trial('A4t', threads=4)
        progress['state'] = 'completed'
    except BaseException as error:
        progress.update(state='failed', errorType=type(error).__name__, error=str(error))
    finally:
        progress['phase'] = 'archive'
        publish()
        archive = ROOT / 'results.tar.gz'
        with tarfile.open(archive, 'w:gz') as bundle:
            for name in ['raw', 'scripts', 'progress.json', 'environment.txt', 'toolchain.txt', 'binaries.sha256']:
                if (ROOT / name).exists():
                    bundle.add(ROOT / name, arcname=name)
            for log in ROOT.glob('*.log'):
                bundle.add(log, arcname=log.name)
        progress['archive'] = {'sha256': hashlib.sha256(archive.read_bytes()).hexdigest(), 'bytes': archive.stat().st_size}
        for attempt in range(10):
            try:
                with archive.open('rb') as handle:
                    s3.put_object(Bucket=handoff['bucket'], Key='results.tar.gz', Body=handle)
                progress['phase'] = 'finished'
                publish()
                break
            except Exception:
                time.sleep(min(2 ** attempt, 30))
        else:
            progress.update(state='failed', phase='upload-failed')
            publish()
        # The boot shutdown remains the final guard; desktop cleanup normally
        # terminates sooner after downloading and hashing the private archive.
        (ROOT / 'AUTONOMOUS-DONE').write_text('finished\n')


if __name__ == '__main__':
    main()
