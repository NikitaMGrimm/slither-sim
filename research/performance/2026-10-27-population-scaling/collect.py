"""Collect the frozen rows sequentially on one logical CPU; no service.

python3 collect.py REPOSITORY BUILT_OUTPUT --cpu 6
Each measured child is pinned to one CPU. Existing
outputs fail closed rather than replacing an attempt. Build first.
"""
import argparse
from pathlib import Path
import subprocess
import sys
import filecmp
import os
import json
import resource


def worker(cpu, cases):
    for case in cases:
        modes = ["verify", "reference", "count", "profile"] if case in ARGS.prepared else ["warm", "verify", "reference", "count", "profile"]
        if case in ARGS.prepared and not (Path(ARGS.output) / case / "summary.json").is_file():
            raise RuntimeError(f"prepared warmup is incomplete: {case}")
        for mode in modes:
            subprocess.run([sys.executable, str(Path(__file__).with_name("reproduce.py")), mode,
                            ARGS.repository, ARGS.output, "--cpu", str(cpu), "--case", case], check=True)
        # All repeated endpoints must already be byte-identical. Hard links
        # retain every logical artifact without storing the same large world
        # seven times on the isolated VM. Counter arrays remain distinct.
        root=Path(ARGS.output)
        canonical=root/f"{case}-count-0/world.json"
        references = json.loads((root / "original-references.json").read_text())["cases"][case]
        for reference in references:
            for name in ["world.json", "driver.json"]:
                if not filecmp.cmp(root / reference["folder"] / name, root / f"{case}-count-0" / name, shallow=False):
                    raise RuntimeError("prepared admission changed the complete original-search continuation")
        if case == "500-common" and (root / "public-import-oracle").is_dir():
            for name in ["world.json", "driver.json"]:
                if not filecmp.cmp(root / "public-import-oracle" / name, root / f"{case}-count-0" / name, shallow=False):
                    raise RuntimeError("bounded-memory preparation changed the original importer continuation")
        for suffix in ["count--1","count-1","count-2","count-3","count-4","profile"]:
            path=root/f"{case}-{suffix}/world.json"
            if not filecmp.cmp(canonical,path,shallow=False):
                raise RuntimeError(f"cannot deduplicate changed endpoint: {path}")
            link=path.with_name("world.json.same-bytes")
            os.link(canonical,link)
            os.replace(link,path)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repository")
    parser.add_argument("output")
    parser.add_argument("--cpu", type=int, default=6)
    parser.add_argument("--prepared", nargs="*", default=[], help="Completed retained warmups; counting outputs must still be new")
    parser.add_argument("--skip-bots", action="store_true", help="Bot component was already collected and validated")
    parser.add_argument("--resource-summary", action="store_true", help="Write the complete serial invocation's child peak RSS")
    parser.add_argument("--cases", nargs="+", default=["500-common", "10000-capped", "5000-uncapped", "5000-capped", "2000-common", "1000-common"])
    ARGS = parser.parse_args()
    os.sched_setaffinity(0, {ARGS.cpu})
    os.environ.update(RAYON_NUM_THREADS="1", CARGO_BUILD_JOBS="1", RUST_TEST_THREADS="1")
    if not (Path(ARGS.output) / "checkpoint-checks.json").is_file():
        subprocess.run([sys.executable, str(Path(__file__).with_name("reproduce.py")), "preparation-checks",
                        ARGS.repository, ARGS.output, "--cpu", str(ARGS.cpu)], check=True)
    if "500-common" in ARGS.cases:
        if "500-common" not in ARGS.prepared:
            subprocess.run([sys.executable, str(Path(__file__).with_name("warm_oracles.py")), ARGS.output,
                            "--cpu", str(ARGS.cpu)], check=True)
        if not (Path(ARGS.output) / "public-import-oracle/source.json").is_file():
            subprocess.run([sys.executable, str(Path(__file__).with_name("reproduce.py")), "import-oracle",
                            ARGS.repository, ARGS.output, "--cpu", str(ARGS.cpu)], check=True)
        if "500-common" not in ARGS.prepared:
            for mode in ["verify", "reference", "count", "profile"]:
                subprocess.run([sys.executable, str(Path(__file__).with_name("reproduce.py")), mode,
                                ARGS.repository, ARGS.output, "--cpu", str(ARGS.cpu), "--case", "500-common"], check=True)
            ARGS.cases.remove("500-common")
    worker(ARGS.cpu, ARGS.cases)
    if not ARGS.skip_bots:
        subprocess.run([sys.executable, str(Path(__file__).with_name("reproduce.py")), "bots",
                        ARGS.repository, ARGS.output, "--cpu", str(ARGS.cpu)], check=True)
    if ARGS.resource_summary:
        with (Path(ARGS.output) / "sequential-resource.json").open("x") as stream:
            json.dump({"affinity": sorted(os.sched_getaffinity(0)), "cargoJobs": 1, "rayonThreads": 1,
                       "maximumIndividualChildRssKiB": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
                       "addressSpaceLimitBytes": 5*1024**3//2}, stream, indent=2)
            stream.write("\n")
