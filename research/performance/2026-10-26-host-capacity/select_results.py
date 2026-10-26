"""Create a bounded, non-identifying result selection from private raw archives."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
from analyze import resources
import profile as study_profile

HERE=Path(__file__).resolve().parent

def digest(path):
    with path.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()

def main(raw, output):
    output.mkdir(parents=True,exist_ok=True)
    state=json.loads((raw/'state.json').read_text())
    trials={}
    profiles={}
    function_exports={}
    for trial in state['trials']:
        name=trial['name']
        # Source runtime selection was made by the frozen controller. Add only
        # covariates derivable from its exact retained resource boundaries.
        summary=copy.deepcopy(trial['summary'])
        host_root=raw/('generator' if trial['bots'] else 'server')/'raw'/(name+'-host')
        load_root=raw/'generator'/'raw'/(name+'-load')
        summary['hostResources']=resources(host_root/'telemetry.json')
        summary['generatorResources']=resources(load_root/'telemetry.json')
        for role in ['hostResources','generatorResources']:
            values=summary[role].pop('perProcessCpuPercentOneCore')
            summary[role]['perProcessCpuPercentOneCore']=list(values.values())
        if name=='sweep-100':
            summary['hostResources'].pop('threadCpuPercentOneCore')
            summary['generatorResources'].pop('threadCpuPercentOneCore')
            summary['threadCpuLimit']='Unavailable: initial sampler read group CPU per thread; corrected before sweep-250. Process CPU is valid.'
        summary['scenario']='500 bots and one watcher' if trial['bots'] else ('profiled sockets' if trial['profile'] else 'unprofiled sockets')
        if trial['profile']:
            summary.pop('perClient')  # Detailed diagnostic receipts remain raw.
        host_config=json.loads((host_root/'host.json').read_text())['config']
        host_config.pop('bind')
        summary['inputHostConfig']=host_config
        trials[name]=summary
        if trial['profile'] and name.startswith('profile-fp-'):
            stacks=host_root/'perf-period.stacks'
            bridge=json.loads((raw/'server-clock-bridge.json').read_text())['unixMinusMonotonic']
            timing=json.loads((host_root/'host.json').read_text())
            window=[timing['holdStartUnix']-bridge,timing['holdEndUnix']-bridge]
            profiles[name]=study_profile.reduce(stacks,window)
            profiles[name]['role']='primary hardware-cycle profile, separate frame-pointer release build'
            function_file=host_root/'perf-noinline-period.stacks'
            if not function_file.exists(): function_file=raw/'early-profiles'/(name+'-noinline.stacks')
            function_exports[name]=digest(function_file)
            named=study_profile.reduce(function_file,window)
            assert named['samples']==profiles[name]['samples'] and named['totalWeight']==profiles[name]['totalWeight']
            profiles[name]['topSelf']=named['topSelf']
            profiles[name]['topInclusive']=named['topInclusive']
            profiles[name]['topApplicationSelf']=named['topApplicationSelf']
            profiles[name]['topApplicationInclusive']=named['topApplicationInclusive']
            profiles[name]['symbolPresentation']='Function tables: --no-inline; detailed inline callchains determine categories. Both exports have identical event count and total period weight.'
        elif trial['profile']:
            summary['profileLimit']='Initial DWARF callchains incomplete; retained as diagnostic attempt, excluded from selected function/category shares.'
    sweep={k:v for k,v in trials.items() if k.startswith('sweep-')}
    passing_a=[v['requested'] for v in sweep.values() if v['criterionA']]
    passing_b=[v['requested'] for v in sweep.values() if v['criterionB']]
    slowing=[v['requested'] for v in sweep.values() if v['deferredTicks']>0]
    confirmations=[v for k,v in trials.items() if k.startswith('confirm-')]
    selected={'dataOrigin':'local-simulator','sourceCommit':state['sourceCommit'],
              'thresholds':{'aLargestSweepN':max(passing_a,default=None),'bLargestSweepN':max(passing_b,default=None),
                            'cFirstSweepNWithDeferredTicks':min(slowing,default=None),
                            'aLargestConfirmedN':max((v['requested'] for v in confirmations if v['criterionA']),default=None),
                            'bLargestConfirmedN':max((v['requested'] for v in confirmations if v['criterionB']),default=None)},
              'trials':trials,'profiles':profiles,
              'limits':'Single source pin, seed and instance pair. Exact nearest-rank receipt quantiles over correlated samples, with one-minute CPU profiles. No product capacity, mature-world, clustered, boosting or slow-reader claim.'}
    (output/'selected-results.json').write_text(json.dumps(selected,indent=2)+'\n',encoding='utf-8',newline='\n')
    pins=json.loads((raw/'build/source-pins.json').read_text())
    machines={}
    for role in ['server','generator']:
        environment=(raw/role/'environment.txt').read_text()
        allowed=['Architecture','CPU(s)','Model name','Thread(s) per core','Core(s) per socket']
        fields={line.split(':',1)[0].strip():line.split(':',1)[1].strip() for line in environment.splitlines() if ':' in line and line.split(':',1)[0].strip() in allowed}
        fields['kernel']=environment.splitlines()[0].split()[2]
        fields['distribution']='Ubuntu 24.04 (Canonical noble AMI selection)'
        machines[role]=fields
    pins.update({'dataOrigin':'local-simulator','access':'local-only; full raw archives and perf data excluded from Git',
                 'planSha256':digest(HERE/'plan.json'),
                 'returnedArchives':{role:json.loads((raw/(role+'-archive.json')).read_text()) for role in ['server','generator']},
                 'reductionSources':{p.name:digest(p) for p in HERE.glob('*.py')},
                 'runtimeAmendments':{p.stem:json.loads(p.read_text()) for p in raw.glob('*amendment.json')},
                 'binariesManifestSha256':digest(raw/'server/binaries.sha256'),
                 'binaries':{line.split()[1]:line.split()[0] for line in (raw/'server/binaries.sha256').read_text().splitlines()},
                 'toolchain':(raw/'server/toolchain.txt').read_text()})
    pins['machines']=machines
    pins['hardwareEventProbe']={k:v for k,v in json.loads((raw/'hardware-event-probe.json').read_text()).items() if k in ['exitCode','stdout','stderr']}
    if (raw/'bot-affinity-verified.json').exists():
        pins['botAffinityVerified']=json.loads((raw/'bot-affinity-verified.json').read_text())
    else:
        bot=json.loads((raw/'generator/raw/bots-500-host/telemetry.json').read_text())
        row=next(r for r in bot['rows'] if r['boundary']=='start')
        pins['botAffinityVerified']=[{'name':t['name'],'cpus':t['cpus']} for tasks in row['threads'].values() for t in tasks.values()]
    if (raw/'followup-plan.json').exists(): pins['followupPlan']=json.loads((raw/'followup-plan.json').read_text())
    pins['serverClockBridge']=json.loads((raw/'server-clock-bridge.json').read_text())
    pins['framePointerProfilePlan']=json.loads((raw/'frame-pointer-plan.json').read_text())
    pins['framePointerBinaries']={line.split()[1]:line.split()[0] for line in (raw/'server/binaries-fp.sha256').read_text().splitlines()}
    pins['functionExports']=function_exports
    if (output/'aws-summary.json').exists(): pins['awsSummarySha256']=digest(output/'aws-summary.json')
    cleanup=raw/'cleanup-verification-final.json'
    if not cleanup.exists(): cleanup=raw/'cleanup-verification.json'
    pins['rawCleanupVerificationSha256']=digest(cleanup)
    pins['creditPlanRecordHashes']={p.name:digest(p) for p in raw.glob('credits-*.json')}
    pins['failedAttemptArchives']={str(p.relative_to(raw.parent)).replace('\\','/'):{'sha256':digest(p),'bytes':p.stat().st_size} for p in raw.parent.glob('aws-1/*partial.tar.gz')}
    if (raw.parent/'ATTEMPTS.md').exists(): pins['scratchNotesSha256']=digest(raw.parent/'ATTEMPTS.md')
    pins['flamegraphs']={p.name:{'sha256':digest(p),'bytes':p.stat().st_size,'access':'local-only'} for p in (raw.parent/'figures').glob('*.svg')}
    if (raw.parent/'figures/flamegraph-source-pins.json').exists(): pins['flamegraphRenderer']=json.loads((raw.parent/'figures/flamegraph-source-pins.json').read_text())
    pins['generatorOrigin']={'label':'2026-10-24 host I/O study host_study_load.rs','sha256':'8d06b4c75c4b52c432372ea305a93fb3de540725f3e2597b93601d1907cdea17','access':'local-only original study'}
    (output/'source-pins.json').write_text(json.dumps(pins,indent=2)+'\n',encoding='utf-8',newline='\n')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('raw',type=Path);p.add_argument('output',type=Path)
    args=p.parse_args();main(args.raw,args.output)
