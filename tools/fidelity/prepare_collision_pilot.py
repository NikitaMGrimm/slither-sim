"""Bind the unexecuted collision pilot to a retained selection (offline only)."""

import argparse
import ipaddress
import json
from pathlib import Path
import tempfile

import evidence


TEMPLATE = evidence.ROOT / "research/fidelity/2026-10-26-collision-pilot/plan.json"
# Official servers close Origin-less handshakes before any frame: all five
# 2026-10-26 scouts without one closed with 1006, while the P1/P2 collectors
# with this Origin joined from the same host that day.
OFFICIAL_ORIGIN = "http://slither.io"


def select(selection):
    """Rank first leaderboard populations; no HTTP or game probes here."""
    if not evidence.sha256_string(selection.get("serverListSha256")):
        raise ValueError("retain the raw official list digest in serverListSha256")
    rows = selection["candidates"]
    if not 1 <= len(rows) <= 5:
        raise ValueError("require 1..5 sequential scouting results in decoded list order")
    indices = [row["listedIndex"] for row in rows]
    if any(type(index) is not int or index < 0 for index in indices) or indices != sorted(set(indices)):
        raise ValueError("listedIndex must increase in decoded active-list order")
    for row in rows:
        address = ipaddress.ip_address(row["server"]["ip"])
        if address.version != 4 or not 1 <= row["server"]["port"] <= 65535:
            raise ValueError("require IPv4 endpoints and valid ports")
        if not evidence.sha256_string(row.get("archiveSha256")):
            raise ValueError("each scouting result needs its retained archive digest")
        population = row["population"]
        if population is not None and (type(population) is not int or not 0 <= population <= 65535):
            raise ValueError("population must be first observed leaderboard count or null")
    eligible = [row for row in rows if row["population"] is not None and row["population"] <= 100]
    if not eligible:
        raise ValueError("no eligible low-population endpoint; do not start a pilot")
    return min(eligible, key=lambda row: (row["population"], row["listedIndex"]))["server"]


def prepare(selection_path, output, origin=OFFICIAL_ORIGIN):
    selection = evidence.read_json(selection_path)
    server = select(selection)
    plan = evidence.read_json(TEMPLATE)
    plan["capture"] = {"server": server, "scenario": "paired-collision", "seconds": 600, "bots": 2}
    if origin is not None:
        plan["capture"]["origin"] = origin
    evidence.validate(plan, "study-plan")
    with tempfile.TemporaryDirectory() as temporary:
        source = Path(temporary) / "plan.json"
        evidence.write_json(source, plan)
        study = evidence.create_study(source, output)
    (study / "selection.json").write_bytes(Path(selection_path).read_bytes())
    (study / "pilot-template.json").write_bytes(TEMPLATE.read_bytes())
    return study


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--origin", default=OFFICIAL_ORIGIN)
    args = parser.parse_args()
    try:
        print(json.dumps({"study": str(prepare(args.selection, args.output, args.origin)), "connected": False}))
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
