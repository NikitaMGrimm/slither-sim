"""Extend the frozen twelve-trace harness with a core shrink replay."""
import hashlib
import os
from pathlib import Path
import runpy
import subprocess
import sys
if not __debug__:
    raise SystemExit('Run without optimization')
if os.environ.get('RUSTFLAGS') or os.environ.get('CARGO_ENCODED_RUSTFLAGS'):
    raise SystemExit('Use the default target: unset RUSTFLAGS/CARGO_ENCODED_RUSTFLAGS')
repo=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve()
base,candidate=sys.argv[3:5]
fixture=out.with_name(out.name+'-fixture');fixture.mkdir(parents=True,exist_ok=False)
ref='8014ad0da6ad8d67f3d305ca3c43ed53841d66bd'
prefix='research/performance/2026-10-27-host-snake-candidates/'
for name in ['reproduce.py','protocol_fixture.rs','tick_counter.rs']:
    data=subprocess.check_output(['git','-C',str(repo),'show',f'{ref}:{prefix}{name}'])
    (fixture/name).write_bytes(data)
driver=(fixture/'reproduce.py').read_text()
driver='if not __debug__: raise SystemExit("Run without optimization")\n'+driver
needle="target=out/'target';names="
assert driver.count(needle)==1
extra='''(source/'src/lib.rs').write_text((source/'src/lib.rs').read_text()+'\\npub mod study_retention;\\n')
        shutil.copy2(study/'retention_fixture.rs',source/'src/study_retention.rs')
        child=source/'src/study_retention';child.mkdir()
        shutil.copy2(study/'tick_counter.rs',child/'tick_counter.rs')
        (source/'examples/study_retention.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::study_retention::run(&a[1],a.get(2).is_some_and(|s|s=="--trace"));}\\n')
        target=(Path(os.environ['RETENTION_TARGET'])/('native-'+arm)/'target') if os.environ.get('RETENTION_TARGET') else out/'target';names='''
driver=driver.replace(needle,extra)
driver=driver.replace("names=['study_protocols'","names=['study_retention','study_protocols'")
(fixture/'reproduce.py').write_text(driver)
(fixture/'retention_fixture.rs').write_bytes(Path(__file__).with_name('retention_fixture.rs').read_bytes())
sys.argv=[str(fixture/'reproduce.py'),'build',str(repo),str(out),'--base',base,'--candidate',candidate,'--cpu',os.environ.get('RETENTION_CPU','3')]
runpy.run_path(str(fixture/'reproduce.py'),run_name='__main__')
guard='if not __debug__: raise SystemExit("Run without optimization")\n'
for arm in ['base','candidate']:
    folder=out/f'native-{arm}'
    helper=folder/'harness/trace_compare.py';helper.write_text(guard+helper.read_text())
    pins={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (folder/'bin').iterdir()}
    import json
    (folder/'binaries.json').write_text(json.dumps(pins,indent=2)+'\n')
print('BUILD COMPLETE',flush=True)
