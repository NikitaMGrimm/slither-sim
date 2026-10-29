"""Extend the earlier portable trace overlay by reference, adding real-queue stress.

Usage: python prepare.py PRIOR_PREPARE REPO OUTPUT ARM REF
PRIOR_PREPARE is the minimap study's prepare_traces.py, staged from its branch.
"""
from pathlib import Path
import sys

prior=Path(sys.argv.pop(1)).resolve()
study=Path(__file__).resolve().parent
text=prior.read_text()
start=text.index("if arm == 'mutation':")
end=text.index('env = dict(',start)
text=text[:start]+'''# Spawn-specific wire mutation: discard otherwise eligible fallback heads.
if arm == 'mutation':
    p = source / 'src/client/simulation.rs'
    payload = p.read_text()
    old = 'if !fallback\\n                && safety'
    assert payload.count(old) == 1
    p.write_text(payload.replace(old, 'if true\\n                && safety'), newline='\\n')

payload = (STUDY / 'fixture_access.rs').read_text()
(source / 'src/client/simulation/study_spawn_fixture.rs').write_text(payload, newline='\\n')
p=source / 'src/client/simulation.rs'
payload=p.read_text()+'\\nmod study_spawn_fixture;\\n'
old='        let circular = self.world.config().spawn_radius.is_some();'
assert payload.count(old)==1
payload=payload.replace(old,'        crate::study_tick::search();\\n'+old)
old='        for attempt in 0..attempts {'
assert payload.count(old)==1
payload=payload.replace(old,old+'\\n            crate::study_tick::attempt();')
old='            let (body, heading) = self\\n                .world\\n                .spawn_body(head, heading)'
assert payload.count(old)==1
payload=payload.replace(old,'            crate::study_tick::body();\\n'+old)
old='        if circular {\\n            if let Some((_, body, heading)) = open {'
assert payload.count(old)==1
payload=payload.replace(old,'        crate::study_tick::exhaust();\\n'+old)
p.write_text(payload,newline='\\n')
(source / 'src/client/server/study_fixed.rs').write_text((STUDY/'fixed_host.rs').read_text(),newline='\\n')
p=source/'src/client/server.rs';p.write_text(p.read_text()+'\\npub mod study_fixed;\\n',newline='\\n')
counter=repo/'research/performance/2026-10-28-broadcast-distance/counter.rs'
(source/'src/study_counter.rs').write_text(counter.read_text(),newline='\\n')
(source/'src/study_tick.rs').write_text((STUDY/'tick_samples.rs').read_text(),newline='\\n')
p=source/'src/lib.rs';p.write_text(p.read_text()+'\\npub mod study_counter;\\npub mod study_tick;\\n',newline='\\n')
(source/'examples/study_fixed.rs').write_text(\"fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::server::study_fixed::run(a[1]==\\\"13\\\",a[2].parse().unwrap(),a.get(3).is_some_and(|s|s==\\\"--measure\\\"));}\\n\",newline='\\n')
'''+text[end:]
text=text.replace("'--example', 'study_protocols', '--example', 'perf_baseline']",
                  "'--example', 'study_protocols', '--example', 'perf_baseline', '--example', 'study_fixed']")
text=text.replace("for binary in ['study_protocols', 'perf_baseline']:",
                  "for binary in ['study_protocols', 'perf_baseline', 'study_fixed']:")
# This Windows overlay does not compile Linux-only syscall code. The same file
# is used unchanged on the released Linux bench and opens the real PMU there.
text=text.replace("p=source/'src/lib.rs';p.write_text(p.read_text()+'\\npub mod study_counter;\\npub mod study_tick;\\n',newline='\\n')",
                  "p=source/'src/lib.rs';p.write_text(p.read_text()+'\\n#[cfg(target_os=\\\"linux\\\")]\\npub mod study_counter;\\npub mod study_tick;\\n',newline='\\n')")
exec(compile(text,str(prior),'exec'),{'__file__':str(Path(__file__).resolve()),'__name__':'__main__','STUDY':study})
