"""Host-only, temporary quiet-image policy; raw receipts stay outside Git."""
import argparse
import json
from pathlib import Path
import subprocess
import time

TIMERS = ('apt-daily.timer', 'apt-daily-upgrade.timer', 'fwupd-refresh.timer',
          'fstrim.timer', 'man-db.timer')


def command(*args):
    return subprocess.check_output(args, text=True, timeout=45)


def discover():
    return {line.split()[0] for line in command('systemctl', 'list-unit-files',
            '--no-legend', '--no-pager').splitlines() if line.strip()}


def targeted_service(name):
    return name in ('fwupd.service', 'snapd.service', 'snapd.socket',
                    'amazon-ssm-agent.service') or (
        name.startswith('snap.amazon-ssm-agent.') and name.endswith('.service'))


def state(unit):
    output = command('systemctl', 'show', unit, '--no-pager',
                     '-p', 'Id', '-p', 'LoadState', '-p', 'ActiveState',
                     '-p', 'SubState', '-p', 'UnitFileState', '-p', 'Result')
    return dict(line.split('=', 1) for line in output.splitlines() if '=' in line)


def record(root, label, units):
    (root / (label + '-timers.txt')).write_text(command('systemctl', 'list-timers',
        '--all', '--no-pager'), encoding='utf-8')
    value = {'unix': time.time(), 'units': {unit: state(unit) for unit in sorted(units)}}
    (root / (label + '.json')).write_text(json.dumps(value, indent=2) + '\n')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('prepare', 'after'))
    parser.add_argument('root', type=Path)
    args = parser.parse_args()
    root = args.root
    root.mkdir(parents=True, exist_ok=True)
    if args.mode == 'after':
        policy = json.loads((root / 'policy.json').read_text())
        result = record(root, 'after', policy['recordedUnits'])
        verify(result, policy['maskedUnits'])
        return
    units = discover()
    timers = sorted(set(TIMERS) & units)
    jobs = sorted({t.replace('.timer', '.service') for t in timers} & units)
    services = sorted(u for u in units if targeted_service(u))
    protected = sorted(set(('systemd-networkd.service', 'systemd-resolved.service',
        'ssh.service', 'chrony.service', 'systemd-timesyncd.service',
        'systemd-journald.service', 'rsyslog.service')) & units)
    recorded = sorted(set(timers + jobs + services + protected))
    record(root, 'before', recorded)
    for timer in timers:
        command('systemctl', 'mask', '--runtime', timer)
        command('systemctl', 'stop', timer)
    deadline = time.monotonic() + 300
    while any(state(job)['SubState'] in ('running', 'start', 'start-pre', 'start-post', 'stop', 'stop-sigterm')
              or state(job)['ActiveState'] in ('activating', 'deactivating') for job in jobs):
        if time.monotonic() >= deadline:
            raise RuntimeError('Existing maintenance job did not finish within five minutes')
        time.sleep(1)
    for service in services:
        command('systemctl', 'mask', '--runtime', service)
        command('systemctl', 'stop', service)
    policy = {'timersPresent': timers, 'timersAbsent': sorted(set(TIMERS) - units),
              'servicesStopped': services, 'maskedUnits': sorted(timers + services),
              'recordedUnits': recorded, 'runningJobsWaitedToCompletion': True}
    (root / 'policy.json').write_text(json.dumps(policy, indent=2) + '\n')
    verify(record(root, 'quiet', recorded), policy['maskedUnits'])


def verify(receipt, masked):
    for name in masked:
        value = receipt['units'][name]
        if value['ActiveState'] != 'inactive' or value['LoadState'] != 'masked':
            raise RuntimeError('Quiet unit not inactive/masked: ' + name)


if __name__ == '__main__':
    main()
