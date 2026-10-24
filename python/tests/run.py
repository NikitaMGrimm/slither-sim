"""Run the package tests against a built wheel without installing it (standard library only).

Unpacks the newest slither_sim wheel in WHEELS into WHEELS/unpacked, imports it
from there and runs python/tests/test_*.py.
"""

import argparse
from pathlib import Path
import shutil
import sys
import unittest
import zipfile


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("wheels", type=Path, help="directory holding the built wheel, such as target/wheels")
    args = parser.parse_args(argv)
    wheels = sorted(args.wheels.glob("slither_sim-*.whl"), key=lambda path: path.stat().st_mtime)
    if not wheels:
        parser.error(f"no slither_sim wheel in {args.wheels}; build one with maturin build")
    unpacked = args.wheels / "unpacked"
    shutil.rmtree(unpacked, ignore_errors=True)
    with zipfile.ZipFile(wheels[-1]) as wheel:
        wheel.extractall(unpacked)
    sys.path.insert(0, str(unpacked.resolve()))
    import slither_sim

    if not Path(slither_sim.__file__).resolve().is_relative_to(unpacked.resolve()):
        raise SystemExit(f"imported slither_sim from {slither_sim.__file__}, not {unpacked}")
    print(f"Testing {wheels[-1].name}", flush=True)
    tests = unittest.defaultTestLoader.discover(str(Path(__file__).resolve().parent), pattern="test_*.py")
    return 0 if unittest.TextTestRunner().run(tests).wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
