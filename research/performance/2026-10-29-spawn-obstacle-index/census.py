"""Recompute geometry census from the external frozen physics checkpoint."""
from pathlib import Path
import hashlib
import json
import math
import statistics as stats
import sys

path, destination = map(Path, sys.argv[1:3])
world = json.loads(path.read_text())
snakes = [slot["snake"] for slot in world["state"]["slots"] if slot["snake"]["alive"]]


def distance(a, b):
    return math.hypot(a["x"] - b["x"], a["y"] - b["y"])


def distribution(values):
    values = sorted(values)
    return {"n": len(values), "min": values[0], "mean": stats.mean(values),
            **{name: values[math.ceil((len(values) - 1) * q)]
               for name, q in [("p50", .5), ("p90", .9), ("p99", .99)]}, "max": values[-1]}


spacing = []
for snake in snakes:
    points = [snake["head"], *snake["body"]]
    spacing.extend(distance(a, b) for a, b in zip(points, points[1:]))
result = {"checkpointSha256": hashlib.sha256(path.read_bytes()).hexdigest(),
          "quantile": "ceil((n-1)*q)", "living": len(snakes),
          "actualRadius": world["state"]["radius"],
          "configuredBaseRadius": world["config"]["radius"],
          "bodyPoints": distribution([len(s["body"]) for s in snakes]),
          "bodyPointSpacing": distribution(spacing),
          "nearestHeadDistance": distribution([
              min(distance(a["head"], b["head"]) for b in snakes if a is not b) for a in snakes])}
destination.write_text(json.dumps(result, indent=2) + "\n", newline="\n")
