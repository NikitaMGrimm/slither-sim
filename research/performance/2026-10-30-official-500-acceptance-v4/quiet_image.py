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
                     '-p', 'SubState', '-p', 'UnitFileState', '-p', 'Result', '-p', 'FragmentPath')
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
        verify(result, policy['maskedUnits'], policy['servicesStopped'])
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
        command('systemctl', 'stop', timer)
        command('systemctl', 'mask', '--runtime', timer)
        if state(timer)['ActiveState'] == 'failed':
            command('systemctl', 'reset-failed', timer)
    deadline = time.monotonic() + 300
    while any(state(job)['SubState'] in ('running', 'start', 'start-pre', 'start-post', 'stop', 'stop-sigterm')
              or state(job)['ActiveState'] in ('activating', 'deactivating') for job in jobs):
        if time.monotonic() >= deadline:
            raise RuntimeError('Existing maintenance job did not finish within five minutes')
        time.sleep(1)
    masked_services, stop_only = [], []
    for service in sorted(services, key=lambda u: not u.endswith('.socket')):
        command('systemctl', 'stop', service)
        # A /run mask cannot override an actual unit file in /etc (snap SSM).
        # Stopping snapd plus its socket removes that service's activation path.
        if state(service).get('FragmentPath', '').startswith('/etc/'):
            stop_only.append(service)
        else:
            command('systemctl', 'mask', '--runtime', service)
            masked_services.append(service)
        if state(service)['ActiveState'] == 'failed':
            command('systemctl', 'reset-failed', service)
    policy = {'timersPresent': timers, 'timersAbsent': sorted(set(TIMERS) - units),
              'servicesStopped': services, 'stopOnlyEtcUnits': stop_only,
              'maskedUnits': sorted(timers + masked_services),
              'recordedUnits': recorded, 'runningJobsWaitedToCompletion': True}
    (root / 'policy.json').write_text(json.dumps(policy, indent=2) + '\n')
    verify(record(root, 'quiet', recorded), policy['maskedUnits'], services)


def verify(receipt, masked, stopped=()):
    for name in masked:
        value = receipt['units'][name]
        # A referenced unit may remain loaded after a runtime mask. UnitFileState
        # reports the mask; LoadState alone would reject valid inactive masks.
        if value['ActiveState'] != 'inactive' or value['UnitFileState'] not in ('masked', 'masked-runtime'):
            raise RuntimeError('Quiet unit not inactive/masked: ' + name)
    for name in stopped:
        if receipt['units'][name]['ActiveState'] != 'inactive':
            raise RuntimeError('Quiet service still active: ' + name)


if __name__ == '__main__':
    main()
