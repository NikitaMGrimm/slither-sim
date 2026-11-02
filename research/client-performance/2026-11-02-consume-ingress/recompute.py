"""Recompute bounded per-run tables; Python 3.11+, standard library only."""
import argparse
import csv
import json
from decimal import Decimal
from pathlib import Path
from statistics import median

BEGIN = "<!-- BEGIN RUN TABLES -->"
END = "<!-- END RUN TABLES -->"


def read_runs(folder):
    identities = json.loads((folder / "identities.json").read_text(encoding="utf-8"))
    with (folder / "runs.csv").open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    runs = {}
    for r in rows:
        key = int(r["pair"]), int(r["zoom"]), r["arm"]
        pair, zoom, arm = key
        if pair < 1 or zoom not in (0, -120) or arm not in ("control", "candidate") or key in runs:
            raise ValueError("invalid or duplicate pair/view/arm")
        if r["binary_sha256"] != identities[arm]["binary_sha256"] or r["source_base"] != identities["base"][:8]:
            raise ValueError("binary/source does not match arm")
        if r["state_sha256"] != identities["state_sha256"][str(zoom)]:
            raise ValueError("per-frame state output mismatch")
        if tuple(int(r[k]) for k in ("callbacks", "selected_callbacks", "messages", "skips", "exit")) != (366, 302, 32106, 0, 0) or r["command_digest"] != "62b9cab3a98e7ac6":
            raise ValueError("incomplete or different workload/output")
        compiler, samples = int(r["rustc_snapshots"]), int(r["process_snapshots"])
        if samples < 2 or not 0 <= compiler <= samples:
            raise ValueError("invalid compiler observations")
        expected = ("excluded", "observed_rustc") if compiler else ("accepted", "none")
        if (r["status"], r["reason"]) != expected:
            raise ValueError("acceptance inconsistent with compiler observations")
        for field, value in r.items():
            if field.endswith(("_ns", "_bytes", "_s")) or field.startswith("load_"):
                number = Decimal(value)
                if not number.is_finite() or number < 0:
                    raise ValueError("invalid numeric summary")
                r[field] = number
        for stage in ("callback", "ingress", "redraw"):
            if not r[stage + "_p50_ns"] <= r[stage + "_p95_ns"] <= r[stage + "_max_ns"]:
                raise ValueError("unordered quantiles")
        if any(not 0 <= r[k] <= 100 for k in ("load_before", "load_after")):
            raise ValueError("invalid CPU load")
        runs[key] = r
    expected_keys = {(p, z, a) for p in range(1, identities["pairs"] + 1) for z in (0, -120) for a in ("control", "candidate")}
    if set(runs) != expected_keys:
        raise ValueError("all planned pairs, views and arms (including exclusions) required")
    return runs


def render(runs):
    lines = []
    for zoom in (0, -120):
        selected = {a: [r for (p, z, arm), r in sorted(runs.items()) if z == zoom and arm == a and r["status"] == "accepted"] for a in ("control", "candidate")}
        if any(not r for r in selected.values()):
            raise ValueError("no accepted runs for a view/arm")

        def med(arm, k):
            return median(r[k] for r in selected[arm])

        def span(arm, k, scale, unit):
            values = [r[k] / scale for r in selected[arm]]
            return f"{median(values):.3f} ({min(values):.3f}-{max(values):.3f}) {unit}"

        def qs(arm, k):
            return f"{med(arm, k + '_p50_ns') / 1_000_000:.3f} / {med(arm, k + '_p95_ns') / 1_000_000:.3f} ms"

        count = len([1 for p, z, a in runs if z == zoom and a == "control"])
        loads = [r[k] for (p, z, a), r in runs.items() if z == zoom for k in ("load_before", "load_after")]
        lines += [f"View: **{'default' if zoom == 0 else 'whole map (-120 wheel notches)'}**. "
                  f"Accepted control/candidate **{len(selected['control'])}/{len(selected['candidate'])}** of {count} per arm; "
                  f"load snapshots **{min(loads):.0f}-{max(loads):.0f}%**.", "",
                  "| Per-run summary; medians across accepted runs | Control | Candidate |", "| --- | ---: | ---: |"]
        for label, fmt in [
            ("Elapsed median (range)", lambda a: span(a, "elapsed_s", 1, "s")),
            ("Process CPU median (range)", lambda a: span(a, "cpu_s", 1, "s")),
            ("Requested allocations", lambda a: f"{med(a, 'alloc_bytes') / 1_000_000_000:.3f} GB"),
            ("Callback p50 / p95", lambda a: qs(a, "callback")),
            ("Ingress p50 / p95", lambda a: qs(a, "ingress")),
            ("Redraw preparation p50 / p95", lambda a: qs(a, "redraw")),
            ("Largest callback median (range)", lambda a: span(a, "callback_max_ns", 1_000_000, "ms")),
        ]:
            lines.append(f"| {label} | {fmt('control')} | {fmt('candidate')} |")
        lines += ["", "| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |", "| --- | ---: | ---: | ---: |"]
        changes, elapsed = [], []
        for pair in range(1, count + 1):
            b, a = runs[pair, zoom, "control"], runs[pair, zoom, "candidate"]
            if b["status"] == a["status"] == "accepted":
                change = (1 - a["callback_p95_ns"] / b["callback_p95_ns"]) * 100
                delta = (1 - a["elapsed_s"] / b["elapsed_s"]) * 100
                changes.append(change); elapsed.append(delta)
                lines.append(f"| {pair} | {b['callback_p95_ns'] / 1_000_000:.3f} / {a['callback_p95_ns'] / 1_000_000:.3f} ms | {change:.1f}% | {delta:.1f}% |")
        lines += [""]
        if changes:
            lines.append(f"Paired p95 reduction median (range): **{median(changes):.1f}% ({min(changes):.1f}-{max(changes):.1f}%)**. "
                         f"Paired elapsed reduction: **{median(elapsed):.1f}% ({min(elapsed):.1f}-{max(elapsed):.1f}%)**.")
        lines.append(f"Requested-byte reduction: **{(1 - med('candidate', 'alloc_bytes') / med('control', 'alloc_bytes')) * 100:.1f}%**.")
        lines += [""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    folder = Path(__file__).resolve().parent
    result = render(read_runs(folder)) + render_profiles(folder)
    print(result)
    if args.check:
        report = (folder / "REPORT.md").read_text(encoding="utf-8")
        if report.split(BEGIN + "\n", 1)[1].split(END, 1)[0] != result + "\n":
            raise ValueError("published aggregate block differs from runs.csv")
        print("Report matches all retained run summaries.")


def render_profiles(folder):
    with (folder / "profiles.csv").open(encoding="utf-8", newline="") as stream:
        profiles = list(csv.DictReader(stream))
    keyed = {(r["view"], r["arm"], r["kind"], r["key"]): r for r in profiles}
    if len(keyed) != len(profiles):
        raise ValueError("duplicate profile row")
    lines = ["", "Separate diagnostic runs, excluded from timing:", "",
             "| View | Family/interval | Control | Candidate |", "| --- | --- | ---: | ---: |"]
    for view in ("default", "zoom"):
        for kind, key, label in [("packet", "99", "639 consumes"), ("packet", "103", "976 anchors"),
                                 ("packet", "110", "225 growth anchors"), ("stage", "host_snapshot", "Host snapshots"),
                                 ("stage", "received_snapshot", "Received snapshots"), ("stage", "core_snapshot", "Core snapshots")]:
            b, a = (keyed[view, arm, kind, key] for arm in ("control", "candidate"))
            lines.append(f"| {view} | {label} | {Decimal(b['ns']) / 1_000_000_000:.4f} s | {Decimal(a['ns']) / 1_000_000_000:.4f} s |")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    main()
