"""Build/run a separately pinned cold diagnostic; never replace a steady row.

python3 cliff_probe.py BUILT_STUDY_OUTPUT --cpu 6
Uses the study-owned cached target, immutable copied executable and original
default mechanics. The only new runtime hooks record dense-grid success/food.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import resource
import math

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("root", type=Path)
parser.add_argument("--cpu", type=int, default=6)
parser.add_argument("--populations", type=int, nargs="+", default=[5000,10000])
parser.add_argument("--ticks", type=int, default=8)
parser.add_argument("--bounded-seconds", type=int, default=0)
parser.add_argument("--cpu-budget-seconds", type=int, default=0)
parser.add_argument("--prior-cpu-seconds", type=float, default=0.)
parser.add_argument("--total-control-budget-seconds", type=float, default=3600.)
args = parser.parse_args()
os.sched_setaffinity(0, {args.cpu})
root = args.root.resolve()
out = root / "cliff-probe"
out.mkdir()
source = out / "source"
shutil.copytree(root / "profile/source", source)
study = Path(__file__).resolve().parent
shutil.copy2(study / "cliff_probe.rs", source / "examples/study_cliff.rs")
module=source/"src/client/simulation/study_scaling.rs"
module.write_text(module.read_text()+"\n"+(study/"bounded_probe.rs").read_text())
(source/"examples/study_bounded.rs").write_text("fn main(){slither_sim::client::simulation::study_scaling::bounded_run();}\n")
path = source / "src/simulator.rs"
text = path.read_text()
anchor = "        let indexed = !dense"
if text.count(anchor) != 1:
    raise RuntimeError("ambiguous dense-path diagnostic anchor")
path.write_text(text.replace(anchor, "        crate::study_counter::record(20, u64::from(dense));\n        crate::study_counter::record(21, self.food.len() as u64);\n" + anchor))
grid=source/"src/food_grid.rs"
text=grid.read_text()
anchor="        if cells > MAX_CELLS.min(max_cells)"
if text.count(anchor)!=1:
    raise RuntimeError("ambiguous grid extent anchor")
grid.write_text(text.replace(anchor,"        crate::study_counter::record(22, cells as u64);\n        crate::study_counter::record(23, max_cells as u64);\n"+anchor))
target = Path(os.environ.get("SCALING_TARGET_ROOT", root)) / "profile/target"
env = dict(os.environ, RAYON_NUM_THREADS="1", CARGO_BUILD_JOBS="1", RUST_TEST_THREADS="1", RUSTFLAGS="", CARGO_ENCODED_RUSTFLAGS="")
env.pop("SCALING_FAST_ADMISSION", None)
env.pop("SCALING_ORIGINAL_WARM", None)
command = [shutil.which("cargo") or str(Path.home()/".cargo/bin/cargo"), "+1.99.0", "build", "--offline", "--release", "--locked", "-j", "1", "--config", 'profile.release.debug=1', "--target-dir", str(target), "--example", "study_cliff", "--example", "study_bounded"]
with (out / "build.log").open("x") as stream:
    subprocess.run(["taskset", "-c", str(args.cpu), *command], cwd=source, env=env, stdout=stream, stderr=subprocess.STDOUT, check=True)
binary = out / "study_cliff"
shutil.copy2(target / "release/examples/study_cliff", binary)
bounded=out/"study_bounded"
shutil.copy2(target/"release/examples/study_bounded",bounded)
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
pins = {"runtime": json.loads((root/"pins.json").read_text())["runtime"], "origin": str(root/"profile/source"),
        "binarySha256":sha(binary),"boundedBinarySha256":sha(bounded),"probeSourceSha256":sha(study/"cliff_probe.rs"),
        "boundedSourceSha256":sha(study/"bounded_probe.rs"),"boundedModuleSha256":sha(module),
        "simulatorOverlaySha256":sha(path),"foodGridOverlaySha256":sha(grid),"buildCommand":command}
(out / "pins.json").write_text(json.dumps(pins,indent=2)+"\n")
usage_before=resource.getrusage(resource.RUSAGE_CHILDREN)
attempts=[]
def run_probe(command,log,cap=None):
    used=resource.getrusage(resource.RUSAGE_CHILDREN)
    spent=used.ru_utime+used.ru_stime-usage_before.ru_utime-usage_before.ru_stime
    remaining=args.cpu_budget_seconds-spent if args.cpu_budget_seconds else math.inf
    if remaining<5:
        attempts.append({"command":list(map(str,command)),"status":"budget-not-started"})
        return
    seconds=min(remaining-2,cap if cap is not None else math.inf)
    def limits():
        resource.setrlimit(resource.RLIMIT_CORE,(0,0))
        ceiling=5*1024**3//2
        resource.setrlimit(resource.RLIMIT_AS,(ceiling,ceiling))
        if math.isfinite(seconds):
            resource.setrlimit(resource.RLIMIT_CPU,(max(1,int(seconds)),max(2,int(seconds)+1)))
    with log.open("x") as stream:
        process=subprocess.run(["taskset","-c",str(args.cpu),*map(str,command)],env=env,stdout=stream,stderr=subprocess.STDOUT,preexec_fn=limits)
    attempts.append({"command":list(map(str,command)),"returncode":process.returncode,"status":"complete" if process.returncode==0 else "incomplete"})
    if process.returncode not in (0,-24,-9):
        raise RuntimeError(f"diagnostic failed: {log}")
if args.bounded_seconds:
    run_probe([bounded,out/"bounded",str(args.bounded_seconds)],out/"bounded.log",args.bounded_seconds+1)
for n in args.populations:
    if sha(binary) != pins["binarySha256"]:
        raise RuntimeError("cliff binary changed")
    run_probe([binary,str(n),out/f"{n}.json",str(args.ticks)],out/f"{n}.log")
usage_after=resource.getrusage(resource.RUSAGE_CHILDREN)
user_cpu=usage_after.ru_utime-usage_before.ru_utime
system_cpu=usage_after.ru_stime-usage_before.ru_stime
(out/"collection-cpu.json").write_text(json.dumps({"userSeconds":user_cpu,"systemSeconds":system_cpu,"budgetSeconds":args.cpu_budget_seconds,"attempts":attempts},indent=2)+"\n")
if args.prior_cpu_seconds+user_cpu+system_cpu>args.total_control_budget_seconds:
    raise RuntimeError("aggregate control CPU budget exceeded")
with (root/"uncapped-status.json").open("x") as stream:
    json.dump({"status":"unsettled","reason":"The maintaining agent capped the uncapped 10k control; convergence not established",
               "cpuBudgetSeconds":args.total_control_budget_seconds,"priorChargedSeconds":args.prior_cpu_seconds,
               "isolatedDiagnosticCpuSeconds":user_cpu+system_cpu,"diagnostic":"cliff-probe",
               "minimumLengthScreenTicks":30000},stream,indent=2)
    stream.write("\n")
