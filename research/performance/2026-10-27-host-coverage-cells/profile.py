"""Diagnostic phase brackets, including grid construction and end-of-use."""
import sys, tarfile
from pathlib import Path
if not __debug__:
    raise SystemExit('run without -O')
archive = Path(sys.argv[1])
with tarfile.open(archive) as t:
    prefix = 'research/performance/2026-10-27-host-food-ranges/'
    driver = t.extractfile(prefix + 'profile.py').read().decode()
    counter = t.extractfile(prefix + 'profile.rs').read().decode()
counter = counter.replace('const N:usize=16;', 'const N:usize=17;')
counter = counter.replace('"hud","broadcast"];', '"hud","broadcast","grid_cleanup"];')
driver = driver.replace("shutil.copy2(folder/'profile.rs',source/'src/recipient_profile.rs')", "(source/'src/recipient_profile.rs').write_text(" + repr(counter) + ")")
driver = driver.replace('range(16)', 'range(17)')
before = "edit(b,line,'        drop(study_bounds);\\n        let study_grid=crate::recipient_profile::Scope::new(4);\\n'+line+'\\n        drop(study_grid);')"
# Scope the whole constructor/update branch on either frozen arm.
assert driver.count(before) == 1
after = """if line in (source/b).read_text():
 edit(b,line,'        drop(study_bounds);\\n        let study_grid=crate::recipient_profile::Scope::new(4);\\n'+line+'\\n        drop(study_grid);')
else:
 edit(b,'        let grid = if let Some(mut grid) = self.coverage_grid.take() {','        drop(study_bounds);\\n        let study_grid=crate::recipient_profile::Scope::new(4);\\n        let grid = if let Some(mut grid) = self.coverage_grid.take() {')
 line='        let prey_bounds = PreyBounds::new(self.world.prey(), self.world.config().vision);'
 edit(b,line,'        drop(study_grid);\\n'+line)
"""
driver = driver.replace(before, after)
# Explicitly bracket the old temporary's drop, or the new cache's hand-back.
hooks = """
if '        self.coverage_grid = Some(grid);' in (source/b).read_text():
 edit(b,'        self.coverage_grid = Some(grid);','        let study_cleanup=crate::recipient_profile::Scope::new(16);\\n        self.coverage_grid = Some(grid);\\n        drop(study_cleanup);')
else:
 line='        self.food_eaters.clear();\\n        self.prey_eaters.clear();\\n        Ok(())'
 edit(b,line,'        let study_cleanup=crate::recipient_profile::Scope::new(16);\\n        drop(grid);\\n        drop(study_cleanup);\\n'+line)
"""
driver = driver.replace('\nenv=dict(os.environ', hooks + '\nenv=dict(os.environ')
exec(compile(driver, str(archive) + '-diagnostic', 'exec'))
