"""Reuse the private host collector; release its control port after retrieval."""
from pathlib import Path
r=Path(__file__).resolve().parent
text=(r/'shared263/host_agent.py').read_text()
old="raw=archive.read_bytes();self.send_response(200);self.send_header('Content-Length',str(len(raw)));self.end_headers();self.wfile.write(raw);return"
assert text.count(old)==1
text=text.replace(old,old.replace(';return',';threading.Thread(target=server.shutdown,daemon=True).start();return'))
old='server.serve_forever(poll_interval=.25)';assert text.count(old)==1
text=text.replace(old,old+';server.server_close()')
exec(compile(text,str(r/'shared263/host_agent.py'),'exec'),{'__file__':str(r/'host_agent.py'),'__name__':'__main__'})
