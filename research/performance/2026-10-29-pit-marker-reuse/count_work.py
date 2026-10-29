"""Untimed operation counts from already qualified fixed-work source copies.

Usage: python count_work.py QUALIFIED_OUTPUT OUTPUT CARGO_TARGET
No clocks or PMU are enabled; do not use these binaries for timing.
"""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys
qualified,out,target=map(lambda p:Path(p).resolve(),sys.argv[1:])
study=Path(__file__).resolve().parent
out.mkdir(exist_ok=False)
qualification=json.loads((qualified/'selected.json').read_text())
for name,count in [('catalog',23),('host',6),('shrinkingRow',2)]:
    rows=qualification[name];assert len(rows)==count and all(r['base']==r['candidate'] for r in rows)
    if name=='shrinkingRow':assert all(r['wireMutationDetected'] for r in rows)
names=['sectorVisits','overlayCalls','emptyOverlays','nonemptyOverlays',
       'coverageQueries','coverageInputKeys','coverageUniqueKeys']
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def edit(path,old,new,count=1):
    text=path.read_text();assert text.count(old)==count,(path.name,old)
    path.write_text(text.replace(old,new),newline='\n')
env=dict(os.environ,CARGO_BUILD_JOBS='2',RUST_TEST_THREADS='1',RAYON_NUM_THREADS='1',
         CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=str(target))
result={'scope':'Untimed synthetic operation census; not CPU, instruction or allocation counts.',
        'qualificationRef':'local-current.json','qualificationSha256':sha(qualified/'selected.json'),
        'counterNames':names,'harnesses':{p.name:sha(p) for p in [Path(__file__),study/'work_counts.rs']},'arms':{},'rows':{}}
for arm in ['base','candidate']:
    pins=json.loads((qualified/arm/'pins.json').read_text())
    original=qualified/arm/'source'
    expected={k.replace('\\','/'):v for k,v in pins['sources'].items()}
    assert expected=={p.relative_to(original).as_posix():sha(p) for p in original.rglob('*.rs')}
    source=out/arm/'source';shutil.copytree(original,source)
    (source/'src/study_work.rs').write_text((study/'work_counts.rs').read_text(),newline='\n')
    p=source/'src/lib.rs';p.write_text(p.read_text()+'\npub mod study_work;\n',newline='\n')
    edit(source/'src/client/simulation/food.rs','let mut list_sector = |index: u16| {',
         'let mut list_sector = |index: u16| { crate::study_work::add(0, 1);')
    markers=source/'src/client/simulation/markers.rs'
    edit(markers,'let current = self.sectors.get(&sector);','crate::study_work::add(1, 1);\n        let current = self.sectors.get(&sector);')
    edit(markers,'if current.is_none() && previous.is_none() {','if current.is_none() && previous.is_none() { crate::study_work::add(2, 1);')
    edit(markers,'let mut run = current.cloned().unwrap_or_default();',
         'crate::study_work::add(3, 1);\n        let mut run = current.cloned().unwrap_or_default();')
    if arm=='candidate':
        edit(markers,'self.affected.binary_search(&sector).is_ok()',
             '{ crate::study_work::add(4, 1); self.affected.binary_search(&sector).is_ok() }')
        for expression,length in [('self.sectors','self.sectors'),('previous','previous')]:
            old=f'self.affected.extend({expression}.keys().copied());'
            edit(markers,old,f'crate::study_work::add(5, {length}.len() as u64); '+old,2)
        edit(markers,'Ok(changed)','if changed { crate::study_work::add(6, self.affected.len() as u64); }\n        Ok(changed)')
    counter=source/'src/study_tick.rs'
    edit(counter,'pub fn begin() -> Option<([u64;3],u64)> {',
         'pub fn begin() -> Option<([u64;3],u64)> { crate::study_work::begin();')
    edit(counter,'let events=EVENTS.with(Cell::get);','let work=crate::study_work::finish();\n    let events=EVENTS.with(Cell::get);')
    edit(counter,'serde_json::json!({"events":events})','serde_json::json!({"events":events,"work":work})')
    manifest={p.relative_to(source).as_posix():sha(p) for p in source.rglob('*') if p.is_file()}
    manifest_path=out/arm/'source-manifest.json';manifest_path.write_text(json.dumps(manifest,sort_keys=True)+'\n')
    with (out/arm/'build.log').open('x') as log:
        subprocess.run(['cargo','build','--locked','--offline','-j','2','--example','study_fixed'],
                       cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
    suffix='.exe' if os.name=='nt' else '';exe=out/arm/('study_fixed'+suffix)
    shutil.copy2(target/'debug/examples'/exe.name,exe)
    assert pins['archiveSha256']==qualification['artifacts'][arm]['archiveSha256']
    result['arms'][arm]={'qualifiedArm':arm,'sourceManifestSha256':sha(manifest_path),'binarySha256':sha(exe)}
    for row in ['markers','minimap']:
        for protocol in [19,13]:
            key=f'{row}-{protocol}';trace=out/arm/f'{key}.bin';stderr=out/arm/f'{key}.stderr'
            with trace.open('xb') as f,stderr.open('xb') as e:
                subprocess.run([str(exe),str(protocol),'360','--trace',row],stdout=f,stderr=e,env=env,check=True)
            data=json.loads(stderr.read_text().splitlines()[-1]);ticks=data['ticks'];assert len(ticks)==360
            assert all('cpuNs' not in t['sample'] and 'instructions' not in t['sample'] for t in ticks)
            groups={'all':ticks,'marker':[t for t in ticks if t['marker']],
                    'other':[t for t in ticks if not t['marker']]}
            reduced={g:{'ticks':len(ts),**{n:sum(t['sample']['work'][i] for t in ts) for i,n in enumerate(names)}} for g,ts in groups.items()}
            result['rows'].setdefault(key,{})[arm]={'groups':reduced,'traceSha256':sha(trace),'traceBytes':trace.stat().st_size}
            if arm=='candidate':
                with trace.open('rb') as a,(out/'base'/f'{key}.bin').open('rb') as b:
                    while True:
                        x,y=a.read(1<<20),b.read(1<<20);assert x==y,key
                        if not x:break
            assert sha(exe)==result['arms'][arm]['binarySha256']
            print('WORK COUNTS',arm,key,flush=True)
    (out/'work-counts.json').write_text(json.dumps(result,indent=2)+'\n',newline='\n')
assert len({a['binarySha256'] for a in result['arms'].values()})==2
for protocol in [19,13]:
    changed=result['rows'][f'markers-{protocol}']
    a,b=[changed[arm]['groups']['marker'] for arm in ['base','candidate']]
    assert a['ticks']==b['ticks']>=3 and a['sectorVisits']==b['sectorVisits']
    assert a['nonemptyOverlays']==b['nonemptyOverlays'] and b['emptyOverlays']<a['emptyOverlays']
    assert a['coverageQueries']==0 and b['coverageQueries']==b['sectorVisits']
    steady=result['rows'][f'minimap-{protocol}']
    assert steady['base']['groups']==steady['candidate']['groups']
    assert steady['base']['groups']['marker']['ticks']==0
print('UNTIMED WORK CENSUS QUALIFIED',flush=True)
