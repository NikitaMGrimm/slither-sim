"""Recompute the retained alternating CPU replay series using only the stdlib."""
import argparse
import csv
import re
from decimal import Decimal
from pathlib import Path
from statistics import median

ARMS = {
    "control": ("6a5ea7a01d708b7c86024fcc501184942f2f9ea9e628eb85103aa402d307f840",
                "def8a9bec210bffed5b1c1c990ac3d04b9266c0e9eadf74da094a69522430d93"),
    "candidate": ("f799dc8469d55acc5abad8a08a7e61daf8a6cbd6cbe845c8c8534e0ebf2a9fd7",
                  "3152c7b95e7e17c97e0d35a184170827d6c6901aca5d4e1d87808a94f08641a4"),
}
BEGIN = "<!-- BEGIN RECOMPUTED RUN AGGREGATES -->"
END = "<!-- END RECOMPUTED RUN AGGREGATES -->"
EXPECTED_PAIRS = 64


def read_runs(path):
    with path.open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    runs = {}
    for row in rows:
        pair, arm = int(row["pair"]), row["arm"]
        if pair < 1 or arm not in ARMS or (pair, arm) in runs:
            raise ValueError("invalid or duplicate pair/arm")
        if (row["binary_sha256"], row["source_patch_sha256"]) != ARMS[arm]:
            raise ValueError("binary/source snapshot does not match arm")
        if row["source_base_commit"] != "7ce9569285e2b6aec22f725675865ee5b3fe9a08":
            raise ValueError("source base mismatch")
        if row["driver_sha256"] != "47c88eff74a0b88aa21765609e55743844ff0a8a944ff2438bae4fe79d0eae65":
            raise ValueError("driver mismatch")
        observations, compiler = int(row["process_snapshots"]), int(row["rustc_snapshots"])
        if observations < 2 or not 0 <= compiler <= observations:
            raise ValueError("invalid process observations")
        expected = ("excluded", "observed_rustc") if compiler else ("accepted", "none")
        if (row["status"], row["exclusion_reason"]) != expected:
            raise ValueError("acceptance disagrees with compiler observations")
        if tuple(int(row[k]) for k in ("callbacks", "populated_callbacks", "messages", "skips", "exit_code")) != (366, 302, 32106, 0, 0):
            raise ValueError("different or incomplete replay")
        if row["command_digest"] != "62b9cab3a98e7ac6":
            raise ValueError("command output mismatch")
        if row["state_observation_sha256"] != "7e733b1c7bc1ae450cd60b81f6907f9728943a56f649f6137c38cdc569327d5a":
            raise ValueError("per-frame state observations differ")
        for field in ("frames_csv_sha256", "process_log_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", row[field]):
                raise ValueError("invalid retained-output hash")
        for field, value in row.items():
            if field.endswith(("_ns", "_bytes", "_s", "_percent")):
                number = Decimal(value)
                if not number.is_finite() or number < 0:
                    raise ValueError("invalid numeric summary")
                row[field] = number
            elif field.endswith("_snapshots") and not 0 <= int(value) <= observations:
                raise ValueError("invalid process count")
        for stage in ("callback", "ingress", "redraw"):
            if row[stage + "_p50_ns"] > row[stage + "_p95_ns"]:
                raise ValueError("unordered quantiles")
        if any(row[k] > 100 for k in ("cpu_load_before_percent", "cpu_load_after_percent")):
            raise ValueError("invalid CPU load")
        runs[pair, arm] = row
    pairs = {pair for pair, arm in runs}
    if pairs != set(range(1, EXPECTED_PAIRS + 1)) or set(runs) != {(pair, arm) for pair in pairs for arm in ARMS}:
        raise ValueError("every consecutive pair and both arms, including exclusions, are required")
    return runs


def recompute(runs):
    count = len(runs) // 2
    accepted = {arm: [r for (pair, a), r in sorted(runs.items()) if a == arm and r["status"] == "accepted"] for arm in ARMS}
    if any(not rows for rows in accepted.values()):
        raise ValueError("each arm needs accepted runs")

    def med(arm, field):
        return median(r[field] for r in accepted[arm])

    def span(arm, field):
        values = [r[field] for r in accepted[arm]]
        return f"{median(values):.3f} s ({min(values):.3f}-{max(values):.3f})"

    def quantiles(arm, prefix):
        return f"{med(arm, prefix + '_p50_ns') / 1_000_000:.2f} / {med(arm, prefix + '_p95_ns') / 1_000_000:.2f} ms"

    def bursts(arm):
        values = [r["max_callback_wall_ns"] / 1_000_000 for r in accepted[arm]]
        return f"{median(values):.2f} ms ({min(values):.2f}-{max(values):.2f})"

    def tail_share(arm):
        values = [r["slow16_ingress_ns"] / r["slow16_wall_ns"] * 100 for r in accepted[arm]]
        return f"{median(values):.1f}% ({min(values):.1f}-{max(values):.1f})"

    def reduction(field):
        return (1 - med("candidate", field) / med("control", field)) * 100

    loads = [r[k] for r in runs.values() for k in ("cpu_load_before_percent", "cpu_load_after_percent")]
    lines = [
        f"{count} alternating pairs: accepted control/candidate **{len(accepted['control'])} / {len(accepted['candidate'])}**; "
        f"excluded **{count - len(accepted['control'])} / {count - len(accepted['candidate'])}** for observed rustc.",
        f"CPU-load snapshot range: **{min(loads):.0f}-{max(loads):.0f}%**.", "",
        "| Fixed 6.1 s history | Main control | Received-snapshot candidate |", "| --- | ---: | ---: |",
    ]
    for label, fmt in [
        ("Launcher elapsed, median (range)", lambda a: span(a, "launcher_elapsed_s")),
        ("Process CPU, median (range)", lambda a: span(a, "process_cpu_s")),
        ("Requested allocation bytes, median", lambda a: f"{med(a, 'requested_allocation_bytes') / 1_000_000_000:.2f} GB"),
        ("Populated callback wall, p50 / p95", lambda a: quantiles(a, "callback")),
        ("Populated ingress, p50 / p95", lambda a: quantiles(a, "ingress")),
        ("Populated redraw preparation, p50 / p95", lambda a: quantiles(a, "redraw")),
        ("Largest populated callback, median (range)", bursts),
        ("Ingress share of slowest 16 callbacks, median (range)", tail_share),
    ]:
        lines.append(f"| {label} | {fmt('control')} | {fmt('candidate')} |")
    lines += ["", f"Reductions: callback p95 **{reduction('callback_p95_ns'):.1f}%**; "
              f"median launcher elapsed **{reduction('launcher_elapsed_s'):.1f}%**; "
              f"requested allocation bytes **{reduction('requested_allocation_bytes'):.1f}%**.", "",
              "| Accepted pair | Control elapsed | Candidate elapsed | Elapsed reduction | Control / candidate callback p95 |",
              "| --- | ---: | ---: | ---: | ---: |"]
    paired_elapsed, paired_p95 = [], []
    for pair in range(1, count + 1):
        b, a = (runs[pair, arm] for arm in ARMS)
        if b["status"] == a["status"] == "accepted":
            paired_elapsed.append((1 - a["launcher_elapsed_s"] / b["launcher_elapsed_s"]) * 100)
            paired_p95.append((1 - a["callback_p95_ns"] / b["callback_p95_ns"]) * 100)
            lines.append(f"| {pair} | {b['launcher_elapsed_s']:.3f} s | {a['launcher_elapsed_s']:.3f} s | "
                         f"{(1 - a['launcher_elapsed_s'] / b['launcher_elapsed_s']) * 100:.1f}% | "
                         f"{b['callback_p95_ns'] / 1_000_000:.2f} / {a['callback_p95_ns'] / 1_000_000:.2f} ms |")
    lines += [""]
    if paired_p95:
        lines.append(f"Complete-pair reduction, median (range): callback p95 **{median(paired_p95):.1f}% "
                     f"({min(paired_p95):.1f}-{max(paired_p95):.1f}%)**; launcher elapsed **{median(paired_elapsed):.1f}% "
                     f"({min(paired_elapsed):.1f}-{max(paired_elapsed):.1f}%)**.")
        lines.append("")
    for arm in ARMS:
        ids = [str(pair) for pair in range(1, count + 1) if runs[pair, arm]["status"] == "accepted"]
        lines.append(f"Accepted {arm} pairs: {', '.join(ids)}.")
    return "\n".join(lines) + "\n"


def recompute_profiles(folder):
    with (folder / "profile-stages.csv").open(encoding="utf-8", newline="") as stream:
        stages = list(csv.DictReader(stream))
    with (folder / "profile-messages.csv").open(encoding="utf-8", newline="") as stream:
        messages = list(csv.DictReader(stream))
    by_message = {(r["arm"], int(r["opcode"])): r for r in messages}
    if len(by_message) != len(messages):
        raise ValueError("duplicate profile opcode")
    for arm in ARMS:
        if sum(int(r["count"]) for r in messages if r["arm"] == arm) != 32106:
            raise ValueError("profile workload mismatch")
    if {int(r["opcode"]): r["count"] for r in messages if r["arm"] == "control"} != {int(r["opcode"]): r["count"] for r in messages if r["arm"] == "candidate"}:
        raise ValueError("profile message counts differ")

    def seconds(arm, stage, context=None):
        return sum((Decimal(r["duration_ns"]) for r in stages if r["arm"] == arm
                    and r["stage"] == stage and (r["context"] == context if context else r["context"].startswith("packet_"))), Decimal(0)) / 1_000_000_000

    copies = ["host_world_clone", "core_received_clone", "received_rollback_clone", "core_world_clone"]

    def other_events(arm):
        return seconds(arm, "presentation_events") - seconds(arm, "presentation_events", "packet_99")

    def other_receive(arm):
        total = sum(Decimal(r["receive_ns"]) for r in messages if r["arm"] == arm) / 1_000_000_000
        return total - sum(seconds(arm, s) for s in copies) - seconds(arm, "received_mutation") - seconds(arm, "presentation_events")
    lines = ["", "Separate passive-attribution runs (excluded from the alternating timing series):", "",
             "| Ingress interval | Main control | Candidate |", "| --- | ---: | ---: |"]
    for label, stage, context in [
        ("Host world snapshot", "host_world_clone", None),
        ("Core received pre-copy", "core_received_clone", None),
        ("Received rollback snapshot", "received_rollback_clone", None),
        ("Core presentation snapshot", "core_world_clone", None),
        ("Received decode/mutation after snapshot", "received_mutation", None),
        ("Consume presentation events", "presentation_events", "packet_99"),
    ]:
        lines.append(f"| {label} | {seconds('control', stage, context):.4f} s | {seconds('candidate', stage, context):.4f} s |")
    lines.append(f"| Sum of the four snapshot intervals | {sum(seconds('control', s) for s in copies):.4f} s | {sum(seconds('candidate', s) for s in copies):.4f} s |")
    lines.append(f"| Other presentation events | {other_events('control'):.4f} s | {other_events('candidate'):.4f} s |")
    lines.append(f"| Other receive work, including validation/commit/teardown | {other_receive('control'):.4f} s | {other_receive('candidate'):.4f} s |")
    lines += ["", "| Leading opcode | Messages | Control receive | Candidate receive | Control / candidate requested bytes |",
              "| --- | ---: | ---: | ---: | ---: |"]
    top = sorted((r for r in messages if r["arm"] == "control"), key=lambda r: int(r["receive_ns"]), reverse=True)[:8]
    for before in top:
        opcode = int(before["opcode"])
        after = by_message["candidate", opcode]
        lines.append(f"| {chr(opcode)} | {before['count']} | {Decimal(before['receive_ns']) / 1_000_000_000:.4f} s | "
                     f"{Decimal(after['receive_ns']) / 1_000_000_000:.4f} s | "
                     f"{Decimal(before['alloc_bytes']) / 1_000_000_000:.3f} / {Decimal(after['alloc_bytes']) / 1_000_000_000:.3f} GB |")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="also verify the generated REPORT.md block")
    args = parser.parse_args()
    folder = Path(__file__).resolve().parent
    result = recompute(read_runs(folder / "runs.csv")) + recompute_profiles(folder)
    print(result, end="")
    if args.check:
        report = (folder / "REPORT.md").read_text(encoding="utf-8")
        if report.split(BEGIN + "\n", 1)[1].split(END, 1)[0] != result + "\n":
            raise ValueError("REPORT.md aggregate block differs from runs.csv")
        print("REPORT.md aggregate block matches runs.csv.")


if __name__ == "__main__":
    main()
