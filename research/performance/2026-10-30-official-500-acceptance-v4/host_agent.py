"""V3 host observer and file path, plus quiet policy and retained diagnostics."""
from pathlib import Path
import subprocess

r = Path(__file__).resolve().parent
subprocess.run(['sudo', 'python3', str(r / 'quiet_image.py'), 'prepare',
                str(r / 'image-policy')], check=True)
s = (r / 'shared258/host_agent.py').read_text()


def swap(old, new):
    global s
    assert s.count(old) == 1, (old, s.count(old))
    s = s.replace(old, new)


swap("'max':751", "'max':500")
swap("(root/'process.json').write_text", """__import__('shutil').copytree(r/'image-policy',root/'image-policy')
diagnostic=subprocess.Popen(['python3',str(r/'host_diagnostics.py'),str(root)],stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT)
(root/'process.json').write_text""")
swap('   log.close()', """   (root/'diagnostic-stop').touch();diagnostic.wait(timeout=5)
   assert diagnostic.returncode==0,'Host diagnostic failed'
   subprocess.run(['sudo','python3',str(r/'quiet_image.py'),'after',str(root/'image-policy')],check=True)
   since=json.loads((root/'diagnostic-start.json').read_text())['unix']
   with (root/'window-journal.log').open('xb') as journal:
    subprocess.run(['sudo','journalctl','--since','@'+str(since),'--until','@'+str(time.time()),'--no-pager','-o','short-unix'],stdout=journal,check=True)
   log.close()""")
exec(compile(s, str(r / 'linked-v4-host-agent.py'), 'exec'),
     {'__file__': str(r / 'linked-v4-host-agent.py'), '__name__': '__main__'})
