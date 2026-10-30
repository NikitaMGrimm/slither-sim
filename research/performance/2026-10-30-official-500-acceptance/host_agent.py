"""The ladder's read-only host observer with the acceptance's 500-slot limit."""
from pathlib import Path
r=Path(__file__).resolve().parent
s=(r/'shared258/host_agent.py').read_text()
assert s.count("'max':751")==1
s=s.replace("'max':751","'max':500")
exec(compile(s,str(r/'linked-host-agent.py'),'exec'),{'__file__':str(r/'linked-host-agent.py'),'__name__':'__main__'})
