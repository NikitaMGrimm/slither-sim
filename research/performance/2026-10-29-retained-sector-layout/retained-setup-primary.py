from pathlib import Path
import subprocess,shutil
r=Path(__file__).resolve().parent
subprocess.run(['git','apply',str(r/'harness.patch')],cwd=r,check=True)
for name in ['build.py','check.py','compact-current-driver.py']:
 p=r/name;p.write_text(p.read_text().replace('/tmp/slither-compact-history-target','/tmp/slither-retained-layout-target'))
shutil.copyfile(r/'retained-check.py',r/'check.py')
p=r/'build.py';text=p.read_text()
needle="(source/'src/client/simulation/study_host.rs').write_text(payload)"
assert text.count(needle)==1
text=text.replace(needle,"payload=payload.replace('\"knownStorage\":','\"sectorStorage\":f.sim.backend.slots.iter().map(|s|s.connection.food_interest.study_storage()).collect::<Vec<_>>(),\"knownStorage\":')\n"+needle)
needle="shutil.copyfile(r/'compact-pmu.rs',source/'src/study_pmu.rs')"
assert text.count(needle)==1
extra="""vf=source/'src/client/visibility.rs';vt=vf.read_text()
if 'last_seen: Vec<f64>' in vt:
 sectorbody='serde_json::json!({"keys":self.subscribed.len(),"times":self.last_seen.len(),"keyCapacityBytes":self.subscribed.capacity()*2,"timeCapacityBytes":self.last_seen.capacity()*8})'
else:
 sectorbody='serde_json::json!({"keys":self.subscribed.len(),"times":self.last_seen.len(),"keyCapacityBytes":null,"timeCapacityBytes":null})'
vf.write_text(vt+'\\nimpl SectorInterest{pub(super) fn study_storage(&self)->serde_json::Value{'+sectorbody+'}}\\n')
"""
text=text.replace(needle,extra+needle);p.write_text(text)
print('Reused primary harness prepared for the chosen layout and sole target.')
