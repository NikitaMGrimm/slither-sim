"""Reduce retained local-only archives into bounded, non-identifying study data."""
import argparse
import hashlib
import json
from pathlib import Path

from analyze import comparison, reduce_trial, stats

HERE = Path(__file__).resolve().parent


def digest(path):
    return {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'bytes': path.stat().st_size}


def write_json(path, obj):
    path.write_text(json.dumps(obj, indent=2) + '\n', encoding='utf-8', newline='\n')


def select(failed, completed):
    raw = completed / 'collected' / 'raw'
    completed_state = json.loads((completed / 'state.json').read_text())
    trials = []
    for name in json.loads((HERE / 'plan.json').read_text())['trialOrder']:
        raw_name = completed_state['completedSlots'][name]
        trial = reduce_trial(raw / raw_name)
        trial['rawDirectory'] = raw_name
        trial['name'] = name
        trials.append(trial)
    if (raw / 'A4t' / 'DONE').exists():
        trials.append(reduce_trial(raw / 'A4t'))
    for trial in trials:
        populations = trial['watcher'].pop('leaderboardPopulationSamples', [])
        trial['receivedPopulation'] = ({**stats(populations), 'mean': sum(populations) / len(populations)}
                                       if populations else {'count': 0})
    attempts = []
    supplementary = []
    spend = 0
    for number, scratch in enumerate([*failed, completed], 1):
        status = ('completed' if scratch == completed else
                  'invalid native smoke; no graded holds' if number == 1 else
                  'AWS-confirmed Spot termination; entire attempt excluded from primary design')
        state = json.loads((scratch / 'state.json').read_text())
        archive = scratch / 'raw-archive.json'
        reasons = []
        reason_file = scratch / 'final-instance-reason.json'
        if reason_file.exists():
            reasons = [i.get('StateReason', {}).get('Code')
                       for reservation in json.loads(reason_file.read_text())['Reservations']
                       for i in reservation['Instances']]
        attempts.append({'status': status, 'rawArchive': json.loads(archive.read_text()) if archive.exists() else None,
                         'observedAwsStateReasonCodes': reasons,
                         'retainedSummaries': {p.name: digest(p) for p in sorted(scratch.glob('*-summary.json'))},
                         'cleanupUtc': state['cleanupUtc'],
                         'cleanup': json.loads((scratch / 'cleanup-verification.json').read_text()),
                         'elapsedHoursUpperBound': state['billedHoursUpperBound'],
                         'quotedSpotPriceUsdHour': state['hourlySpotPrice'],
                         'estimatedSpendUsd': state['estimatedSpendUsd'],
                         'computeAtPriceCapUsd': state['computeCapEstimateUsd']})
        spend += state['estimatedSpendUsd']
        if scratch != completed:
            for path in sorted(scratch.glob('*-summary.json')):
                summary = json.loads(path.read_text())
                if summary['name'] not in ['A1', 'B1', 'A2', 'B2', 'A3', 'B3']:
                    continue
                supplementary.append({'attempt': len(attempts), 'slot': summary['name'], 'excludedFromPrimary': True,
                    'revisions': state.get('launchRevisions', json.loads((HERE / 'plan.json').read_text())['revisions']),
                    'summaryFile': digest(path), 'holdSeconds': summary['holdSeconds'],
                    'simulationCpuPercentOneCore': summary['simulationCpuPercentOneCore'],
                    'botCpuCores': summary['botCpuCores'], 'ticksPerSecond': summary['ticksPerSecond'],
                    'deferredTicks': summary['deferredTicks'], 'excessMs': summary['excessMs'],
                    'valid': summary['valid'], 'realTime': summary['realTime'], 'officialLike': summary['officialLike'],
                    'rawAccess': 'Full raw not recovered; returned summary only'})
    environment = (completed / 'collected' / 'environment.txt').read_text()
    allowed = ['CPU(s):', 'On-line CPU(s) list:', 'Thread(s) per core:', 'Core(s) per socket:',
               'Socket(s):', 'Model name:', 'Architecture:']
    hardware = [' '.join(line.split()) for line in environment.splitlines() if any(line.startswith(k) for k in allowed)]
    result = {'dataOrigin': 'local-simulator', 'access': 'Raw logs and full archives local-only; no public download',
              'planSha256': digest(HERE / 'plan.json')['sha256'],
              'revisions': completed_state['launchRevisions'],
              'attempt5AmendmentSha256': digest(HERE / 'attempt5-amendment.json')['sha256'],
              'awsInstanceType': completed_state['launchAws']['instanceType'], 'hardware': hardware,
              'toolchain': (completed / 'collected' / 'toolchain.txt').read_text().strip(),
              'trials': trials, 'comparison': comparison(trials), 'attempts': attempts, 'supplementary': supplementary,
              'estimatedSpendUsdTotal': spend + .01, 'computeStorageIpv4EstimateUsd': spend,
              'handoffAndTransferAllowanceUsd': .01,
              'spendScope': 'Spot at launch quote plus public IPv4 at $0.005/h and 24 GiB gp3 rounded up to $0.096/GiB-month, plus $0.01 S3/request/transfer allowance; estimate, not invoice',
              'handoffCleanup': json.loads((completed / 'handoff-cleanup.json').read_text()),
              'finalCleanup': json.loads((HERE / 'cleanup-verification.json').read_text()),
              'botDecisionCost': None, 'botDecisionCostReason': 'Production host exposes no per-decision timing counter'}
    write_json(HERE / 'selected-results.json', result)
    sources = {'dataOrigin': 'local-simulator', 'access': 'local-only raw archives',
               'planSha256': result['planSha256'], 'revisions': result['revisions'],
               'sourceArchives': {rev: digest(completed / (rev + '.tar')) for rev in ['A', 'B']},
               'binaries': {line.split()[-1].split('/')[0]: line.split()[0]
                            for line in (completed / 'collected' / 'binaries.sha256').read_text().splitlines()},
               'executedScripts': {p.name: digest(p) for p in sorted((completed / 'collected' / 'scripts').iterdir()) if p.is_file()},
               'collectorVersions': [digest(s / 'collector-at-launch.py') for s in [*failed, completed]],
               'awsReasonInventories': [digest(s / 'final-instance-reason.json') for s in failed
                                        if (s / 'final-instance-reason.json').exists()],
               'rawArchives': [a['rawArchive'] for a in attempts if a['rawArchive'] is not None]}
    write_json(HERE / 'source-pins.json', sources)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('attempts', type=Path, nargs='+', help='Failed attempt directories, then completed attempt last')
    args = parser.parse_args()
    select(args.attempts[:-1], args.attempts[-1])
