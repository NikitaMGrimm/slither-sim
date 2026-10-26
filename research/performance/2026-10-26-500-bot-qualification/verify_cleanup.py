"""Read-only, ownership-scoped final audit across all retained attempt states."""
import argparse
import datetime as dt
import json
from pathlib import Path
import subprocess

REGIONS = ['eu-central-1', 'us-east-1', 'us-west-2', 'eu-west-1']


def verify(attempts, scratch, output):
    repo = Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip()).resolve()
    scratch = scratch.resolve()
    if scratch.is_relative_to(repo):
        raise ValueError('Private AWS audit scratch must be outside the repository')
    scratch.mkdir(parents=True, exist_ok=True)
    states = [json.loads((root / 'state.json').read_text()) for root in attempts]
    tags = [state['tag'] for state in states]
    spots = {value for state in states for value in state['spots']}
    instances = {value for state in states for value in state['instances']}
    keys = {state.get('keyName') for state in states}

    def aws(service, action, *args, region='eu-central-1', absent_ok=False):
        proc = subprocess.run(['aws', service, action, *args, '--region', region,
                               '--output', 'json', '--no-cli-pager'], capture_output=True, text=True, timeout=120)
        path = scratch / (region + '-' + service + '-' + action + '.json')
        path.write_text(proc.stdout or proc.stderr, encoding='utf-8')
        if proc.returncode:
            if absent_ok and any(reason in proc.stderr for reason in ['NoSuchEntity', 'NoSuchBucket', '(404)']):
                return None
            raise RuntimeError('AWS audit call failed; identifiers retained only in private scratch')
        return json.loads(proc.stdout or '{}')

    result = {'verifiedUtc': dt.datetime.now(dt.timezone.utc).isoformat(), 'attemptCount': len(states), 'regions': {}}
    for region in REGIONS:
        filters = 'Name=tag:Task,Values=' + ','.join(tags)
        owned_instances = aws('ec2', 'describe-instances', '--filters', filters, region=region)
        volumes = aws('ec2', 'describe-volumes', '--filters', filters, region=region)
        groups = aws('ec2', 'describe-security-groups', '--filters', filters, region=region)
        spot_requests = aws('ec2', 'describe-spot-instance-requests', region=region)
        key_pairs = aws('ec2', 'describe-key-pairs', region=region)
        result['regions'][region] = {
            'nonterminatedInstances': sum(i['State']['Name'] != 'terminated'
                for reservation in owned_instances['Reservations'] for i in reservation['Instances']),
            'openOrActiveSpotRequests': sum(s['State'] in ['open', 'active']
                for s in spot_requests['SpotInstanceRequests']
                if s['SpotInstanceRequestId'] in spots or s.get('InstanceId') in instances),
            'volumes': len(volumes['Volumes']), 'securityGroups': len(groups['SecurityGroups']),
            'keyPairs': sum(k['KeyName'] in keys for k in key_pairs['KeyPairs']),
        }
    buckets = {state['bucket'] for state in states if state.get('bucket')}
    roles = {state['role'] for state in states if state.get('role')}
    bucket_inventory = aws('s3api', 'list-buckets')
    result['handoff'] = {'buckets': sum(b['Name'] in buckets for b in bucket_inventory['Buckets']),
                         'roles': 0, 'instanceProfiles': 0}
    for name in roles:
        result['handoff']['roles'] += aws('iam', 'get-role', '--role-name', name, absent_ok=True) is not None
        result['handoff']['instanceProfiles'] += aws('iam', 'get-instance-profile', '--instance-profile-name', name, absent_ok=True) is not None
    result['allZero'] = not any(value for region in result['regions'].values() for value in region.values()) and not any(result['handoff'].values())
    output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8', newline='\n')
    if not result['allZero']:
        raise RuntimeError('Study-owned resources remain; inspect private inventory and complete cleanup')
    print('All study resources verified absent in four regions; S3/IAM handoff also absent')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('attempts', type=Path, nargs='+')
    parser.add_argument('--private-scratch', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    verify(args.attempts, args.private_scratch, args.output)
