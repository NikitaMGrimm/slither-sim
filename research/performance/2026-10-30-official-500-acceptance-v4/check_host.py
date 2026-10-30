from pathlib import Path
import json,os,py_compile,subprocess,tempfile,time
import quiet_image as q
for name in ['quiet_image.py','host_diagnostics.py','host_agent.py','check_replay.py']:py_compile.compile(name,doraise=True)
assert all(q.targeted_service(n) for n in ['fwupd.service','snapd.service','snapd.socket','amazon-ssm-agent.service','snap.amazon-ssm-agent.amazon-ssm-agent.service'])
assert not any(q.targeted_service(n) for n in ['ssh.service','chrony.service','systemd-networkd.service','systemd-journald.service','rsyslog.service'])
q.verify({'units':{'a':{'ActiveState':'inactive','LoadState':'loaded','UnitFileState':'masked-runtime'}}},['a'])
for state in [('active','masked'),('inactive','enabled')]:
 try:q.verify({'units':{'a':{'ActiveState':state[0],'UnitFileState':state[1]}}},['a'])
 except RuntimeError:pass
 else:raise AssertionError('Quiet-policy failure accepted')
with tempfile.TemporaryDirectory() as d:
 root=Path(d);p=subprocess.Popen(['python3','host_diagnostics.py',d]);time.sleep(2.2);(root/'diagnostic-stop').touch();assert p.wait(timeout=5)==0
 rows=[json.loads(l) for l in (root/'background.jsonl').read_text().splitlines()];assert len(rows)>=2
 assert all('ctxt ' in r['procStat'] and 'procs_running ' in r['procStat'] and r['processes'] for r in rows)
 assert all(all(set(['pid','start','userTicks','systemTicks','comm'])<=set(t) for t in r['processes']) for r in rows)
 assert all(r['endUnix']>=r['unix'] for r in rows)
print('V4 syntax, service targeting/protected exclusions, fail-closed state checks, live all-process/counter cadence passed')
