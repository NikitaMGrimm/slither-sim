"""The existing fifteen-trace driver, changing only its runtime-file pin."""
from pathlib import Path
if not __debug__:raise SystemExit('run without -O')
p=Path(__file__).resolve().parent.parent/'2026-10-27-host-interest-refresh/reproduce.py'
s=p.read_text();assert s.count('src/client/simulation/broadcast.rs')==1
s=s.replace('src/client/simulation/broadcast.rs','src/client/server/readiness.rs').replace('broadcastSha256','readinessSha256')
exec(compile(s,str(p),'exec'),{'__file__':str(p),'__name__':'__main__'})
