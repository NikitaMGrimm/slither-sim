"""Private command inventories around ladder windows, on the control core."""
from pathlib import Path
import json
import os
import subprocess
import time

r = Path(__file__).resolve().parent
os.sched_setaffinity(0, {7})
previous = None
while True:
    try:
        state = json.loads((r / "run-status.json").read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        time.sleep(.05)
        continue
    phase = state.get("phase")
    current = (phase, state.get("bots")) if phase in ["counting", "confirm-counting"] else None
    if current != previous:
        for label, boundary in [(previous, "end"), (current, "start")]:
            if label is not None:
                path = r / "load-run" / f"generator-{label[0]}-{label[1]}-{boundary}.txt"
                raw = subprocess.check_output(["ps", "-eo", "pid,ppid,stat,pcpu,args", "-ww"], text=True)
                path.write_text(f"unix={time.time()}\n" + raw)
        previous = current
    if state.get("status") in ["complete", "failed"]:
        break
    time.sleep(.05)
