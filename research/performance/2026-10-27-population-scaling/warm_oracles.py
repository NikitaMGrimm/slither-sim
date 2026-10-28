"""Check complete long warmup continuations against retained original search.

python3 warm_oracles.py BUILT_OUTPUT ORIGINAL_OUTPUT PILOT_OUTPUT
The old raw directories are local-only; fresh reproduction can instead generate
original-search warmups with SCALING_ORIGINAL_WARM=1 and the plain executable.
"""
import filecmp
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import os
import argparse

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("root", type=Path)
parser.add_argument("original", type=Path, nargs="?")
parser.add_argument("pilot", type=Path, nargs="?")
parser.add_argument("--cpu", type=int, default=6)
args = parser.parse_args()
os.sched_setaffinity(0, {args.cpu})
if (args.original is None) != (args.pilot is None):
    parser.error("supply both retained reference directories, or neither")
root = args.root
if args.original is not None:
    original, pilot = args.original, args.pilot
else:
    original = root / "original-search"
    original.mkdir()
    pilot = original / "pilot-500-max12"
dest = root / "oracle-500-max12"
def warm(cpu, folder, blocks, log, full_search=False):
    env = dict(os.environ, RAYON_NUM_THREADS="1", CARGO_BUILD_JOBS="1", RUST_TEST_THREADS="1")
    env.pop("SCALING_ORIGINAL_WARM", None)
    if full_search:
        env["SCALING_ORIGINAL_WARM"] = "1"
    with log.open("x") as stream:
        subprocess.run(["taskset", "-c", str(cpu), root / "plain/study_scaling", "warm", folder, "500", "common", str(blocks)],
                       env=env, stdout=stream, stderr=subprocess.STDOUT, check=True)

if args.original is None:
    warm(args.cpu, original / "500-common", 24, original / "500-common.log", True)
    warm(args.cpu, pilot, 12, original / "pilot-500-max12.log", True)

warm(args.cpu, root / "500-common", 24, root / "500-common-warm.log")
warm(args.cpu, dest, 12, root / "oracle-500-max12.log")
rows = []
for left, right in [(dest, pilot), (root / "500-common", original / "500-common")]:
    digests = {}
    for name in ["world.json", "driver.json", "warmup.json"]:
        if not filecmp.cmp(left / name, right / name, shallow=False):
            raise RuntimeError(f"warmup continuation changed: {left}/{name}")
        digests[name] = hashlib.sha256((left / name).read_bytes()).hexdigest()
    rows.append({"accelerated": str(left), "original": str(right), "equal": True, "sha256": digests})
with (root / "warmup-oracles.json").open("x") as stream:
    json.dump(rows, stream, indent=2)
    stream.write("\n")
