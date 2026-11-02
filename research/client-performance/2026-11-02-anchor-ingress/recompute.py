"""Recompute independent and incremental non-growing-anchor comparisons (stdlib)."""
import argparse
import csv
import json
from decimal import Decimal
from pathlib import Path
from statistics import median

BEGIN = "<!-- BEGIN SUMMARY -->"
END = "<!-- END SUMMARY -->"


def load(folder, incremental):
    identities = json.loads((folder / "identities.json").read_text(encoding="utf-8"))
    arms = ("combined", "allthree") if incremental else ("control", "anchor")
    with (folder / ("combined-runs.csv" if incremental else "runs.csv")).open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    indexed = {}
    for r in rows:
        key = int(r["pair"]), int(r["zoom"]), r["arm"]
        if key in indexed or key[0] < 1 or key[1] not in (0, -120) or key[2] not in ("control", "candidate"):
            raise ValueError("duplicate or invalid run")
        binary = identities["binaries"][arms[key[2] == "candidate"]]
        if r["binary_sha256"] != binary or r["source_base"] != identities["base"][:8]:
            raise ValueError("arm/binary/source mismatch")
        if r["state_sha256"] != identities["state_sha256"][str(key[1])]:
            raise ValueError("frame observation mismatch")
        if tuple(int(r[k]) for k in ("callbacks", "selected_callbacks", "messages", "skips", "exit")) != (366, 302, 32106, 0, 0) or r["command_digest"] != "62b9cab3a98e7ac6":
            raise ValueError("incomplete/different replay")
        n, compiler = int(r["process_snapshots"]), int(r["rustc_snapshots"])
        if n < 2 or not 0 <= compiler <= n:
            raise ValueError("invalid process observations")
        if (r["status"], r["reason"]) != (("excluded", "observed_rustc") if compiler else ("accepted", "none")):
            raise ValueError("wrong exclusion")
        for k, v in r.items():
            if k.endswith(("_ns", "_s", "_bytes")) or k.startswith("load_"):
                number = Decimal(v)
                if not number.is_finite() or number < 0:
                    raise ValueError("invalid metric")
                r[k] = number
        for stage in ("callback", "ingress", "redraw"):
            if not r[stage + "_p50_ns"] <= r[stage + "_p95_ns"] <= r[stage + "_max_ns"]:
                raise ValueError("unordered quantiles")
        if any(not 0 <= r[k] <= 100 for k in ("load_before", "load_after")):
            raise ValueError("invalid CPU load")
        indexed[key] = r
    expected = {(p, z, a) for p in range(1, identities["pairs"] + 1) for z in (0, -120) for a in ("control", "candidate")}
    if set(indexed) != expected:
        raise ValueError("missing pair/view/arm, including exclusions")
    return indexed


def render(folder):
    lines = []
    for incremental in (False, True):
        runs = load(folder, incremental)
        title = "Incremental: consume + snake snapshots / all three fixes" if incremental else "Independent: main / anchor fast path"
        lines += [f"**{title}**", ""]
        for zoom in (0, -120):
            arm_rows = {a: [r for (p, z, arm), r in sorted(runs.items()) if z == zoom and arm == a and r["status"] == "accepted"] for a in ("control", "candidate")}
            if any(not rows for rows in arm_rows.values()):
                raise ValueError("no accepted runs in a cell")

            def med(a, field):
                return median(r[field] for r in arm_rows[a])

            def span(a, field, scale, unit):
                values = [r[field] / scale for r in arm_rows[a]]
                return f"{median(values):.3f} ({min(values):.3f}-{max(values):.3f}) {unit}"

            loads = [r[k] for (p, z, a), r in runs.items() if z == zoom for k in ("load_before", "load_after")]
            lines += [f"View **{'default' if zoom == 0 else 'whole map'}**, accepted control/candidate "
                      f"**{len(arm_rows['control'])}/{len(arm_rows['candidate'])}**; load **{min(loads):.0f}-{max(loads):.0f}%**.", "",
                      "| Medians of accepted per-run summaries | Control | Candidate |", "| --- | ---: | ---: |"]
            metrics = [("Elapsed, median (range)", lambda a: span(a, "elapsed_s", 1, "s")),
                       ("Process CPU, median (range)", lambda a: span(a, "cpu_s", 1, "s")),
                       ("Allocation requests", lambda a: f"{med(a, 'alloc_bytes') / 1_000_000_000:.3f} GB")]
            for stage in ("callback", "ingress", "redraw"):
                metrics.append((stage + " p50 / p95", lambda a, stage=stage: f"{med(a, stage + '_p50_ns') / 1_000_000:.3f} / {med(a, stage + '_p95_ns') / 1_000_000:.3f} ms"))
            metrics.append(("Largest callback, median (range)", lambda a: span(a, "callback_max_ns", 1_000_000, "ms")))
            for label, fmt in metrics:
                lines.append(f"| {label} | {fmt('control')} | {fmt('candidate')} |")
            lines += ["", "| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |", "| --- | ---: | ---: | ---: |"]
            deltas = []
            for pair in sorted({p for p, z, a in runs}):
                b, a = runs[pair, zoom, "control"], runs[pair, zoom, "candidate"]
                if b["status"] == a["status"] == "accepted":
                    delta = (1 - a["callback_p95_ns"] / b["callback_p95_ns"]) * 100
                    deltas.append(delta)
                    lines.append(f"| {pair} | {b['callback_p95_ns'] / 1_000_000:.3f} / {a['callback_p95_ns'] / 1_000_000:.3f} ms | {delta:.1f}% | {(1 - a['elapsed_s'] / b['elapsed_s']) * 100:.1f}% |")
            if deltas:
                lines += ["", f"Paired p95 reduction median (range): **{median(deltas):.1f}% ({min(deltas):.1f}-{max(deltas):.1f}%)**."]
            lines += [""]
    with (folder / "profiles.csv").open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    profiles = {(r["view"], r["arm"], r["kind"], r["key"]): Decimal(r["ns"]) / 1_000_000_000 for r in rows}
    if len(profiles) != len(rows):
        raise ValueError("duplicate profile row")
    lines += ["Separate whole-map diagnostic attribution (excluded from timing):", "",
              "| Interval/family | Main | Consume + snapshots | Anchor | All three |", "| --- | ---: | ---: | ---: | ---: |"]
    for kind, key, label in [("stage", "host_snapshot", "Host snapshots"), ("stage", "received_snapshot", "Received snapshots"),
                             ("stage", "core_snapshot", "Core snapshots"), ("packet", "103", "976 anchors"),
                             ("packet", "110", "225 growth anchors"), ("packet", "99", "639 consumes")]:
        values = [profiles["zoom", a, kind, key] for a in ("control", "combined", "anchor", "allthree")]
        lines.append(f"| {label} | " + " | ".join(f"{v:.4f} s" for v in values) + " |")
    with (folder / "residual.csv").open(encoding="utf-8", newline="") as stream:
        residual = list(csv.DictReader(stream))
    lines += ["", "Remaining all-three whole-map diagnostic families:", "",
              "| Family | Complete-history time | Populated callback contribution p95 / max | In largest callback |",
              "| --- | ---: | ---: | ---: |"]
    for r in sorted((r for r in residual if r["view"] == "zoom"), key=lambda r: int(r["receive_ns"]), reverse=True)[:8]:
        lines.append(f"| {chr(int(r['opcode']))} | {Decimal(r['receive_ns']) / 1_000_000_000:.4f} s | "
                     f"{Decimal(r['populated_frame_p95_ns']) / 1_000_000:.3f} / {Decimal(r['populated_frame_max_ns']) / 1_000_000:.3f} ms | "
                     f"{Decimal(r['in_largest_callback_ns']) / 1_000_000:.3f} ms |")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    folder = Path(__file__).resolve().parent
    result = render(folder)
    print(result, end="")
    if args.check:
        report = (folder / "REPORT.md").read_text(encoding="utf-8")
        if report.split(BEGIN + "\n", 1)[1].split(END, 1)[0] != result + "\n":
            raise ValueError("report aggregates differ from retained data")
        print("All independent/incremental anchor tables match retained summaries.")
