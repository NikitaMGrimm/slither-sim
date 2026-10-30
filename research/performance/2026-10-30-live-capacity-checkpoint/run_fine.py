"""Fresh mature diagnostic using the unchanged received-frame client/warmup."""
from pathlib import Path
import sys
r=Path(__file__).resolve().parent;s=(r/'run.py').read_text()
def sub(old,new):
 # A missed rewrite would silently run the ordinary or uninstrumented cohort.
 global s;assert old in s,old;s=s.replace(old,new)
sub('arm=sys.argv[1];name=sys.argv[2];',"arm='diagnostic-candidate';name=sys.argv[2];")
sub("CAPACITY_DETAIL=str(root/'DETAIL')","CAPACITY_DETAIL=str(root/'DETAIL'),CAPACITY_FINE=str(root/'FINE')")
sub('hold_seconds=20 if smoke else (30 if short_mature else 300)','hold_seconds=15')
sub('diagnostic_seconds=5 if smoke else (15 if short_mature else 60)','diagnostic_seconds=20')
sub("(root/'DETAIL').unlink();meta['coarseEndUnix']=time.time();(root/'DIAGNOSTIC').touch();profile=now+diagnostic_seconds;meta['diagnosticStartUnix']=time.time();save()", "meta['coarseEndUnix']=time.time();(root/'FINE').touch();profile=now+20;meta['fineStartUnix']=time.time();meta['diagnosticStartUnix']=time.time();meta['fineStartInventorySha256']=inventory('fine-start');save()")
sys.argv=[str(r/'run.py'),'candidate',sys.argv[1],'500','--short-mature']
exec(compile(s,str(r/'run.py'),'exec'),{'__file__':str(r/'run.py'),'__name__':'__main__'})
