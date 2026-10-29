"""Adapt the retained helpers, without changing Rust instrumentation or fixture."""
from pathlib import Path
import subprocess,sys

r = Path(__file__).resolve().parent
subprocess.run([sys.executable, str(r / 'retained-setup-primary.py')], check=True)
def change(path, before, after):
    text = path.read_text()
    assert text.count(before) == 1, (path.name, before)
    path.write_text(text.replace(before, after))

change(r / 'compact-cohort.py', 'range(-1,5)', 'range(-1,10)')
change(r / 'compact-cohort.py', "'timedPairs':5 if", "'timedPairs':10 if")
change(r / 'compact-proof-short.py', '[(500,0),(1,600),(10,600),(50,600)]', '[(500,0)]')
change(r / 'compact-proof-short.py', 'all 8 cases', 'both young-500 cases')
# setup.py creates serve.py only when the proof/build pipeline invokes it.
with (r / 'setup.py').open('a') as out:
    out.write('''
server=r/'serve.py';text=server.read_text()
text='from confirmation_background import snapshot,finish\\n'+text
needle="shared.inventory(session,label+'-before');start=time.monotonic()"
assert text.count(needle)==1
text=text.replace(needle,"shared.inventory(session,label+'-before');background_before=snapshot(root);start=time.monotonic()")
needle="   shared.inventory(session,label+'-after')"
assert text.count(needle)==1
extra="   background_after=snapshot(root)\\n   (session/(label+'-background-raw.json')).write_text(json.dumps({'before':background_before,'after':background_after},indent=2)+'\\\\n')\\n   (session/(label+'-background.json')).write_text(json.dumps(finish(background_before,background_after),indent=2)+'\\\\n')\\n"
text=text.replace(needle,extra+needle)
server.write_text(text)
''')
print('Four-row, ten-pair confirmation helpers prepared; no data collected.', flush=True)
