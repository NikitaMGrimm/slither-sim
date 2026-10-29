"""Run #262's collector with only the requested three-count high-boost schedule."""
from pathlib import Path
r=Path(__file__).resolve().parent;s=(r/'shared262/run_ladder.py').read_text()
def change(old,new):
 global s
 assert s.count(old)==1,(old,s.count(old));s=s.replace(old,new)
change('for target in [80,120,160,240,320,480,640]:','for target in [80,160,320]:')
change("else:summary['stopReason']=['predeclared-640-ceiling']","else:summary['stopReason']=['requested-three-counts-complete']")
change(" window(80,'down-80',30)"," # No descending arm or human hold in this follow-up.")
change("  if not result['clientValid']:reasons.append('generator-or-queue-limit')","  if not result['clientValid']:reasons.append('generator-or-queue-limit')\n  if result['boost']['aliveTickShare']<.7:reasons.append('boost-duty-below-70-percent')")
# Keep measured overload rows, but stop rather than invent host capacity if the
# generator, output queues or treatment are invalid. No world/default change.
change("  if reasons:summary['stopReason']=reasons;break","  if reasons:\n   summary.setdefault('observedLimits',[]).append({'bots':target,'reasons':reasons})\n   if not result['clientValid'] or result['boost']['aliveTickShare']<.7:summary['stopReason']=reasons;break")
exec(compile(s,str(r/'shared262/run_ladder.py'),'exec'),{'__file__':str(r/'run_ladder.py'),'__name__':'__main__'})
