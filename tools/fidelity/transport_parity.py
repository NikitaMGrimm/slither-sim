"""Offline protocol-19 transport measurements and empirical reference gates.

See docs/research-workflow.md. No collection or network access happens here.
"""

import argparse
from collections import Counter, defaultdict, deque
import json
import math
from pathlib import Path
import statistics
import sys
import zlib

import packet_census as pc
import slither_protocol as sp
import transport_views as tv


ROOT = Path(__file__).resolve().parents[2]
# Six completed, unfiltered near-server batches; the older transport-official-reference.json is a
# retained home-route/collector case, not a server delivery envelope.
DEFAULT_REFERENCE = ROOT / "tests/fixtures/transport-official-near-server.json"
DEFAULT_TOLERANCES = Path(__file__).with_name("transport_tolerances.json")
MEASUREMENT_ERRORS = (OSError, ValueError, IndexError, KeyError, TypeError, EOFError, zlib.error)
GATES = {
    "anchor_receipt_p50": ("anchors.receipt.all.abs.p50", "world units"),
    "anchor_receipt_p95": ("anchors.receipt.all.abs.p95", "world units"),
    "frame_server_p50": ("frameGapsMs.server.p50", "ms"),
    "delay_p90": ("excessDelayMs.p90", "ms"),
}


def quantiles(values):
    ordered = sorted(values)
    result = {"n": len(ordered)}
    for p in (10, 50, 90, 95, 99):
        result[f"p{p}"] = ordered[max(0, math.ceil(len(ordered) * p / 100) - 1)] if ordered else None
    return result


def rolling_excess(offsets, radius=600):
    """Inclusive centered rolling minimum in O(n), separately for each epoch."""
    queue, end, result = deque(), 0, []
    for i, offset in enumerate(offsets):
        while end < min(len(offsets), i + radius + 1):
            while queue and offsets[queue[-1]] >= offsets[end]:
                queue.pop()
            queue.append(end)
            end += 1
        while queue[0] < i - radius:
            queue.popleft()
        result.append(offset - offsets[queue[0]])
    return result


class Samples:
    def __init__(self):
        self.gaps = {clock: [] for clock in ("receipt", "server")}
        self.anchors = {clock: {group: [] for group in ("all", "big")}
                        for clock in ("receipt", "server")}
        self.motion = {group: [] for group in ("own", "other")}
        self.motion_tracks = Counter()
        self.skipped = Counter()
        self.delays = []
        self.families = Counter()
        self.frames = 0
        self.durations = Counter()

    def merge(self, other):
        for clock in self.gaps:
            self.gaps[clock].extend(other.gaps[clock])
            for group in self.anchors[clock]:
                self.anchors[clock][group].extend(other.anchors[clock][group])
        for group in self.motion:
            self.motion[group].extend(other.motion[group])
        self.motion_tracks.update(other.motion_tracks)
        self.skipped.update(other.skipped)
        self.delays.extend(other.delays)
        self.families.update(other.families)
        self.frames += other.frames
        self.durations.update(other.durations)

    def summary(self):
        def correction(values):
            return {"n": len(values), "rms": math.sqrt(statistics.fmean(x*x for x in values)) if values else None,
                    "abs": quantiles([abs(x) for x in values])}
        return {
            "frames": self.frames,
            "durationMs": dict(self.durations),
            "framesPerSecond": {clock: 1000 * self.frames / duration if duration else None
                                for clock, duration in self.durations.items()},
            "frameGapsMs": {clock: quantiles(values) for clock, values in self.gaps.items()},
            "headerDeltaHistogramMs": dict(sorted(Counter(self.gaps["server"]).items())),
            "anchors": {clock: {group: correction(values) for group, values in groups.items()}
                        for clock, groups in self.anchors.items()},
            "anchorSkippedWindows": dict(self.skipped),
            "motionGapsMs": {group: {**quantiles(values), "snakeTracks": self.motion_tracks[group]}
                             for group, values in self.motion.items()},
            "excessDelayMs": quantiles(self.delays),
            "packetFamilies": [dict(command=command, length=length, packets=count,
                                    perSecond={clock: 1000*count/duration if duration else None
                                               for clock, duration in self.durations.items()})
                               for (command, length), count in sorted(self.families.items())],
        }


class Epoch:
    def __init__(self, connection, setup_index, receipt):
        self.connection, self.setup_index, self.setup_receipt = connection, setup_index, receipt
        self.world = sp.World(contract=False)
        self.anchors = defaultdict(list)
        self.motion = defaultdict(list)
        self.incarnations = Counter()
        self.samples = Samples()
        self.frames = []
        self.view = tv.ViewEpoch()

    def feed(self, frame):
        self.frames.append((frame.t, frame.server))
        was_active, counts = self.view.active, Counter()
        for packet in frame.packets:
            if not packet:
                raise ValueError("empty logical packet")
            self.world.feed(packet, frame.server)
            self.view.packet(packet, self.world, self.world.events, counts)
            # Consume immediately: later packets in a bundle may change the speed,
            # remove the snake or reuse its ID.
            for event in self.world.events:
                if event[0] not in ("spawn", "remove", "anchor", "rot"):
                    continue
                sid = event[2]
                if event[0] in ("spawn", "remove"):
                    self.incarnations[sid] += 1
                key = (sid, self.incarnations[sid])
                if event[0] == "anchor":
                    snake = self.world.snakes[sid]
                    self.anchors[key].append((frame.t, frame.server, snake.sp, snake.sct))
                elif event[0] == "rot":
                    group = "own" if sid == self.world.self_id else "other"
                    self.motion[(group, key)].append((frame.t, frame.server))
            self.world.events.clear()
        self.view.frame(frame, self.world, was_active, counts)
        # Only body/motion state is used; avoid retaining food and prey histories.
        self.world.foods.clear()
        self.world.prey.clear()

    def finish(self, interval=None):
        # Optional corpus selection keeps the original clock/quantile formulas.
        # The clean-path reference requires whole gap/anchor windows to pass
        # the frozen telemetry mask, never bridges removed frames:
        # research/fidelity/2026-10-26-server-timing-references/REPORT.md
        eligible = interval or (lambda a, b: True)
        samples = self.samples
        # Setup's header is an epoch, not elapsed time. Exclude its frame from
        # delay samples and cadence, but include its time as the first gap origin.
        pairs = [(a, b) for a, b in zip(self.frames, self.frames[1:]) if eligible(a[0], b[0])]
        samples.frames = len(pairs)
        for index, clock in enumerate(("receipt", "server")):
            samples.gaps[clock] = [b[index] - a[index] for a, b in pairs]
            samples.durations[clock] = (sum(samples.gaps[clock]) if interval else
                                        self.frames[-1][index] - self.frames[0][index])
        delays = rolling_excess([receipt - server for receipt, server in self.frames[1:]])
        samples.delays = [value for i, value in enumerate(delays)
                          if eligible(self.frames[i][0], self.frames[i+1][0])]
        for run in self.anchors.values():
            for k in range(5, len(run) - 4):
                window = run[k-5:k+5]
                if not eligible(window[0][0], window[-1][0]):
                    samples.skipped["selection"] += 1
                    continue
                speeds = [row[2] for row in window]
                if min(speeds) <= 0 or max(speeds) > 1.15 * min(speeds):
                    samples.skipped["speedVariation"] += 1
                    continue
                for index, clock in enumerate(("receipt", "server")):
                    span = run[k+4][index] - run[k-5][index]
                    if span <= 0:
                        samples.skipped[clock + "ZeroSpan"] += 1
                        continue
                    # Prototype's local speed estimator: nine anchor intervals
                    # centered on this interval, each spanning 42 world units.
                    error = 9 * 42 / span * (run[k][index] - run[k-1][index]) - 42
                    samples.anchors[clock]["all"].append(error)
                    if run[k][3] >= 100:
                        samples.anchors[clock]["big"].append(error)
        for (group, _), times in self.motion.items():
            samples.motion_tracks[group] += 1
            # Include zero and long gaps; filtering them would hide batching/stalls.
            samples.motion[group].extend(b[1]-a[1] for a, b in zip(times, times[1:])
                                        if eligible(a[0], b[0]))
        return samples


def measure_capture(source, selection=None):
    source = Path(source)
    archive_sha = pc.sha256(source)
    # Reuse census validation (complete gzip/JSONL, timestamps, framing, literal
    # protocol 19, footer counts) and its packet-family counts, not a second parser.
    census = pc.census(source)
    canonical = [segment for segment in census["connections"] if segment["setupReceiptMs"] is not None]
    if any(segment["framing"] != "elapsed_header" for segment in census["connections"]):
        raise ValueError("elapsed-ms framing required")
    epochs, total, epoch, setup_index = [], Samples(), None, 0
    views = tv.ViewSamples()
    connection = None
    header, terminal, collector = {}, {}, []
    servers = set()

    def finish():
        nonlocal epoch
        if epoch is not None:
            interval = selection(epoch) if selection else None
            samples = epoch.finish(interval)
            views.merge(epoch.view.finish(interval or (lambda a, b: True)))
            total.merge(samples)
            epochs.append(dict(connection=epoch.connection, setupIndex=epoch.setup_index,
                               setupReceiptMs=epoch.setup_receipt, summary=samples.summary()))
            epoch = None

    for record in sp.read(source):
        if isinstance(record, dict):
            if record.get("type") == "header":
                header = record
            server = record.get("server")
            if isinstance(server, dict):
                servers.add((server.get("ip"), server.get("port")))
            if record.get("event") in ("connect", "close", "connection-failed"):
                finish()
            if record.get("event") == "collector-timing":
                collector.append(record)
            if record.get("event") == "capture-end":
                terminal = record
            continue
        if not isinstance(record, sp.Frame):
            continue
        if record.conn != connection:
            finish()
            connection, setup_index = record.conn, 0
        if any(packet[:1] == b"a" for packet in record.packets):
            finish()
            setup_index += 1
            epoch = Epoch(connection, setup_index, record.t)
            segment = canonical[len(epochs)]
            epoch.samples.families = Counter({(row["command"], row["length"]): row["packets"]
                                              for row in segment["families"]})
        if epoch is not None:
            epoch.feed(record)
    finish()
    if len(canonical) != len(epochs):
        raise ValueError("decoder/census setup epoch mismatch (elapsed headers required)")
    timing = [*collector]
    if isinstance(terminal.get("collectorTiming"), dict):
        timing.append(terminal["collectorTiming"])
    if pc.sha256(source) != archive_sha:
        raise ValueError("capture changed during measurement")
    return dict(path=str(source), sha256=archive_sha, servers=[dict(ip=ip, port=port) for ip, port in sorted(servers)],
                scenario=header.get("scenario"), connections=epochs,
                collectorTiming=dict(events=len(collector), available=bool(timing),
                                     lateTicks=max((row.get("lateTicks", 0) for row in timing), default=None),
                                     maxTickGapMs=max((row.get("maxTickGapMs", 0) for row in timing), default=None)),
                summary=total.summary(), viewStrata=views.summary(quantiles)), total


def measure(paths, skip_invalid=False, summary_only=False, relative_to=None):
    captures, excluded, total, hashes = [], [], Samples(), set()
    for path in paths:
        display = str(Path(path).relative_to(relative_to).as_posix()) if relative_to else str(path)
        try:
            report, samples = measure_capture(path)
        except MEASUREMENT_ERRORS as error:
            if not skip_invalid:
                raise
            excluded.append(dict(path=display, sha256=pc.sha256(Path(path)), reason=str(error)))
            continue
        if report["sha256"] in hashes:
            raise ValueError("duplicate capture SHA-256")
        hashes.add(report["sha256"])
        report["path"] = display
        if summary_only:
            report["connectionSetupEpochs"] = len(report.pop("connections"))
            report["summary"].pop("packetFamilies")
            report["summary"].pop("headerDeltaHistogramMs")
        captures.append(report)
        total.merge(samples)
    if not captures:
        raise ValueError("no valid captures")
    summary = total.summary()
    if summary_only:
        summary.pop("packetFamilies")
        summary.pop("headerDeltaHistogramMs")
    return dict(schemaVersion=1, wireVersion=19, status="complete", summaryOnly=summary_only,
                toolSha256=pc.sha256(Path(__file__)), decoderSha256=pc.sha256(Path(sp.__file__)),
                viewToolSha256=pc.sha256(Path(tv.__file__)),
                censusSha256=pc.sha256(Path(pc.__file__)), captures=captures, excluded=excluded, summary=summary,
                definitions=dict(quantiles="nearest rank, including p50", anchorSpacingWorldUnits=42,
                                 big="at least 100 body anchors at the measured anchor (not lifetime reports)",
                                 speed="9 * 42 / span of 9 neighboring intervals; skip >15% reported speed spread",
                                 delay="receipt minus server time, minus inclusive rolling minimum over +/-600 post-setup frames",
                                 cadence="setup-to-last-frame duration; setup excluded from frame count and delay; gaps start at setup",
                                 families="census command/length counts including setup, divided by setup-to-last duration",
                                 pooling="pool raw samples within epochs/captures; never average quantiles",
                                 viewStrata="transport-view-v1: active-to-active gaps by preceding cache peers; time-weighted state; complete stable server-second windows including silence; see docs/research-workflow.md#transport-parity",
                                 limits="correlated samples; receipt includes network/collector stalls; no stock-renderer simulation"))


def lookup(summary, path):
    result = summary
    for part in path.split("."):
        result = result[part]
    return result


def compare(reference, candidate, tolerances):
    for report in (reference, candidate):
        if report.get("schemaVersion") != 1 or report.get("wireVersion") != 19 or report.get("status") != "complete":
            raise ValueError("expected complete transport-parity schema 1, wire 19 report")
    print("| Metric | Official range | Candidate | Upper limit | Result |")
    print("| --- | ---: | ---: | ---: | --- |")
    failed = False
    minimum = tolerances["minimumSamples"]
    for name, (path, units) in GATES.items():
        limits = tolerances["metrics"][name]
        if limits["path"] != path:
            raise ValueError(f"tolerance metric path mismatch: {name}")
        values = []
        for capture in reference["captures"]:
            summary = capture["summary"]
            value = lookup(summary, path)
            if lookup(summary, path.rsplit(".", 1)[0] + ".n") >= minimum and value is not None:
                values.append(value)
        if not values:
            raise ValueError(f"no eligible reference samples: {name}")
        low, high = min(values), max(values)
        # The integer elapsed header has 1 ms resolution. Only frame-gap p50
        # uses that spread floor; receipt-derived metrics remain continuous.
        # See the fixed LOO rule and its retained no-floor failure:
        # research/fidelity/2026-10-26-server-timing-references/REPORT.md
        upper = high + limits["spreadAllowance"] * max(high-low, limits.get("spreadFloor", 0))
        actual = lookup(candidate["summary"], path)
        n = lookup(candidate["summary"], path.rsplit(".", 1)[0] + ".n")
        ok = n >= minimum and actual is not None and math.isfinite(actual) and actual <= upper
        failed |= not ok
        shown = f"{actual:.3f}" if actual is not None else "missing"
        status = "PASS" if ok else "FAIL" if n >= minimum else f"FAIL (n={n} < {minimum})"
        print(f"| {name} ({units}) | {low:.3f}–{high:.3f} | {shown} | {upper:.3f} | {status} |")
    stalls = [capture["collectorTiming"] for capture in candidate["captures"]]
    print("Collector timing: " + json.dumps(stalls, sort_keys=True))
    return int(failed)


def compare_views(reference, candidate, tolerances):
    """Independent opt-in workload/rate gate; never changes the pooled gates."""
    for report in (reference, candidate):
        if report.get("schemaVersion") != 1 or report.get("wireVersion") != 19 or report.get("status") != "complete":
            raise ValueError("expected complete transport-parity schema 1, wire 19 report")
        if not report.get("captures"):
            raise ValueError("view comparison requires captures")
    for name, report in (("reference", reference), ("candidate", candidate)):
        if any("viewStrata" not in capture for capture in report["captures"]):
            print(f"INCONCLUSIVE: {name} lacks view bins")
            return 3
        if any(not isinstance(capture["viewStrata"], dict) for capture in report["captures"]):
            raise ValueError(f"{name} viewStrata must be an object")
        if any(capture["viewStrata"].get("contract") != tv.CONTRACT for capture in report["captures"]):
            raise ValueError(f"unsupported {name} view contract")
    limits = tolerances["viewMatched"]
    minimum = limits["minimumSeconds"]["value"]
    minimum_windows = limits["minimumWholeWindows"]["value"]
    if (not math.isfinite(minimum) or minimum <= 0 or not math.isfinite(minimum_windows)
            or minimum_windows < 1 or minimum_windows != int(minimum_windows)):
        raise ValueError("positive view exposure and whole-window minima required")

    def bound(rule, reference_value):
        absolute, relative = rule["absolute"], rule["relative"]
        if any(not math.isfinite(x) or x < 0 for x in (absolute, relative)):
            raise ValueError("view bounds must be finite and nonnegative")
        return max(absolute, relative * abs(reference_value))

    def number(value):
        return isinstance(value, (float, int)) and math.isfinite(value)

    for rule in (limits["emissionRate"], *limits["activity"].values()):
        bound(rule, 0)  # Validate configuration even when no bin is eligible.

    def fmt(value):
        return f"{value:.3f}" if number(value) else "missing"

    def validate(data):
        if set(data) != set(tv.BINS):
            raise ValueError("view bins do not match transport-view-v1")
        for row in data.values():
            for value in (row["seconds"], row["frames"], row["wholeWindows"]["seconds"], row["wholeWindows"]["frames"]):
                if not number(value) or value < 0:
                    raise ValueError("invalid view exposure/count")

    print("View contract: transport-view-v1. Mixture: each candidate capture's active exposure weights, fixed across all reference captures; no support renormalization.")
    print(f"Admission per bin/capture: >= {minimum} active seconds and >= {minimum_windows} whole seconds. Emission-rate bound: max({limits['emissionRate']['absolute']} frames/s, {limits['emissionRate']['relative']:.0%} of reference).")
    print("Each candidate/reference capture pair is validated separately. Activity differences use max(absolute, relative * reference). <=9 ms remains diagnostic only.")
    for prefix, report in (("c", candidate), ("r", reference)):
        for index, capture in enumerate(report["captures"], 1):
            print(f"{prefix}{index} capture SHA-256: {capture.get('sha256', 'unspecified')}")
    print("| Candidate / reference | Peers | Exposure s (candidate/reference) | Whole seconds (candidate/reference) | Frames/s (candidate/reference) | <=9 ms fraction (candidate/reference) | Weight | Result / activity mismatches |")
    print("| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |")
    failed, inconclusive = False, False
    for ci, capture in enumerate(candidate["captures"], 1):
        cbins = capture["viewStrata"]["bins"]
        validate(cbins)
        exposure = sum(row["seconds"] for row in cbins.values())
        if not exposure:
            inconclusive = True
        for ri, official in enumerate(reference["captures"], 1):
            rbins = official["viewStrata"]["bins"]
            validate(rbins)
            support, weighted_candidate, weighted_reference = 0, 0, 0
            pair_failed, pair_inconclusive = False, not bool(exposure)
            for label in tv.BINS:
                c, r = cbins[label], rbins[label]
                cw, rw = c["wholeWindows"], r["wholeWindows"]
                weight = c["seconds"] / exposure if exposure else 0
                eligible = (c["seconds"] >= minimum and r["seconds"] >= minimum and
                            cw["seconds"] >= minimum_windows and rw["seconds"] >= minimum_windows)
                mismatches = []
                if not eligible:
                    status = "INCONCLUSIVE (minimum exposure/windows)"
                    pair_inconclusive |= weight > 0
                else:
                    for key, rule in limits["activity"].items():
                        cv, rv = c["activity"].get(key), r["activity"].get(key)
                        # No peer state exists in the zero-peer bin; both missing
                        # is structural only there, never a general missing-data pass.
                        if label == "0" and key.startswith("peer") and cv is None and rv is None:
                            continue
                        if not number(cv) or not number(rv):
                            mismatches.append(key + "=missing")
                        elif abs(cv-rv) > bound(rule, rv):
                            mismatches.append(f"{key}={cv:.3f}/{rv:.3f} (bound {bound(rule, rv):.3f})")
                    cr, rr = cw["framesPerSecond"], rw["framesPerSecond"]
                    if not number(cr) or not number(rr):
                        status = "INCONCLUSIVE (missing emission rate)"
                        pair_inconclusive = True
                    elif mismatches:
                        status = "NOT ACTIVITY-MATCHED"
                        pair_inconclusive = True
                    else:
                        support += weight
                        weighted_candidate += weight * cr
                        weighted_reference += weight * rr
                        ok = abs(cr-rr) <= bound(limits["emissionRate"], rr)
                        status = "PASS" if ok else "FAIL (emission rate)"
                        pair_failed |= not ok
                print(f"| c{ci}/r{ri} | {label} | {c['seconds']:.3f}/{r['seconds']:.3f} | {cw['seconds']}/{rw['seconds']} | {fmt(cw['framesPerSecond'])}/{fmt(rw['framesPerSecond'])} | {fmt(c['le9Fraction'])}/{fmt(r['le9Fraction'])} | {weight:.4f} | {status}" + (": " + "; ".join(mismatches) if mismatches else "") + " |")
            # Full support only: a sparse or unmatched bin never disappears from
            # the candidate mixture, and averaging cannot rescue a failed bin.
            status = "FAIL" if pair_failed else "INCONCLUSIVE" if pair_inconclusive else "PASS"
            if not pair_inconclusive:
                ok = abs(weighted_candidate-weighted_reference) <= bound(limits["emissionRate"], weighted_reference)
                pair_failed |= not ok
                status = "FAIL" if pair_failed else "PASS"
            print(f"c{ci}/r{ri} mixture: {status}; activity-matched exposure weight={support:.4f}; weighted frames/s={weighted_candidate:.3f}/{weighted_reference:.3f} (partial sums when inconclusive).")
            failed |= pair_failed
            inconclusive |= pair_inconclusive
    print("View result: " + ("FAIL" if failed else "INCONCLUSIVE" if inconclusive else "PASS"))
    return 1 if failed else 3 if inconclusive else 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    measuring = commands.add_parser("measure")
    measuring.add_argument("captures", nargs="+", type=Path)
    measuring.add_argument("--output", required=True, type=Path)
    measuring.add_argument("--skip-invalid", action="store_true", help="record excluded paths, hashes and parse errors")
    measuring.add_argument("--summary-only", action="store_true", help="omit connections, histograms and family rows in a compact reference")
    measuring.add_argument("--relative-to", type=Path, help="root for portable input paths")
    comparison = commands.add_parser("compare")
    comparison.add_argument("reference", type=Path)
    comparison.add_argument("candidate", type=Path)
    comparison.add_argument("--tolerances", type=Path, default=DEFAULT_TOLERANCES)
    comparison.add_argument("--view-matched", action="store_true", help="independent per-capture view/activity gate: 0 pass, 1 rate failure, 2 invalid, 3 inconclusive")
    args = parser.parse_args(argv)
    try:
        if args.command == "measure":
            if args.output.exists():
                raise ValueError(f"refusing to overwrite {args.output}")
            report = measure(args.captures, args.skip_invalid, args.summary_only, args.relative_to)
            with args.output.open("x", encoding="utf-8", newline="\n") as stream:
                json.dump(report, stream, indent=2, allow_nan=False)
                stream.write("\n")
            return 0
        comparator = compare_views if args.view_matched else compare
        return comparator(*(json.loads(path.read_text(encoding="utf-8"))
                            for path in (args.reference, args.candidate, args.tolerances)))
    except MEASUREMENT_ERRORS as error:
        print(f"transport parity failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
