"""Diagnostic brackets; --fine instruments the baseline no-op census only."""
from pathlib import Path
import tarfile,sys,re,json
if not __debug__:raise SystemExit('run without -O')
archive=Path(sys.argv[1]);script=Path('research/performance/2026-10-27-host-coverage-cells/profile.py')
# Use the frozen profiler, with no production source edits.
with tarfile.open(archive) as t:wrapper=t.extractfile(str(script)).read().decode()
if '--fine' not in sys.argv:
    exec(compile(wrapper,str(archive)+'-coarse','exec'))
    raise SystemExit(0)
sys.argv.remove('--fine')
with tarfile.open(archive) as t:
    if b'wanted: impl FnOnce()' in t.extractfile('src/client/simulation/snakes.rs').read():
        raise SystemExit('--fine requires the baseline eager-wanted runtime')
ns={'__file__':__file__,'__name__':'__main__'}
exec(compile(wrapper[:wrapper.rindex('exec(compile(driver')],__file__,'exec'),ns)
driver,counter=ns['driver'],ns['counter'];old=counter
names=['world_step','shared_food','reports','bounds','grid','prey_bounds','candidates','snake_updates','recipient_food','recipient_prey','framing','legacy','board','map','hud','broadcast','grid_cleanup','covered','delta_cache','uncached_delta','no_op','cache_hit','wanted','empty','observe','own_no_op']
counter=counter.replace('const N:usize=17;','const N:usize=26;');counter=re.sub(r'const NAMES:\[&str;N\]=\[.*?\];','const NAMES:[&str;N]='+json.dumps(names,separators=(',',':'))+';',counter)
counter+='\npub(crate) fn event(i:usize,n:usize){DATA.with(|d|{let mut d=d.borrow_mut();if d.active{d.calls[i]+=n as u64;}});}\n'
before="(source/'src/recipient_profile.rs').write_text("+repr(old)+")";assert driver.count(before)==1;driver=driver.replace(before,"(source/'src/recipient_profile.rs').write_text("+repr(counter)+")").replace('range(17)','range(26)')
hooks='''
edit(s,'pub(super) fn covered(head: Point, snake: &Snake, [x0, y0, x1, y1]: [f64; 4], vision: f64) -> bool {','pub(super) fn covered(head: Point, snake: &Snake, [x0, y0, x1, y1]: [f64; 4], vision: f64) -> bool {\\n    let _study_scope=crate::recipient_profile::Scope::new(17);')
edit(s,'        if own {\\n            return changes','''+repr('''        let _study_scope=crate::recipient_profile::Scope::new(18);
        let noop = snake.body.first()==Some(&known.front) && known.len==snake.body.len() && known.kills==snake.kills
            && (!reports.motion_due || [known.heading,known.wanted,known.speed]==reports.motion)
            && (!reports.fam_due || known.fam==encode::fam_u24(snake.fam));
        if noop {crate::recipient_profile::event(20,1);if own{crate::recipient_profile::event(25,1);}}
        if own {
            return changes''')+''')
edit(s,'    let body = &snake.body;','    let _study_scope=crate::recipient_profile::Scope::new(19);\\n    let body = &snake.body;')
edit(s,'            packets.extend_from_slice(&delta.packets);','            crate::recipient_profile::event(21,1);\\n            packets.extend_from_slice(&delta.packets);')
edit(b,'                    let wanted = physics::quantize_heading(control);','                    let study_wanted=crate::recipient_profile::Scope::new(22);\\n                    let wanted = physics::quantize_heading(control);\\n                    drop(study_wanted);\\n                    let study_empty=crate::recipient_profile::Scope::new(23);\\n                    drop(study_empty);',2)
scope(s,'    pub(super) fn observe(&mut self, known: &KnownSnake) {',24)
'''
driver=driver.replace('\nenv=dict(os.environ',hooks+'\nenv=dict(os.environ')
exec(compile(driver,'snake-noop-diagnostic','exec'),ns)
