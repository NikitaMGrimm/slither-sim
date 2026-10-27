"""Replay the declared inputs serially at receipt/60Hz, server/60Hz and receipt/125Hz.

Private input JSON: [{"label": "official-a", "capture": "/external/capture.jsonl.gz"}].
Do not commit that private locator file. Existing result directories are never replaced.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys
from replay import limit_cpus


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--inputs", type=Path, required=True)
    p.add_argument("--binary", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    a = p.parse_args()
    limit_cpus()
    a.output.mkdir(parents=True, exist_ok=False)
    inputs = json.loads(a.inputs.read_text(encoding="utf-8"))
    for entry in inputs:
        label = entry["label"]
        if not label or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in label):
            raise ValueError("labels must contain only lowercase ASCII letters, digits and hyphens")
        for name, clock, tick in (("receipt60", "receipt", 1000 / 60),
                                  ("server60", "server", 1000 / 60),
                                  ("receipt125", "receipt", 8)):
            subprocess.run([sys.executable, str(Path(__file__).with_name("replay.py")),
                entry["capture"], "--binary", str(a.binary), "--label", label,
                "--output", str(a.output / (label + "-" + name)),
                "--clock", clock, "--tick-ms", str(tick), "--seconds", "120"], check=True)


if __name__ == "__main__":
    main()
