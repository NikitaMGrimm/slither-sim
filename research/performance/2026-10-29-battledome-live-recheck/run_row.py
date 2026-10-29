"""Fresh natural #263 ladder up to one predeclared primary comparison row.

The previous collector and its metrics/reducer/delivery files stay beside this
wrapper in external staging. Earlier stages are retained calibration windows.
"""
from pathlib import Path
import json
r=Path(__file__).resolve().parent
job=json.loads((r/'job.json').read_text());target=job['target'];assert target in [160,320,480]
text=(r/'shared262/run_ladder.py').read_text()
def change(old,new):
    global text
    assert text.count(old)==1,(old,text.count(old));text=text.replace(old,new)
change("'runtime':'165a5bb764aba6cbf39f42503e090f9b5e3c4d22'", "'runtime':job['runtime'],'arm':job['arm'],'primaryTarget':job['target'],'runLabel':job['label']")
change('for target in [80,120,160,240,320,480,640]:',f'for target in {[x for x in [80,160,320,480] if x<=target]}:')
change("  if not result['clientValid']:reasons.append('generator-or-queue-limit')", "  if not result['clientValid']:reasons.append('generator-or-queue-limit')\n  if result['boost']['aliveTickShare']<.99:reasons.append('boost-duty-below-99-percent')")
change("  if reasons:summary['stopReason']=reasons;break", "  if reasons:\n   summary.setdefault('observedLimits',[]).append({'bots':target,'reasons':reasons})\n   if not result['clientValid'] or result['boost']['aliveTickShare']<.99:summary['stopReason']=reasons;break")
change("else:summary['stopReason']=['predeclared-640-ceiling']", "else:summary['stopReason']=['requested-primary-row-complete']")
change(" window(80,'down-80',30)", " # No descending arm or owner hold.")
exec(compile(text,str(r/'shared262/run_ladder.py'),'exec'),{'__file__':str(r/'run_row.py'),'__name__':'__main__'})
