"""Narrow prey scopes over the pinned recipient-work diagnostic."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys

if not __debug__:
    raise SystemExit('Run without optimization')
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('ref');p.add_argument('output');p.add_argument('--control',type=Path,required=True)
p.add_argument('--cpu',type=int,default=3);p.add_argument('--prepare-only',action='store_true');a=p.parse_args()
ref, output = a.ref,a.output
root = Path(output).resolve()
root.mkdir(parents=True, exist_ok=False)
archive = root / 'source.tar'
subprocess.run(['git', 'archive', '-o', str(archive), ref], check=True)
prefix = 'research/performance/2026-10-27-host-food-ranges/'
driver = subprocess.check_output(['git', 'show', f'{ref}:{prefix}profile.py'], text=True)
counter = subprocess.check_output(['git', 'show', f'{ref}:{prefix}profile.rs'], text=True)
counter = counter.replace('const N:usize=16;', 'const N:usize=4;')
counter = re.sub(r'const NAMES:\[&str;N\]=\[.*?\];', 'const NAMES:[&str;N]=["shared_bounds","recipient_total","lookup_exact","bookkeeping"];', counter)
start = driver.index("b='src/client/simulation/broadcast.rs'")
end = driver.index('\nenv=dict(os.environ', start)
hooks = '''b='src/client/simulation/broadcast.rs'
line='        let prey_bounds = PreyBounds::new(self.world.prey(), self.world.config().vision);'
edit(b,line,'        let study_build=crate::recipient_profile::Scope::new(0);\\n'+line+'\\n        drop(study_build);')
line='        let mut visible = BTreeMap::new();'
edit(b,line,'        let study_total=crate::recipient_profile::Scope::new(1);\\n        let study_lookup=crate::recipient_profile::Scope::new(2);\\n'+line)
line='        let gone: Vec<u64> = connection'
edit(b,line,'        drop(study_lookup);\\n        let study_bookkeeping=crate::recipient_profile::Scope::new(3);\\n'+line)
line='        Ok((packets, presented))'
edit(b,line,'        drop(study_bookkeeping);\\n        drop(study_total);\\n'+line)
'''
driver = driver[:start] + hooks + driver[end:]
driver = driver.replace("shutil.copy2(folder/'profile.rs',source/'src/recipient_profile.rs')", "(source/'src/recipient_profile.rs').write_text(" + repr(counter) + ")")
driver = driver.replace('range(16)', 'range(4)')
driver = driver.replace("CARGO_TARGET_DIR=str(out/'target')", "CARGO_TARGET_DIR=os.environ.get('PREY_TARGET',str(out/'target'))")
driver = driver.replace("out/'target/release/examples/study_protocols'", "Path(env['CARGO_TARGET_DIR'])/'release/examples/study_protocols'")
(root / 'driver.py').write_text(driver)
if a.prepare_only:
    driver = driver[:driver.index('\nenv=dict(os.environ')]
sys.argv = [sys.argv[0], str(archive), str(root), '--cpu', str(a.cpu), '--control', str(a.control.resolve())]
exec(compile(driver, str(root/'driver.py'), 'exec'))
