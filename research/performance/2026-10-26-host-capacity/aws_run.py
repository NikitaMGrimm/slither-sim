"""Bounded AWS study controller. Run on operator machine; all identifying data stays in scratch.

Requires AWS CLI login, OpenSSH, Git and Python 3.12+. Never commits cloud metadata.
Resources are tagged, price capped and boot-timed to self-terminate after 150 minutes.
"""
import argparse
import datetime as dt
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import time
import urllib.request

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from prepare import prepare

class Study:
    def __init__(self, args):
        self.args = args
        self.root = args.scratch.resolve()
        self.aws_bin = args.aws
        if args.resume:
            self.state=json.loads((self.root/'state.json').read_text())
            return
        self.root.mkdir(exist_ok=False, parents=True)
        self.state = {'tag': 'host-capacity-20261026', 'instances': {}, 'groups': {}, 'spots': []}
        self.save('state', self.state)

    def save(self, name, value):
        (self.root/(name+'.json')).write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8')

    def log(self, text):
        print(dt.datetime.now(dt.timezone.utc).isoformat(), text, flush=True)

    def aws(self, service, *args):
        result = subprocess.run([self.aws_bin,service,*args,'--region','us-east-1' if service=='freetier' else self.args.region,
                                 '--output','json','--no-cli-pager'],capture_output=True,text=True,timeout=120)
        if result.returncode:
            self.save('last-aws-error', {'action':service+' '+args[0], 'stderr':result.stderr})
            raise RuntimeError('AWS '+service+' '+args[0]+' failed; private details in scratch')
        return json.loads(result.stdout) if result.stdout.strip() else {}

    def ec2(self, *args): return self.aws('ec2',*args)

    def update(self): self.save('state',self.state)

    def ssh(self, role, command, check=True, timeout=90):
        result = subprocess.run(['ssh','-i',str(self.root/'study.pem'),'-o','StrictHostKeyChecking=accept-new',
                                 '-o','UserKnownHostsFile='+str(self.root/'known_hosts'),'-o','ConnectTimeout=10',
                                 '-o','ServerAliveInterval=15','ubuntu@'+self.state['instances'][role]['public'],command],
                                capture_output=True,text=True,timeout=timeout)
        if check and result.returncode:
            (self.root/'last-ssh-error.txt').write_text(result.stdout+'\n'+result.stderr,encoding='utf-8')
            raise RuntimeError('SSH '+role+' failed; private details in scratch')
        return result

    def scp(self, role, local, remote, download=False):
        endpoint='ubuntu@'+self.state['instances'][role]['public']+':'+remote
        cmd=['scp','-i',str(self.root/'study.pem'),'-o','StrictHostKeyChecking=accept-new',
             '-o','UserKnownHostsFile='+str(self.root/'known_hosts'),'-o','ConnectTimeout=10']
        result=subprocess.run(cmd+([endpoint,str(local)] if download else [str(local),endpoint]),capture_output=True,text=True,timeout=600)
        if result.returncode:
            (self.root/'last-scp-error.txt').write_text(result.stderr,encoding='utf-8')
            raise RuntimeError('SCP '+role+' failed; private details in scratch')

    def launch_job(self,role,script,name):
        # Detach every stdio descriptor, including stdin, to release the SSH channel.
        code=f'import subprocess; subprocess.Popen(["bash","-lc",{script!r}],stdin=subprocess.DEVNULL,stdout=open("/home/ubuntu/study/{name}.log","w"),stderr=subprocess.STDOUT,start_new_session=True)'
        import shlex
        self.ssh(role,'python3 -c '+shlex.quote(code))

    def wait_file(self,role,path,seconds):
        deadline=time.monotonic()+seconds
        while time.monotonic()<deadline:
            r=self.ssh(role,'test -f '+path,check=False)
            if r.returncode==0: return
            time.sleep(10)
        raise RuntimeError(role+' timed out waiting for '+path)

    def gate(self):
        deadline=time.monotonic()+7200
        while True:
            found=self.ec2('describe-instances')
            self.save('prelaunch-instances',found)
            active=[i for r in found['Reservations'] for i in r['Instances'] if i['State']['Name'] in ['pending','running','stopping','shutting-down']]
            if not active: break
            if time.monotonic()>=deadline: raise RuntimeError('Account still occupied after two hours; no launch')
            self.log('Launch gated: account has '+str(len(active))+' active instance(s)')
            time.sleep(45)

    def provision(self):
        self.save('credits-before',self.aws('freetier','get-account-plan-state'))
        self.gate()
        subprocess.run(['git','fetch','origin'],cwd=self.args.repo,check=True)
        self.state['sourceCommit']=subprocess.check_output(['git','rev-parse','origin/main'],cwd=self.args.repo,text=True).strip()
        self.save('pr-states', {str(n):json.loads(subprocess.check_output(['gh','pr','view',str(n),'--json','state,headRefOid'],cwd=self.args.repo,text=True)) for n in [111,112]})
        source=prepare(self.args.repo,self.root/'build','origin/main')
        bundle=self.root/'source.tar.gz'
        with tarfile.open(bundle,'w:gz') as tar:
            tar.add(source,arcname='source')
            for p in HERE.iterdir():
                if p.is_file(): tar.add(p,arcname='scripts/'+p.name)
        vpcs=self.ec2('describe-vpcs')['Vpcs']
        defaults=[v for v in vpcs if v.get('IsDefault')]
        if len(defaults)!=1: raise RuntimeError('Expected one default VPC')
        vpc=defaults[0]['VpcId']
        subnets=self.ec2('describe-subnets')['Subnets']
        prices=self.ec2('describe-spot-price-history','--instance-types','c7a.medium','c7a.4xlarge','--product-descriptions','Linux/UNIX','--start-time',dt.datetime.now(dt.timezone.utc).isoformat())
        self.save('prices-before',prices)
        zones={s['AvailabilityZone']:s for s in subnets if s['VpcId']==vpc and s.get('DefaultForAz')}
        def cost(zone): return sum(float(next(p['SpotPrice'] for p in prices['SpotPriceHistory'] if p['AvailabilityZone']==zone and p['InstanceType']==typ)) for typ in ['c7a.medium','c7a.4xlarge'])
        zone=min(zones,key=cost); subnet=zones[zone]
        images=self.ec2('describe-images','--owners','099720109477','--filters',json.dumps([{'Name':'name','Values':['ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*']},{'Name':'state','Values':['available']}]))['Images']
        image=max(images,key=lambda i:i['CreationDate'])
        ip=urllib.request.urlopen('https://checkip.amazonaws.com',timeout=20).read().decode().strip(); ipaddress.IPv4Address(ip)
        name=self.state['tag']+'-'+dt.datetime.now(dt.timezone.utc).strftime('%H%M%S')
        self.state.update(keyName=name,launchUTC=dt.datetime.now(dt.timezone.utc).isoformat(),zone=zone)
        self.update()
        tags=[{'Key':'Project','Value':'slither-sim'},{'Key':'Task','Value':self.state['tag']},{'Key':'ExpiresAt','Value':(dt.datetime.now(dt.timezone.utc)+dt.timedelta(minutes=150)).isoformat()}]
        for role in ['server','generator']:
            group=self.ec2('create-security-group','--group-name',name+'-'+role,'--description','Temporary capacity study','--vpc-id',vpc,'--tag-specifications',json.dumps([{'ResourceType':'security-group','Tags':tags}]))['GroupId']
            self.state['groups'][role]=group; self.update()
            self.ec2('authorize-security-group-ingress','--group-id',group,'--protocol','tcp','--port','22','--cidr',ip+'/32')
        self.ec2('authorize-security-group-ingress','--group-id',self.state['groups']['server'],'--ip-permissions',json.dumps([{'IpProtocol':'tcp','FromPort':8766,'ToPort':8766,'UserIdGroupPairs':[{'GroupId':self.state['groups']['generator']}]}]))
        key=self.ec2('create-key-pair','--key-name',name,'--key-type','ed25519')
        (self.root/'study.pem').write_text(key.pop('KeyMaterial')+'\n',encoding='ascii'); self.save('key-metadata',key)
        if os.name=='nt':
            subprocess.run(['icacls',str(self.root/'study.pem'),'/inheritance:r','/grant:r',subprocess.check_output(['whoami'],text=True).strip()+':R'],capture_output=True,check=True)
        else: os.chmod(self.root/'study.pem',0o600)
        userdata='#!/bin/bash\nshutdown -P +150\nexport DEBIAN_FRONTEND=noninteractive\napt-get update\napt-get install -y build-essential pkg-config linux-tools-generic linux-tools-common chrony\ntouch /var/tmp/study-bootstrap-done\n'
        (self.root/'user-data.sh').write_text(userdata,encoding='utf-8',newline='\n')
        # Worst-case compute: (0.06+0.65)*2.5=$1.775; reserve >$3 for storage/IP/transfer.
        assert (.06+.65)*2.5 + 16*.12 + .25 < 5
        for role,typ,maxprice in [('generator','c7a.4xlarge','0.65'),('server','c7a.medium','0.06')]:
            req={'ImageId':image['ImageId'],'InstanceType':typ,'MinCount':1,'MaxCount':1,'KeyName':name,
                 'InstanceInitiatedShutdownBehavior':'terminate','InstanceMarketOptions':{'MarketType':'spot','SpotOptions':{'SpotInstanceType':'one-time','MaxPrice':maxprice,'InstanceInterruptionBehavior':'terminate'}},
                 'NetworkInterfaces':[{'DeviceIndex':0,'SubnetId':subnet['SubnetId'],'Groups':[self.state['groups'][role]],'AssociatePublicIpAddress':True}],
                 'BlockDeviceMappings':[{'DeviceName':image['RootDeviceName'],'Ebs':{'VolumeSize':24,'VolumeType':'gp3','DeleteOnTermination':True}}],
                 'TagSpecifications':[{'ResourceType':r,'Tags':tags} for r in ['instance','volume']],
                 'MetadataOptions':{'HttpTokens':'required','HttpEndpoint':'enabled'}}
            self.save('launch-'+role,req)
            for attempt in range(10):
                try:
                    launch=self.ec2('run-instances','--cli-input-json','file://'+str(self.root/('launch-'+role+'.json')),'--user-data','file://'+str(self.root/'user-data.sh'),'--client-token',name+'-'+role)
                    break
                except RuntimeError:
                    error=json.loads((self.root/'last-aws-error.json').read_text())['stderr']
                    if 'MaxSpotInstanceCountExceeded' not in error or attempt==9: raise
                    self.log('Spot quota accounting has not released terminated capacity; retrying same idempotent request')
                    time.sleep(45)
            self.save('launched-'+role,launch)
            inst=launch['Instances'][0]; self.state['instances'][role]={'id':inst['InstanceId'],'type':typ,'launch':inst['LaunchTime']}
            if inst.get('SpotInstanceRequestId'): self.state['spots'].append(inst['SpotInstanceRequestId'])
            self.update()
        self.log('Launched requested native Linux pair; self-termination maximum 150 minutes')
        for role in ['generator','server']:
            for _ in range(60):
                inst=self.ec2('describe-instances','--instance-ids',self.state['instances'][role]['id'])['Reservations'][0]['Instances'][0]
                if inst['State']['Name'] in ['terminated','shutting-down','stopped','stopping']: raise RuntimeError(role+' ended before readiness')
                if inst.get('PublicIpAddress'):
                    self.state['instances'][role].update(public=inst['PublicIpAddress'],private=inst['PrivateIpAddress']); self.update()
                    if self.ssh(role,'true',check=False).returncode==0: break
                time.sleep(10)
            else: raise RuntimeError(role+' SSH readiness timeout')
            timer=self.ssh(role,'sudo cat /run/systemd/shutdown/scheduled').stdout
            if 'USEC=' not in timer: raise RuntimeError('Missing hard shutdown timer')
            (self.root/(role+'-shutdown.txt')).write_text(timer)
            self.wait_file(role,'/var/tmp/study-bootstrap-done',900)
            self.ssh(role,'mkdir -p /home/ubuntu/study')
            self.scp(role,bundle,'/home/ubuntu/study/source.tar.gz')
            self.ssh(role,'cd /home/ubuntu/study && tar xzf source.tar.gz && uname -a > environment.txt && lscpu >> environment.txt && ulimit -n >> environment.txt && chronyc tracking >> environment.txt')
        build='cd /home/ubuntu/study; set -e; curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.99.0; source "$HOME/.cargo/env"; cd source; CARGO_BUILD_JOBS=8 CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --locked --bin slither-sim --example host_probe --example host_capacity_load; cd ..; mkdir bin; cp source/target/release/examples/host_probe source/target/release/examples/host_capacity_load source/target/release/slither-sim bin/; rustc -Vv > toolchain.txt; sha256sum bin/* > binaries.sha256; touch BUILD-DONE'
        self.launch_job('generator',build,'build')
        self.wait_file('generator','/home/ubuntu/study/BUILD-DONE',1200)
        binary_tar=self.root/'binaries.tar.gz'
        self.ssh('generator','cd /home/ubuntu/study && tar czf binaries.tar.gz bin toolchain.txt binaries.sha256')
        self.scp('generator',binary_tar,'/home/ubuntu/study/binaries.tar.gz',True)
        self.scp('server',binary_tar,'/home/ubuntu/study/binaries.tar.gz')
        self.ssh('server','cd /home/ubuntu/study && tar xzf binaries.tar.gz && sha256sum -c binaries.sha256')
        self.log('Pinned release binaries built once with line-table debuginfo; hashes matched on server')

    def trial(self,name,count,hold=120,bots=0,profile=False):
        epoch=time.time()+8
        common=f'--warmup 15 --hold {hold} --epoch {epoch}'
        role='generator' if bots else 'server'
        host=f'cd /home/ubuntu/study; python3 scripts/run_trial.py host raw/{name}-host --count {1 if bots else count} --bots {bots} {common}'
        if profile:
            perf=self.ssh(role,"find -L /usr/lib/linux-tools -name perf -type f | sort | tail -1").stdout.strip()
            if not perf: raise RuntimeError('No perf binary after bootstrap')
            host+=' --profile '+perf
        host+=f' && touch {name}-host-DONE'
        self.launch_job(role,host,name+'-host')
        endpoint='127.0.0.1' if bots else self.state['instances']['server']['private']
        load=f'cd /home/ubuntu/study; python3 scripts/run_trial.py load raw/{name}-load --count {count} --server {endpoint} {common}'+(' --watcher --shards 1' if bots else '')+f' && touch {name}-load-DONE'
        self.launch_job('generator',load,name+'-load')
        self.log(f'Running {name}: {count} socket client(s), {bots} bots, 15s warmup + {hold}s hold')
        return self.finish_trial(name,count,hold,bots,profile)

    def finish_trial(self,name,count,hold=120,bots=0,profile=False):
        role='generator' if bots else 'server'
        self.wait_file(role,f'/home/ubuntu/study/{name}-host-DONE',hold+100)
        self.wait_file('generator',f'/home/ubuntu/study/{name}-load-DONE',90)
        local=self.root/name; local.mkdir()
        for filename in ['host.json','telemetry.json']:
            self.scp(role,local/filename,f'/home/ubuntu/study/raw/{name}-host/{filename}',True)
        if not bots:
            self.ssh('generator',f'mkdir -p /home/ubuntu/study/raw/{name}-host')
            for filename in ['host.json','telemetry.json']:
                self.scp('generator',local/filename,f'/home/ubuntu/study/raw/{name}-host/{filename}')
        self.ssh('generator',f'cd /home/ubuntu/study && python3 scripts/analyze.py raw/{name}-host raw/{name}-load {count} raw/{name}-summary.json',timeout=180)
        self.scp('generator',local/'summary.json',f'/home/ubuntu/study/raw/{name}-summary.json',True)
        summary=json.loads((local/'summary.json').read_text())
        self.log(f"{name}: playing={summary['clientsTimeWeighted']['playing']:.3f}, ticks/s={summary['ticksPerSecond']:.3f}, deferred={summary['deferredTicks']}, excess p99={summary['excessMs']['p99']:.3f} ms")
        self.state.setdefault('trials',[]).append({'name':name,'count':count,'bots':bots,'profile':profile,'summary':summary}); self.update()
        return summary

    def native_smoke(self):
        epoch=time.time()+5
        for role in ['host','load']:
            cmd=f'cd /home/ubuntu/study; python3 scripts/run_trial.py {role} raw/native-smoke-{role} --count 50 --shards 2 --warmup 3 --hold 8 --epoch {epoch} && touch native-smoke-{role}-DONE'
            self.launch_job('generator',cmd,'native-smoke-'+role)
        for role in ['host','load']:
            self.wait_file('generator',f'/home/ubuntu/study/native-smoke-{role}-DONE',60)
        self.ssh('generator','cd /home/ubuntu/study && python3 scripts/analyze.py raw/native-smoke-host raw/native-smoke-load 50 raw/native-smoke-summary.json',timeout=60)
        self.scp('generator',self.root/'native-smoke-summary.json','/home/ubuntu/study/raw/native-smoke-summary.json',True)
        result=json.loads((self.root/'native-smoke-summary.json').read_text())
        if result.get('errors') or result['clientsTimeWeighted']['playing']<49.99 or result['deferredTicks']:
            raise RuntimeError('Native end-to-end sampler smoke failed')
        self.log('Native 50-client end-to-end telemetry smoke passed')

    def collect(self):
        # Resolve stacks while the exact remote binaries and their debug tables exist.
        self.ssh('server', 'cd /home/ubuntu/study; sudo chown -R ubuntu:ubuntu raw; PERF=$(find -L /usr/lib/linux-tools -name perf -type f | sort | tail -1); '
                 'for DATA in raw/*/perf.data; do test -f "$DATA" || continue; DIR=$(dirname "$DATA"); '
                 '"$PERF" report -i "$DATA" --stdio --no-children --percent-limit 0 --sort symbol --call-graph none > "$DIR/perf-self.txt"; '
                 '"$PERF" report -i "$DATA" --stdio --children --percent-limit 0 --sort symbol --call-graph none > "$DIR/perf-inclusive.txt"; '
                 '"$PERF" script -i "$DATA" > "$DIR/perf.stacks"; done', timeout=300)
        for role in ['server','generator']:
            self.ssh(role,'cd /home/ubuntu/study && sudo chown -R ubuntu:ubuntu raw && tar czf results.tar.gz raw *.log *.txt *.sha256 scripts && sha256sum results.tar.gz > results.tar.gz.sha256',timeout=300)
            size=int(self.ssh(role,'stat -c %s /home/ubuntu/study/results.tar.gz').stdout)
            if size>8*1024**3: raise RuntimeError('Download exceeds conservative transfer budget')
            target=self.root/(role+'-results.tar.gz')
            self.scp(role,target,'/home/ubuntu/study/results.tar.gz',True)
            digest=self.ssh(role,'cat /home/ubuntu/study/results.tar.gz.sha256').stdout.split()[0]
            if hashlib.file_digest(target.open('rb'),'sha256').hexdigest()!=digest: raise RuntimeError('Archive hash mismatch')
            self.save(role+'-archive',{'sha256':digest,'bytes':size})
            with tarfile.open(target) as tar: tar.extractall(self.root/role,filter='data')
        self.log('Raw archives returned and SHA-256 verified')

    def build_fp(self):
        self.save('frame-pointer-plan',{'sourceCommit':self.state['sourceCommit'],'flags':'CARGO_PROFILE_RELEASE_DEBUG=1 RUSTFLAGS=-C force-frame-pointers=yes','capacityBinaryUnchanged':True})
        if self.ssh('generator','test -f /home/ubuntu/study/build-fp.log',check=False).returncode:
            script='cd /home/ubuntu/study; set -e; source "$HOME/.cargo/env"; cd source; CARGO_BUILD_JOBS=8 CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_TARGET_DIR=target-fp RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release --locked --example host_probe; cd ..; mkdir -p bin-fp; cp source/target-fp/release/examples/host_probe bin-fp/host_probe; sha256sum bin-fp/host_probe > binaries-fp.sha256; touch BUILD-FP-DONE'
            self.launch_job('generator',script,'build-fp')
        self.wait_file('generator','/home/ubuntu/study/BUILD-FP-DONE',1200)
        self.ssh('generator','cd /home/ubuntu/study && tar czf binaries-fp.tar.gz bin-fp binaries-fp.sha256')
        target=self.root/'binaries-fp.tar.gz'
        self.scp('generator',target,'/home/ubuntu/study/binaries-fp.tar.gz',True)
        self.scp('server',target,'/home/ubuntu/study/binaries-fp.tar.gz')
        self.ssh('server','cd /home/ubuntu/study && tar xzf binaries-fp.tar.gz && sha256sum -c binaries-fp.sha256')
        perf=self.ssh('server','find -L /usr/lib/linux-tools -name perf -type f | sort | tail -1').stdout.strip()
        probe=self.ssh('server','sudo '+perf+' stat -e cycles -- true',check=False)
        self.save('hardware-event-probe',{'exitCode':probe.returncode,'stdout':probe.stdout,'stderr':probe.stderr})
        self.state['profileEvent']='cycles' if probe.returncode==0 else 'cpu-clock'; self.update()
        import shlex
        clock='import time,json; a=time.time(); m=time.monotonic(); b=time.time(); print(json.dumps({"unixMinusMonotonic":(a+b)/2-m,"readUncertaintySeconds":(b-a)/2}))'
        self.save('server-clock-bridge',json.loads(self.ssh('server','python3 -c '+shlex.quote(clock)).stdout))
        self.log('Frame-pointer profile binary transferred and hash verified; capacity binary unchanged')

    def profile_fp(self,count):
        name='profile-fp-'+str(count)
        epoch=time.time()+8
        common=f'--warmup 15 --hold 120 --epoch {epoch}'
        perf=self.ssh('server','find -L /usr/lib/linux-tools -name perf -type f | sort | tail -1').stdout.strip()
        event=self.state.get('profileEvent','cycles')
        self.launch_job('server',f'cd /home/ubuntu/study; python3 scripts/run_trial.py host raw/{name}-host --bin ./bin-fp --count {count} {common} --profile {perf} --event {event} --call-graph fp && touch {name}-host-DONE',name+'-host')
        self.launch_job('generator',f'cd /home/ubuntu/study; python3 scripts/run_trial.py load raw/{name}-load --count {count} --server '+self.state['instances']['server']['private']+f' {common} && touch {name}-load-DONE',name+'-load')
        self.log(f'Running {name}: 60-second cycle profile with frame-pointer call chains')
        return self.finish_trial(name,count,profile=True)

    def cleanup(self):
        self.log('Cleanup: terminating study resources')
        # Discover by tag as well as saved IDs, covering a lost launch response.
        found=self.ec2('describe-instances','--filters','Name=tag:Task,Values='+self.state['tag'])
        ids=[i['InstanceId'] for r in found['Reservations'] for i in r['Instances'] if i['State']['Name']!='terminated']
        if ids:
            self.ec2('terminate-instances','--instance-ids',*ids)
            subprocess.run([self.aws_bin,'ec2','wait','instance-terminated','--instance-ids',*ids,'--region',self.args.region],capture_output=True,check=True,timeout=600)
        spots=self.ec2('describe-spot-instance-requests')['SpotInstanceRequests']
        own=set(self.state['spots'])
        own.update(s['SpotInstanceRequestId'] for s in spots if s.get('InstanceId') in ids)
        if own:
            self.ec2('cancel-spot-instance-requests','--spot-instance-request-ids',*sorted(own))
            # EC2 can briefly return the pre-cancellation request state.
            for attempt in range(12):
                current=self.ec2('describe-spot-instance-requests','--spot-instance-request-ids',*sorted(own))['SpotInstanceRequests']
                if all(s['State'] not in ['open','active'] for s in current): break
                if attempt==11: raise RuntimeError('Spot cancellation did not converge')
                time.sleep(5)
        if self.state.get('keyName'): self.ec2('delete-key-pair','--key-name',self.state['keyName'])
        # Revoke the cross-group reference before deleting either group.
        groups=self.state['groups']
        if set(groups)=={'server','generator'}:
            self.ec2('revoke-security-group-ingress','--group-id',groups['server'],'--ip-permissions',json.dumps([{'IpProtocol':'tcp','FromPort':8766,'ToPort':8766,'UserIdGroupPairs':[{'GroupId':groups['generator']}]}]))
        for group in groups.values():
            for attempt in range(12):
                try: self.ec2('delete-security-group','--group-id',group); break
                except RuntimeError:
                    if attempt==11: raise
                    time.sleep(5)
        checks={}
        for name,cmd in [('instances',('describe-instances',)),('volumes',('describe-volumes',)),('addresses',('describe-addresses',)),('snapshots',('describe-snapshots','--owner-ids','self')),('spots',('describe-spot-instance-requests',)),('keys',('describe-key-pairs',)),('groups',('describe-security-groups',))]:
            checks[name]=self.ec2(*cmd); self.save('after-'+name,checks[name])
        # The unfiltered request inventory can lag the ID-filtered response too.
        for attempt in range(12):
            if not any(s['State'] in ['open','active'] for s in checks['spots']['SpotInstanceRequests']): break
            time.sleep(5)
            checks['spots']=self.ec2('describe-spot-instance-requests'); self.save('after-spots',checks['spots'])
        self.save('credits-after',self.aws('freetier','get-account-plan-state'))
        result={'region':self.args.region,'nonterminatedInstances':sum(i['State']['Name']!='terminated' for r in checks['instances']['Reservations'] for i in r['Instances']),
                'volumes':len(checks['volumes']['Volumes']),'addresses':len(checks['addresses']['Addresses']),'snapshots':len(checks['snapshots']['Snapshots']),
                'openOrActiveSpotRequests':sum(s['State'] in ['open','active'] for s in checks['spots']['SpotInstanceRequests']),
                'keyPairDeleted':not any(k['KeyName']==self.state.get('keyName') for k in checks['keys']['KeyPairs']),
                'securityGroupsDeleted':not any(g['GroupId'] in groups.values() for g in checks['groups']['SecurityGroups'])}
        self.save('cleanup-verification',result)
        self.state['terminatedUTC']=dt.datetime.now(dt.timezone.utc).isoformat(); self.update()
        self.log('Cleanup verification '+json.dumps(result))
        if any(result[k] for k in ['nonterminatedInstances','volumes','addresses','snapshots','openOrActiveSpotRequests']) or not result['keyPairDeleted'] or not result['securityGroupsDeleted']:
            raise RuntimeError('Cleanup inventory not empty; inspect private evidence')

    def run(self):
        try:
            self.provision()
            self.native_smoke()
            passing=[]
            for count in [100,250,500,750]:
                s=self.trial('sweep-'+str(count),count)
                if s['criterionB']: passing.append(count)
            if 750 in passing:
                if self.trial('sweep-1000',1000)['criterionB']: passing.append(1000)
            confirmed=self.trial('confirm-'+str(max(passing)),max(passing),300) if passing else None
            self.trial('bots-500',1,bots=500)
            if confirmed and not confirmed['criterionB']:
                self.save('followup-plan',{'reason':'Largest sweep pass failed 300-second confirmation','additionalCounts':sorted(passing,reverse=True)[1:],'selection':'Next lower predeclared sweep count, stop at first long pass','frozenBeforeFollowup':True})
                for count in sorted(passing,reverse=True)[1:]:
                    candidate=self.trial('confirm-'+str(count),count,300)
                    if candidate['criterionB']:
                        confirmed=candidate
                        break
            self.build_fp()
            for count in sorted(set([500]+([max(passing)] if passing else [])+([confirmed['requested']] if confirmed and confirmed['criterionB'] else []))): self.profile_fp(count)
            self.collect()
        except BaseException as exc:
            self.save('failure',{'type':type(exc).__name__,'error':str(exc)})
            self.log('Study failed; details saved privately. Cleaning up.')
            # Retain partial evidence when an instance remains reachable.
            for role in self.state['instances']:
                try:
                    self.ssh(role,'cd /home/ubuntu/study && sudo chown -R ubuntu:ubuntu raw; tar czf partial.tar.gz raw *.log *.txt',check=False,timeout=120)
                    self.scp(role,self.root/(role+'-partial.tar.gz'),'/home/ubuntu/study/partial.tar.gz',True)
                except Exception: pass
            raise
        finally:
            self.cleanup()

    def resume(self):
        """Finish the recorded profiler correction on the existing instance pair."""
        try:
            names={t['name'] for t in self.state['trials']}
            assert {'confirm-100','bots-500','profile-100','profile-500'} <= names
            self.build_fp()
            for count in [500,100]:
                if 'profile-fp-'+str(count) not in names: self.profile_fp(count)
            self.collect()
        except BaseException as exc:
            self.save('resume-failure',{'type':type(exc).__name__,'error':str(exc)})
            raise
        finally:
            self.cleanup()

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('scratch',type=Path)
    p.add_argument('--repo',type=Path,default=Path.cwd())
    p.add_argument('--aws',default='aws')
    p.add_argument('--region',default='eu-central-1')
    p.add_argument('--resume',action='store_true',help='Resume the recorded attempt-3 follow-up on its existing resources')
    args=p.parse_args()
    study=Study(args)
    if args.resume: study.resume()
    else: study.run()
