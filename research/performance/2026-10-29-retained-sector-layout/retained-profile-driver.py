"""Reuse #269's frozen-backend driver with the predeclared three events/repeats."""
from pathlib import Path
import sys,subprocess
r=Path(__file__).resolve().parent
text=(r/'donor-run.py').read_text()
before="jobs=[(event,rep,False) for rep in range(3) for event in [1,2,3,5,6,7]]+[(event,0,True) for event in [1,3,5,6]]"
after="jobs=[(event,rep,False) for rep in range(3) for event in [1,3,5]]+[(event,rep,True) for rep in range(3) for event in [1,3,5]]"
assert text.count(before)==1
text=text.replace(before,after)
before="   if '\"ready\":true' in text:break"
after="   if '\"ready\":true' in text:print(label,'WARM READY',flush=True);break\n   if text.startswith('{') and json.loads(text).get('tick',-1)%25000==0:print(label,'warm tick',json.loads(text)['tick'],flush=True)"
assert text.count(before)==1
(r/'run.py').write_text(text.replace(before,after))
subprocess.run([sys.executable,str(r/'run.py')],check=True)
