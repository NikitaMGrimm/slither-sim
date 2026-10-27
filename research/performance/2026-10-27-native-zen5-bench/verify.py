"""Re-reduce retained valid attempts without modifying original summaries."""
from pathlib import Path
import fcntl
import importlib.util
import json
import os
root=Path(__file__).resolve().parent
lock=(root/".bench.lock").open("w")
fcntl.flock(lock,fcntl.LOCK_EX | fcntl.LOCK_NB)
os.sched_setaffinity(0,{1,2,3})
spec=importlib.util.spec_from_file_location("bench",root/"bench.py")
bench=importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)
qualification=[]
verified=[]
for p in sorted((root/"raw").glob("*/summary.json")):
    d=json.loads(p.read_text())
    if not d["validTraffic"] or not d["validAffinity"]:
        continue
    fresh=bench.reduce_trial(p.parent,d["requested"])
    for key,value in fresh.items():
        if d[key]!=value:
            raise RuntimeError("reduction mismatch: "+p.parent.name+" / "+key)
    if "diagnosticCpuClock" in d:
        diagnostics=bench.module("diagnostics",root/"diagnostics.py")
        bench.require(d["diagnosticCpuClock"]==diagnostics.cpu_clock(p.parent),"CPU/clock reduction mismatch")
    verified.append(d["attempt"])
    if d["label"]=="qualification":
        qualification.append((d["commit"],d["preset"],d["requested"]))
        if d["preset"]=="official":
            bench.require(d["clientsTimeWeighted"]["playing"]>=d["requested"]*.99,"official occupancy shortfall")
        bench.require(d["generatorHasHeadroom"],"generator saturation")
    print("VERIFIED",d["label"],d["commit"][:7],d["preset"],d["requested"],flush=True)
expected={(r,p,n) for r in [bench.BASE,bench.INDEX,bench.MAIN] for p,n in [("official",500),("official",375),("battledome",80)]}
bench.require(len(qualification)==9 and set(qualification)==expected,"qualification census mismatch")
for rev in [bench.BASE,bench.INDEX,bench.MAIN,bench.CURRENT]:
    if not (root/"bin"/rev/"pins.json").exists():
        continue
    pins=json.loads((root/"bin"/rev/"pins.json").read_text())
    bench.require(bench.sha(root/"archives"/(rev+".tar"))==pins["archiveSha256"],"source archive changed")
    bench.require(bench.sha(root/"bin"/rev/"slither-sim")==pins["binarySha256"],"binary changed")
bench.save(root/"verification.json",{"verifiedAttempts":verified,"qualificationRows":len(qualification),"exactReductionMatch":True,"sourceAndBinaryHashesMatch":True})
print("All retained valid attempts re-reduced exactly; nine qualification rows and hashes matched.",flush=True)
