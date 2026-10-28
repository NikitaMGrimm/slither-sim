"""Private run.sh adapter: shared lock, owned paths, one host/load CPU."""
from pathlib import Path
import fcntl,gzip,hashlib,json,os,shutil,subprocess
if not __debug__:raise SystemExit('run without -O')
r=Path(__file__).resolve().parent;b=r.parents[1]
with (b/'.bench.lock').open('r') as lock:
    try:fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    except BlockingIOError:raise SystemExit('LOCK_BUSY')
    os.sched_setaffinity(0,{1})
    for name in ['archives','bin','targets','sources','raw']:(r/name).mkdir(exist_ok=True)
    for arm in ['base','candidate']:
        with gzip.open(r/(arm+'.tar.gz'),'rb') as src:
            data=src.read()
        import tarfile,io
        with tarfile.open(fileobj=io.BytesIO(data)) as t:revision=t.pax_headers['comment']
        with (r/'archives'/(revision+'.tar')).open('xb') as f:f.write(data)
    for name,target in [('harness',b/'harness'),('diagnostics.py',b/'diagnostics.py'),('.bench.lock',b/'.bench.lock'),('bin/host_capacity_load',b/'bin/host_capacity_load'),('bin/4c2471ba9afff3609faae964cdaad49e3209da68',b/'bin/4c2471ba9afff3609faae964cdaad49e3209da68')]:
        (r/name).symlink_to(target)
    shutil.copy2(b/'run.sh',r/'run.sh')
    s=(b/'bench.py').read_text()
    def replace(a,z):
        global s
        assert s.count(a)==1,(a,s.count(a));s=s.replace(a,z)
    replace('import argparse\n','import argparse\nif not __debug__: raise SystemExit("run without -O")\n')
    replace("'bind': '127.0.0.1:8766'","'bind': '127.0.0.1:18934'")
    anchor='    replace(worker, "str(i + (1 if args.watcher else 0))", "str(1 + i % 3)")'
    replace(anchor,'    replace(worker, "str(count), \'8766\'", "str(count), \'18934\'")\n'+anchor.replace('str(1 + i % 3)','str(1)'))
    replace('f":{8766:04X}"','f":{18934:04X}"')
    replace('"--shards", "8"','"--shards", "1"')
    replace('os.sched_setaffinity(0, {1, 2, 3})','os.sched_setaffinity(0, {1})\n    os.environ.update(CARGO_NET_OFFLINE="true",RAYON_NUM_THREADS="1",RUST_TEST_THREADS="1")')
    replace('trial(commit(args.commit), args.preset, args.count, label="rerun", diagnostic=True)', 'trial(commit(args.commit), args.preset, args.count, hold=8 if os.environ.get("STUDY_SMOKE") else 120, warmup=3 if os.environ.get("STUDY_SMOKE") else 15, label="epoll-smoke" if os.environ.get("STUDY_SMOKE") else "epoll-pair", diagnostic=True)')
    with (r/'bench.py').open('x') as f:f.write(s)
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    (r/'adapter-pins.json').write_text(json.dumps({'parentBench':sha(b/'bench.py'),'adapterBench':sha(r/'bench.py'),'runSh':sha(r/'run.sh'),'parentRunSh':sha(b/'run.sh'),'setup':sha(Path(__file__)),'port':18934,'hostCpus':[0],'loadCpus':[1],'shards':1},indent=2)+'\n')
    assert sha(r/'run.sh')==sha(b/'run.sh')
    print('READY',flush=True)
