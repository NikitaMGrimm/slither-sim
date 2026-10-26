"""Single-instance, price-capped AWS qualification. Identifiers stay in scratch.

Requires an existing AWS CLI login, OpenSSH and Git. No credentials are printed.
Run `prepare`, commit the frozen plan, then `run`. Cleanup also has its own command.
"""
import argparse
import datetime as dt
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import shlex
import subprocess
import tarfile
import time
import urllib.request

HERE = Path(__file__).resolve().parent
REGIONS = ['eu-central-1', 'us-east-1', 'us-west-2', 'eu-west-1']


class Study:
    def __init__(self, args):
        self.args = args
        self.root = args.scratch.resolve()
        self.root.mkdir(parents=True, exist_ok=True)
        state = self.root / 'state.json'
        self.state = json.loads(state.read_text()) if state.exists() else {
            'tag': '500-bot-qualification-' + dt.datetime.now(dt.timezone.utc).strftime('%Y%m%d%H%M%S'),
            'spots': [], 'instances': [], 'groups': [], 'estimatedSpendUsd': 0,
        }
        if not state.exists() and (HERE / 'plan.json').exists():
            self.state['planSha256'] = hashlib.sha256((HERE / 'plan.json').read_bytes()).hexdigest()
        amendment = HERE / 'attempt5-amendment.json'
        if not state.exists() and amendment.exists():
            self.state.setdefault('launchRevisions', json.loads(amendment.read_text())['revisions'])
            self.state['launchAws'] = json.loads(amendment.read_text())['aws']
        self.save('state', self.state)

    def save(self, name, data):
        (self.root / (name + '.json')).write_text(json.dumps(data, indent=2) + '\n', encoding='utf-8')

    def log(self, message):
        print(dt.datetime.now(dt.timezone.utc).isoformat(), message, flush=True)

    def command(self, command, **kwargs):
        result = subprocess.run(command, capture_output=True, text=True, **kwargs)
        if result.returncode:
            (self.root / 'last-command-error.txt').write_text(result.stdout + '\n' + result.stderr, encoding='utf-8')
            raise RuntimeError('Command failed; private details in scratch')
        return result.stdout

    def aws(self, action, *args, region='eu-central-1'):
        return json.loads(self.command(['aws', 'ec2', action, *args, '--region', region,
                                       '--output', 'json', '--no-cli-pager'], timeout=120) or '{}')

    def service(self, service, action, *args):
        return json.loads(self.command(['aws', service, action, *args, '--region', 'eu-central-1',
                                       '--output', 'json', '--no-cli-pager'], timeout=120) or '{}')

    def handoff_create(self):
        name = self.state['tag']
        self.state['bucket'] = name
        self.state['role'] = name
        self.save('state', self.state)
        self.service('s3api', 'create-bucket', '--bucket', name, '--create-bucket-configuration', 'LocationConstraint=eu-central-1')
        self.service('s3api', 'put-public-access-block', '--bucket', name, '--public-access-block-configuration',
                     'BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true')
        trust = {'Version': '2012-10-17', 'Statement': [{'Effect': 'Allow',
                 'Principal': {'Service': 'ec2.amazonaws.com'}, 'Action': 'sts:AssumeRole'}]}
        self.service('iam', 'create-role', '--role-name', name, '--assume-role-policy-document', json.dumps(trust))
        policy = {'Version': '2012-10-17', 'Statement': [{'Effect': 'Allow', 'Action': 's3:PutObject',
                  'Resource': ['arn:aws:s3:::' + name + '/' + key for key in ['progress.json', 'results.tar.gz']]}]}
        self.service('iam', 'put-role-policy', '--role-name', name, '--policy-name', 'study-handoff', '--policy-document', json.dumps(policy))
        self.service('iam', 'create-instance-profile', '--instance-profile-name', name)
        self.service('iam', 'add-role-to-instance-profile', '--instance-profile-name', name, '--role-name', name)
        (self.root / 'handoff.json').write_text(json.dumps({'bucket': name}) + '\n')
        time.sleep(15)

    def git(self, *args):
        return self.command(['git', '-C', str(self.args.repo), *args], timeout=120).strip()

    def prepare(self):
        if (HERE / 'plan.json').exists():
            raise RuntimeError('Frozen plan already exists; never overwrite it')
        self.git('fetch', 'origin')
        pr = json.loads(self.command(['gh', 'pr', 'view', '136', '--repo', 'NikitaMGrimm/slither-sim',
                                     '--json', 'state,headRefOid,mergeCommit,mergedAt'], timeout=60))
        main = self.git('rev-parse', 'origin/main')
        if pr['state'] == 'MERGED':
            b = pr['mergeCommit']['oid']
            a = self.git('rev-parse', b + '^1')
            selection = 'Merged at launch: A first parent, B merge commit'
        else:
            self.git('fetch', 'origin', 'pull/136/head:refs/remotes/origin/pr-136')
            a, b = main, pr['headRefOid']
            selection = 'Open at launch: A origin/main, B latest PR head'
        plan = {
            'studyId': '2026-10-26-500-bot-qualification', 'dataOrigin': 'local-simulator',
            'frozenUtc': dt.datetime.now(dt.timezone.utc).isoformat(),
            'question': 'Can one host core plus separate A4 bot threads deliver 500 bots and one watcher in real time; does PR 136 regress?',
            'revisions': {'A': a, 'B': b, 'originMain': main, 'selection': selection,
                          'pr136State': pr['state'], 'mergeBase': self.git('merge-base', a, b)},
            'build': {'toolchain': '1.99.0', 'profile': 'repository release: thin LTO, one codegen unit',
                      'command': 'cargo build --release --locked --example host_qualification',
                      'jobsAws': 4, 'localBuildJobsMax': 2, 'localLogicalCpusMax': 4,
                      'flags': 'none; no added profiling instrumentation', 'isolatedTargets': True},
            'aws': {'region': 'eu-central-1', 'instanceType': 'c7a.2xlarge', 'vcpus': 8,
                    'market': 'one-time Spot', 'maxPriceUsdHour': .5, 'capUsdTotal': 10,
                    'hardShutdownMinutes': 100, 'instanceInitiatedShutdown': 'terminate',
                    'rootGp3GiB': 24, 'deleteOnTermination': True},
            'workload': {'harness': 'host_qualification.rs, study extension of examples/host_load.rs',
                         'seed': 7, 'bots': 500, 'playersMax': 1, 'world': 'Config::official(501)',
                         'delivery': 'default responsive', 'policy': 'default A4 Forager',
                         'botThreads': 2, 'fixedRoundTripMs': 40, 'warmupSeconds': 30, 'holdSeconds': 300,
                         'watcher': 'loopback slow circle; reconnect 250 ms after death/closure',
                         'affinity': {'simulationAndSockets': [0], 'watcherAndWrapper': [1],
                                      'botWorker0': [2], 'botWorker1': [3], 'sampler': [6]},
                         'rayonNumThreads': 1},
            'trialOrder': ['A1', 'B1', 'A2', 'B2', 'A3', 'B3'],
            'exploratory': {'revision': 'A', 'botThreads': 4, 'botCpus': [2, 3, 4, 5],
                            'rule': 'One 30+300 s hold after primary trials if elapsed instance time < 70 min and no interruption'},
            'metrics': ['ticks/s', 'deferred ticks', 'simulation thread Linux user+system CPU / one core',
                        'socket CPU', 'bot-thread CPU cores', 'simulation busy wall fraction',
                        'watcher frame-excess p50/p99/max', 'system steal', 'watcher coverage and errors'],
            'botDecisionCost': 'No per-decision production counter: unavailable; do not alter runtime for it',
            'timing': 'All post-setup binary frames received during hold; receipt ms minus cumulative server elapsed header; minimum-normalize per connection over the entire hold, exact nearest-rank quantiles. No 1200-frame reset, filtering or stall masking.',
            'grades': {'realTime': 'No newly deferred ticks AND excess p99 <= 10 ms',
                       'officialLike': 'No newly deferred ticks AND excess p99 <= 3 ms'},
            'validity': 'Complete 300 s hold, exact pinned thread affinities at both boundaries and samples, no watcher read/send errors, >=95% watcher playing coverage. Report warmup deferrals separately. Invalid/interrupted attempts retained, never silently replaced.',
            'regression': {'difference': 'B median minus A median, simulation CPU percentage points and excess p99 ms',
                           'spread': 'max(A max-min, B max-min) separately for each metric',
                           'criterion': 'Call regression for a metric only when positive median difference strictly exceeds spread',
                           'alsoReport': 'All trial values and paired B_i-A_i differences with min-max range; descriptive, no inferential significance claim'},
            'limits': 'Single machine/type/seed, fresh world plus five-minute growth; asynchronous A4 runs are not trajectory-identical; no official-server or WAN traffic, no mature-world or 500 socket-player claim; no fitting, held-out fitting validation inapplicable.',
            'failures': 'Stop primary collection on invalid/incomplete trial; retain error and raw files; repair only harness faults with a recorded amendment and rerun all affected comparisons',
            'cleanup': {'regions': REGIONS, 'scope': 'Only resources created by this study',
                        'verify': ['nonterminated instances', 'open/active Spot requests', 'volumes', 'security groups', 'key pairs']},
        }
        (HERE / 'plan.json').write_text(json.dumps(plan, indent=2) + '\n', encoding='utf-8', newline='\n')
        self.save('pr136-at-freeze', pr)
        self.state['planSha256'] = hashlib.sha256((HERE / 'plan.json').read_bytes()).hexdigest()
        self.save('state', self.state)
        self.log('Plan frozen: A=' + a + ', B=' + b + ', PR=' + pr['state'])

    def ssh(self, command, timeout=90, check=True):
        cmd = ['ssh', '-i', str(self.root / 'study.pem'), '-o', 'StrictHostKeyChecking=accept-new',
               '-o', 'UserKnownHostsFile=' + str(self.root / 'known_hosts'), '-o', 'ConnectTimeout=10',
               '-o', 'ServerAliveInterval=15', '-o', 'ServerAliveCountMax=3',
               'ubuntu@' + self.state['public'], command]
        for attempt in range(5):
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
            if result.returncode == 0:
                return result.stdout if check else result
            # Retry connection establishment only. A remote command's failure
            # is never retried here, because it may already have performed work.
            transient = result.returncode == 255 and any(marker in result.stderr.lower() for marker in [
                'kex_exchange_identification', 'connection reset', 'connection timed out',
                'connection refused', 'connection closed by'])
            self.save('ssh-error-' + str(time.time_ns()), {'stderr': result.stderr, 'exitCode': result.returncode})
            if not transient or attempt == 4:
                if not check:
                    return result
                raise RuntimeError('SSH failed after bounded connection retries; private details in scratch')
            self.log(f'Transient SSH setup failure; retry {attempt + 1}/4 after {2 ** (attempt + 1)} s')
            time.sleep(2 ** (attempt + 1))

    def scp(self, local, remote, download=False):
        endpoint = 'ubuntu@' + self.state['public'] + ':' + remote
        command = ['scp', '-i', str(self.root / 'study.pem'), '-o', 'StrictHostKeyChecking=accept-new',
                      '-o', 'UserKnownHostsFile=' + str(self.root / 'known_hosts'),
                      '-o', 'ConnectTimeout=10',
                      *([endpoint, str(local)] if download else [str(local), endpoint])]
        for attempt in range(5):
            try:
                self.command(command, timeout=300)
                return
            except RuntimeError:
                if attempt == 4:
                    raise
                time.sleep(2 ** (attempt + 1))


    def launch(self):
        plan = json.loads((HERE / 'plan.json').read_text())
        revisions = self.state.get('launchRevisions', plan['revisions'])
        settings = self.state.get('launchAws', plan['aws'])
        instance_type = settings['instanceType']
        price_cap = settings['maxPriceUsdHour']
        if hashlib.sha256((HERE / 'plan.json').read_bytes()).hexdigest() != self.state['planSha256']:
            raise RuntimeError('Frozen plan changed')
        self.git('fetch', 'origin')
        current_pr = json.loads(self.command(['gh', 'pr', 'view', '136', '--repo', 'NikitaMGrimm/slither-sim',
                                             '--json', 'state,headRefOid,mergeCommit'], timeout=60))
        if (current_pr['state'] != revisions['pr136State'] or
                (current_pr['state'] != 'MERGED' and (current_pr['headRefOid'] != revisions['B'] or
                 self.git('rev-parse', 'origin/main') != revisions['A']))):
            raise RuntimeError('Launch revisions changed since freeze; record a new plan before launch')
        if current_pr['state'] == 'MERGED' and (current_pr['mergeCommit']['oid'] != revisions['B'] or
                self.git('rev-parse', revisions['B'] + '^1') != revisions['A']):
            raise RuntimeError('Merged PR pins do not match its first parent and merge')
        self.handoff_create()
        self.save('before-instances', self.aws('describe-instances'))
        zones = self.aws('describe-subnets')['Subnets']
        vpc = next(v for v in self.aws('describe-vpcs')['Vpcs'] if v.get('IsDefault'))['VpcId']
        zones = {s['AvailabilityZone']: s for s in zones if s['VpcId'] == vpc and s.get('DefaultForAz')}
        prices = self.aws('describe-spot-price-history', '--instance-types', instance_type,
                          '--product-descriptions', 'Linux/UNIX', '--start-time', dt.datetime.now(dt.timezone.utc).isoformat())
        self.save('spot-prices', prices)
        usable = [p for p in prices['SpotPriceHistory'] if p['AvailabilityZone'] in zones and float(p['SpotPrice']) <= price_cap]
        scores = self.aws('get-spot-placement-scores', '--instance-types', 'c7a.2xlarge', 'c7a.4xlarge', 'c6a.4xlarge',
                          '--target-capacity', '1', '--single-availability-zone', '--region-names', 'eu-central-1')
        zone_names = {z['ZoneId']: z['ZoneName'] for z in self.aws('describe-availability-zones')['AvailabilityZones']}
        by_zone = {zone_names[s['AvailabilityZoneId']]: s['Score'] for s in scores['SpotPlacementScores']
                   if s.get('AvailabilityZoneId') in zone_names}
        prior_zones = {json.loads((p / 'state.json').read_text()).get('zone')
                       for p in self.root.parent.glob('500-bot-qualification-20261026*')
                       if (p / 'final-instance-reason.json').exists()}
        candidates = [p for p in usable if p['AvailabilityZone'] not in prior_zones]
        if not candidates:
            raise RuntimeError('No eligible untried Spot pool below price cap')
        price = min(candidates, key=lambda p: (-by_zone.get(p['AvailabilityZone'], 0), float(p['SpotPrice'])))
        self.save('placement-scores-private', scores)
        self.state.update(zone=price['AvailabilityZone'], hourlySpotPrice=float(price['SpotPrice']))
        # Canonical's public SSM alias avoids putting an AWS owner/account ID in the study.
        parameter = json.loads(self.command(['aws', 'ssm', 'get-parameter', '--region', 'eu-central-1',
            '--name', '/aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id',
            '--output', 'json', '--no-cli-pager'], timeout=120))
        image = self.aws('describe-images', '--image-ids', parameter['Parameter']['Value'])['Images'][0]
        ip = urllib.request.urlopen('https://checkip.amazonaws.com', timeout=20).read().decode().strip()
        ipaddress.IPv4Address(ip)
        tags = [{'Key': 'Project', 'Value': 'slither-sim'}, {'Key': 'Task', 'Value': self.state['tag']}]
        name = self.state['tag']
        self.state['keyName'] = name
        self.save('state', self.state)
        group = self.aws('create-security-group', '--group-name', name, '--description', 'Temporary local host qualification',
                         '--vpc-id', vpc, '--tag-specifications', json.dumps([{'ResourceType': 'security-group', 'Tags': tags}]))['GroupId']
        self.state['groups'].append(group)
        self.save('state', self.state)
        self.aws('authorize-security-group-ingress', '--group-id', group, '--protocol', 'tcp', '--port', '22', '--cidr', ip + '/32')
        key = self.aws('create-key-pair', '--key-name', name, '--key-type', 'ed25519')
        (self.root / 'study.pem').write_text(key.pop('KeyMaterial') + '\n', encoding='ascii')
        self.save('key-metadata', key)
        if os.name == 'nt':
            self.command(['icacls', str(self.root / 'study.pem'), '/inheritance:r', '/grant:r',
                          self.command(['whoami']).strip() + ':R'])
        else:
            os.chmod(self.root / 'study.pem', 0o600)
        userdata = '#!/bin/bash\nset -e\nshutdown -h +100\nexport DEBIAN_FRONTEND=noninteractive\napt-get update\napt-get install -y build-essential pkg-config\ncurl -LsSf https://astral.sh/uv/install.sh | sh\ncp /root/.local/bin/uv /usr/local/bin/uv\ntouch /var/tmp/study-bootstrap-done\n'
        (self.root / 'user-data.sh').write_text(userdata, encoding='utf-8', newline='\n')
        req = {'ImageId': image['ImageId'], 'InstanceType': instance_type, 'MinCount': 1, 'MaxCount': 1,
               'KeyName': name, 'InstanceInitiatedShutdownBehavior': 'terminate',
               'IamInstanceProfile': {'Name': name},
               'InstanceMarketOptions': {'MarketType': 'spot', 'SpotOptions': {'SpotInstanceType': 'one-time',
                   'MaxPrice': str(price_cap), 'InstanceInterruptionBehavior': 'terminate'}},
               'NetworkInterfaces': [{'DeviceIndex': 0, 'SubnetId': zones[price['AvailabilityZone']]['SubnetId'],
                                      'Groups': [group], 'AssociatePublicIpAddress': True}],
               'BlockDeviceMappings': [{'DeviceName': image['RootDeviceName'], 'Ebs': {'VolumeSize': 24,
                                        'VolumeType': 'gp3', 'DeleteOnTermination': True}}],
               'TagSpecifications': [{'ResourceType': r, 'Tags': tags} for r in ['instance', 'volume']],
               'MetadataOptions': {'HttpTokens': 'required', 'HttpEndpoint': 'enabled'}}
        # Maximum compute liability at price cap and hard deadline < $0.84;
        # reserve $2 more for EBS, IPv4 and transfer, below the $10 total cap.
        assert settings['priorSpendUsd'] + price_cap * 100 / 60 + 2 < plan['aws']['capUsdTotal']
        self.save('launch-request', req)
        launched = self.aws('run-instances', '--cli-input-json', 'file://' + str(self.root / 'launch-request.json'),
                            '--user-data', 'file://' + str(self.root / 'user-data.sh'), '--client-token', name)
        self.save('launched', launched)
        instance = launched['Instances'][0]
        self.state['instances'].append(instance['InstanceId'])
        self.state['launchedUtc'] = instance['LaunchTime']
        if instance.get('SpotInstanceRequestId'):
            self.state['spots'].append(instance['SpotInstanceRequestId'])
        self.save('state', self.state)
        self.log(f'One {settings["vcpus"]}-core Spot instance launched in a new pool; 100-minute hard shutdown, price cap ${price_cap:.2f}/hour')
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            item = self.aws('describe-instances', '--instance-ids', instance['InstanceId'])['Reservations'][0]['Instances'][0]
            if item['State']['Name'] in ['shutting-down', 'terminated', 'stopping', 'stopped']:
                raise RuntimeError('Instance ended before readiness')
            if item.get('PublicIpAddress'):
                self.state['public'] = item['PublicIpAddress']
                self.save('state', self.state)
                if self.ssh('test -f /var/tmp/study-bootstrap-done', check=False).returncode == 0:
                    break
            time.sleep(10)
        else:
            raise RuntimeError('Instance readiness timeout')
        timer = self.ssh('sudo cat /run/systemd/shutdown/scheduled')
        if 'USEC=' not in timer:
            raise RuntimeError('Hard shutdown timer missing')
        (self.root / 'shutdown-verification.txt').write_text(timer)
        self.ssh('mkdir -p /home/ubuntu/study/scripts /home/ubuntu/study/raw')
        with tarfile.open(self.root / 'source.tar.gz', 'w:gz') as bundle:
            for revision in ['A', 'B']:
                archive = self.root / (revision + '.tar')
                self.git('archive', '--format=tar', '-o', str(archive), revisions[revision])
                bundle.add(archive, arcname=revision + '.tar')
            for name in ['host_qualification.rs', 'run_trial.py', 'analyze.py', 'remote_job.py', 'autonomous_run.py', 'plan.json', 'attempt5-amendment.json']:
                bundle.add(HERE / name, arcname='scripts/' + name)
        self.scp(self.root / 'source.tar.gz', '/home/ubuntu/study/source.tar.gz')
        self.scp(self.root / 'handoff.json', '/home/ubuntu/study/handoff.json')
        self.ssh('cd /home/ubuntu/study && tar xzf source.tar.gz && uname -sr > environment.txt && lscpu >> environment.txt && for R in A B; do mkdir "$R"; tar xf "$R.tar" -C "$R"; cp scripts/host_qualification.rs "$R/examples/host_qualification.rs"; done')
        self.log('Shutdown timer verified; pinned A/B sources uploaded')

    def autonomous(self):
        command = ['python3', '/home/ubuntu/study/scripts/remote_job.py', 'launch', '/home/ubuntu/study/jobs/sequence',
                   'uv', 'run', '--no-project', '--with', 'boto3==1.43.118', 'python', '/home/ubuntu/study/scripts/autonomous_run.py']
        self.ssh(shlex.join(command))
        self.log('Whole build/ABAB sequence detached; progress and raw archive use private S3, no further SSH needed')
        deadline = time.monotonic() + 85 * 60
        last_phase = None
        while time.monotonic() < deadline:
            target = self.root / 'progress.json'
            try:
                self.service('s3api', 'get-object', '--bucket', self.state['bucket'], '--key', 'progress.json', str(target))
                progress = json.loads(target.read_text())
                if progress['phase'] != last_phase:
                    self.log('Instance sequence phase: ' + progress['phase'])
                    last_phase = progress['phase']
                self.state['completedSlots'] = progress['completedSlots']
                self.state['trialAttempts'] = progress['trialAttempts']
                self.save('state', self.state)
                for slot, summary in progress.get('summaries', {}).items():
                    if not (self.root / (slot + '-summary.json')).exists():
                        self.save(slot + '-summary', summary)
                        self.log(f'{slot}: {summary["ticksPerSecond"]:.3f} ticks/s, {summary["deferredTicks"]} deferred, '
                                 f'sim CPU {summary["simulationCpuPercentOneCore"]:.2f}%, '
                                 f'bots {summary["botCpuCores"]:.3f} cores, excess p99 {summary["excessMs"]["p99"]:.3f} ms')
                if progress['phase'] == 'finished':
                    archive = self.root / 'results.tar.gz'
                    self.service('s3api', 'get-object', '--bucket', self.state['bucket'], '--key', 'results.tar.gz', str(archive))
                    if hashlib.sha256(archive.read_bytes()).hexdigest() != progress['archive']['sha256']:
                        raise ValueError('S3 archive hash mismatch')
                    self.save('raw-archive', {**progress['archive'], 'access': 'local-only'})
                    target = self.root / 'collected'
                    target.mkdir(exist_ok=False)
                    with tarfile.open(archive) as bundle:
                        bundle.extractall(target, filter='data')
                    self.log('Autonomous archive downloaded from private S3 and SHA-256 verified')
                    if progress['state'] != 'completed':
                        raise ValueError('Autonomous sequence failed; retained archive')
                    return
            except RuntimeError:
                # AWS read may race the first progress write or a short API outage.
                pass
            current = self.aws('describe-instances', '--instance-ids', *self.state['instances'])
            instance = current['Reservations'][0]['Instances'][0]
            if instance['State']['Name'] in ['terminated', 'shutting-down', 'stopped', 'stopping']:
                self.save('termination-detail', current)
                raise ValueError('Instance ended before complete archive; inspect private AWS reason')
            time.sleep(90)
        raise RuntimeError('Autonomous handoff exceeded 85-minute deadline; cleaning up before hard timer')




    def cleanup(self):
        self.log('Terminating only resources owned by this study; checking four regions')
        checks = {}
        for region in REGIONS:
            tagged = self.aws('describe-instances', '--filters', 'Name=tag:Task,Values=' + self.state['tag'], region=region)
            instances = [i for r in tagged['Reservations'] for i in r['Instances']]
            ids = [i['InstanceId'] for i in instances if i['State']['Name'] != 'terminated']
            spots = {i['SpotInstanceRequestId'] for i in instances if i.get('SpotInstanceRequestId')}
            if region == 'eu-central-1':
                spots.update(self.state['spots'])
            if ids:
                self.aws('terminate-instances', '--instance-ids', *ids, region=region)
                self.command(['aws', 'ec2', 'wait', 'instance-terminated', '--instance-ids', *ids,
                              '--region', region], timeout=600)
            if spots:
                self.aws('cancel-spot-instance-requests', '--spot-instance-request-ids', *sorted(spots), region=region)
            groups = self.aws('describe-security-groups', '--filters', 'Name=tag:Task,Values=' + self.state['tag'], region=region)['SecurityGroups']
            for group in groups:
                self.aws('delete-security-group', '--group-id', group['GroupId'], region=region)
            if region == 'eu-central-1' and self.state.get('keyName'):
                self.aws('delete-key-pair', '--key-name', self.state['keyName'], region=region)
            after_instances = self.aws('describe-instances', '--filters', 'Name=tag:Task,Values=' + self.state['tag'], region=region)
            volumes = self.aws('describe-volumes', '--filters', 'Name=tag:Task,Values=' + self.state['tag'], region=region)['Volumes']
            after_groups = self.aws('describe-security-groups', '--filters', 'Name=tag:Task,Values=' + self.state['tag'], region=region)['SecurityGroups']
            after_spots = self.aws('describe-spot-instance-requests', region=region)['SpotInstanceRequests']
            own_spots = [s for s in after_spots if s['SpotInstanceRequestId'] in spots]
            keys = self.aws('describe-key-pairs', region=region)['KeyPairs']
            checks[region] = {
                'nonterminatedInstances': sum(i['State']['Name'] != 'terminated' for r in after_instances['Reservations'] for i in r['Instances']),
                'openOrActiveSpotRequests': sum(s['State'] in ['open', 'active'] for s in own_spots),
                'volumes': len(volumes), 'securityGroups': len(after_groups),
                'keyPairs': sum(k['KeyName'] == self.state.get('keyName') for k in keys),
            }
            self.save('cleanup-private-' + region, {'instances': after_instances, 'spots': own_spots, 'volumes': volumes, 'groups': after_groups})
        self.save('cleanup-verification', checks)
        if any(v for region in checks.values() for v in region.values()):
            raise RuntimeError('Cleanup verification nonzero; inspect private evidence')
        if self.state.get('bucket'):
            bucket = self.state['bucket']
            objects = self.service('s3api', 'list-objects-v2', '--bucket', bucket).get('Contents', [])
            for obj in objects:
                self.service('s3api', 'delete-object', '--bucket', bucket, '--key', obj['Key'])
            self.service('s3api', 'delete-bucket', '--bucket', bucket)
            name = self.state['role']
            self.service('iam', 'remove-role-from-instance-profile', '--instance-profile-name', name, '--role-name', name)
            self.service('iam', 'delete-instance-profile', '--instance-profile-name', name)
            self.service('iam', 'delete-role-policy', '--role-name', name, '--policy-name', 'study-handoff')
            self.service('iam', 'delete-role', '--role-name', name)
            self.save('handoff-cleanup', {'bucketDeleted': True, 'instanceProfileDeleted': True, 'roleDeleted': True})
        self.state['cleanupUtc'] = dt.datetime.now(dt.timezone.utc).isoformat()
        if self.state.get('launchedUtc'):
            duration = (dt.datetime.now(dt.timezone.utc) - dt.datetime.fromisoformat(self.state['launchedUtc'].replace('Z', '+00:00'))).total_seconds() / 3600
            self.state['billedHoursUpperBound'] = duration
            self.state['estimatedSpendUsd'] = duration * (self.state.get('hourlySpotPrice', .5) + .005 + 24 * .096 / 730)
            self.state['computeCapEstimateUsd'] = duration * self.state.get('launchAws', {}).get('maxPriceUsdHour', .5)
        self.save('state', self.state)
        self.log('Cleanup verified: zero study instances, active Spot requests, volumes, security groups or keys in all four regions')

    def run(self):
        try:
            self.launch()
            self.autonomous()
        except BaseException as error:
            self.save('failure', {'type': type(error).__name__, 'error': str(error)})
            if self.state.get('public') and not self.state.get('bucket'):
                try:
                    self.collect(partial=True)
                except Exception:
                    pass
            raise
        finally:
            self.cleanup()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['prepare', 'run', 'cleanup'])
    parser.add_argument('scratch', type=Path)
    parser.add_argument('--repo', type=Path, default=Path.cwd())
    args = parser.parse_args()
    getattr(Study(args), args.action)()
