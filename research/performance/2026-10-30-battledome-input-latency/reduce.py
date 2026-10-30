"""Remote-only reduction of buffered probe records, captures and shipped CLI stats.

python3 reduce.py ROOT OUTPUT_JSON
No endpoint or cloud identifier is copied into the selected result.
"""
from pathlib import Path
import bisect
import collections
import hashlib
import json
import math
import re
import statistics
import struct
import sys
import tarfile


def stats(values):
    values = sorted(values)
    if not values:
        return {"n": 0}
    return {"n": len(values), "min": values[0], "mean": statistics.mean(values),
            **{name: values[max(0, math.ceil(len(values) * fraction) - 1)]
               for name, fraction in [("p50", .5), ("p95", .95), ("p99", .99), ("max", 1.)]}}


def pcap_frames(path, server_direction=True, include_payload=False):
    """Reassemble one plaintext server flow, retaining complete-frame capture times.

    Linux receive/GRO capture timestamps are kernel observations, not physical
    wire timestamps. Overlapping/retransmitted payloads are counted explicitly.
    """
    pieces = []
    with path.open("rb") as stream:
        header = stream.read(24)
        assert len(header) == 24
        endian, divisor = {b"\xd4\xc3\xb2\xa1": ("<", 1e6), b"\x4d\x3c\xb2\xa1": ("<", 1e9),
                           b"\xa1\xb2\xc3\xd4": (">", 1e6), b"\xa1\xb2\x3c\x4d": (">", 1e9)}[header[:4]]
        link = struct.unpack(endian + "I", header[20:24])[0]
        offset = {1: 14, 113: 16, 276: 20}[link]
        while record := stream.read(16):
            assert len(record) == 16
            seconds, fraction, length, original = struct.unpack(endian + "IIII", record)
            packet = stream.read(length)
            assert len(packet) == length == original, "Truncated packet capture"
            ip = packet[offset:]
            if not ip or ip[0] >> 4 != 4 or ip[9] != 6:
                continue
            assert int.from_bytes(ip[6:8], "big") & 0x3fff == 0, "Fragmented IP packet"
            ihl = (ip[0] & 15) * 4
            total = int.from_bytes(ip[2:4], "big")
            tcp = ip[ihl:total]
            if len(tcp) < 20:
                continue
            port_at = 0 if server_direction else 2
            if int.from_bytes(tcp[port_at:port_at + 2], "big") != 8766:
                continue
            data = tcp[(tcp[12] >> 4) * 4:]
            if data:
                pieces.append((int.from_bytes(tcp[4:8], "big"), data, seconds + fraction / divisor))
    assert pieces, "No captured server data"
    base = pieces[0][0]
    pieces.sort(key=lambda piece: ((piece[0] - base) & 0xffffffff, piece[2]))
    data = bytearray()
    spans = []
    overlaps = 0
    for sequence, payload, timestamp in pieces:
        begin = (sequence - base) & 0xffffffff
        assert begin <= len(data), "TCP capture has a gap or missing initial segment"
        overlap = min(len(data) - begin, len(payload))
        if overlap:
            assert data[begin:begin + overlap] == payload[:overlap]
            overlaps += 1
        if overlap < len(payload):
            start = len(data)
            data.extend(payload[overlap:])
            spans.append((start, len(data), timestamp))
    prefix = b"HTTP/1.1 101" if server_direction else b"GET "
    assert data.startswith(prefix), "Missing HTTP upgrade"
    cursor = data.index(b"\r\n\r\n") + 4
    ends = [span[1] for span in spans]
    frames = []
    while cursor + 2 <= len(data):
        begin = cursor
        first, second = data[cursor:cursor + 2]
        cursor += 2
        assert first & 0x80, "Fragmented frame"
        assert bool(second & 0x80) != server_direction, "Unexpected mask direction"
        length = second & 127
        if length == 126:
            if cursor + 2 > len(data): break
            length = int.from_bytes(data[cursor:cursor + 2], "big")
            cursor += 2
        elif length == 127:
            if cursor + 8 > len(data): break
            length = int.from_bytes(data[cursor:cursor + 8], "big")
            cursor += 8
        mask = bytes(data[cursor:cursor + 4]) if second & 0x80 else None
        if mask is not None:
            cursor += 4
        end = cursor + length
        if end > len(data): break
        first_span = bisect.bisect_right(ends, begin)
        last_span = bisect.bisect_left(ends, end)
        completed = max(span[2] for span in spans[first_span:last_span + 1])
        if first & 15 == 2:
            payload = data[cursor:end]
            if mask is not None:
                payload = bytes(value ^ mask[index % 4] for index, value in enumerate(payload))
            frame = {"sha256": hashlib.sha256(payload).hexdigest(), "unix": completed}
            if include_payload:
                frame["payloadHex"] = payload.hex()
            frames.append(frame)
        cursor = end
    return frames, {"payloadSegments": len(pieces), "overlappingSegments": overlaps,
                    "binaryFrames": len(frames), "unparsedTailBytes": len(data) - cursor}


def load_records(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line]


def host_intervals(path):
    values = load_records(path)
    result = []
    previous = None
    for value in values:
        match = re.search(r"ticks/s=([\d.]+) deferred_ticks=(\d+) simulation_load=([\d.]+)%", value["line"])
        if match:
            if previous is not None:
                result.append({"start": previous, "end": value["unix"], "deferred": int(match[2]),
                               "ticksPerSecond": float(match[1]), "simulationLoad": float(match[3])})
            previous = value["unix"]
    return result


def classify(start, end, intervals):
    selected = [v for v in intervals if v["start"] <= end and v["end"] >= start]
    if not selected or selected[0]["start"] > start or selected[-1]["end"] < end:
        return "uncovered"
    if any(v["end"] - v["start"] > .25 for v in selected):
        return "reporting-gap"
    return "deferred-bucket" if any(v["deferred"] for v in selected) else "normal-bucket"


def icmp_records(path):
    values = []
    for line in path.read_text().splitlines():
        match = re.search(r"\[([\d.]+)\].*icmp_seq=\d+.*time=([\d.]+) ms", line)
        if match:
            values.append({"unix": float(match[1]), "milliseconds": float(match[2])})
    return values


def nearest(values, when):
    if not values:
        return None
    # Bounded summaries have only a few thousand pings; no clock assumptions
    # beyond the explicitly retained two-second pairing limit.
    value = min(values, key=lambda value: abs(value["unix"] - when))
    return value if abs(value["unix"] - when) <= 2 else None


def histogram_summary(reports, field):
    counts = collections.Counter()
    for report in reports:
        for value, count in report[field]:
            counts[value] += count
    size = sum(counts.values())
    result = {"n": size}
    if not size:
        return result
    # Preserve #262/#281's ceil((n-1)*p) order statistic for the inherited
    # worker histograms. Probe latency uses the plan's nearest-rank convention.
    for name, fraction in [("p50", .5), ("p95", .95), ("p99", .99), ("max", 1.)]:
        rank, seen = math.ceil((size - 1) * fraction), 0
        for value, count in sorted(counts.items()):
            seen += count
            if seen > rank:
                result[name] = value / 1000
                break
    return result


def summarize_row(directory, intervals, icmp, clients):
    window = json.loads((directory / "window.json").read_text())
    start, end = window["startUnix"], window["endUnix"]
    records = load_records(directory / "probe.jsonl")
    assert records[-1]["type"] == "complete"
    inside = lambda value: start <= value["unix"] < end
    sends = {value["id"]: value for value in records if value["type"] == "send" and inside(value)}
    echoes = [value for value in records if value["type"] == "echo" and value["id"] in sends and inside(value)]
    censors = [value for value in records if value["type"] == "censored" and value["id"] in sends]
    endpoints = {value["id"] for value in echoes + censors}
    boundary = len(sends) - len(endpoints)
    assert boundary >= 0 and len(endpoints) == len(echoes) + len(censors)
    controls = [value for value in records if value["type"] == "controlRtt"]
    groups = collections.defaultdict(list)
    corrected = {"icmp": [], "control": []}
    for value in echoes:
        sent = sends[value["id"]]
        assert value["life"] == sent["life"] and value["wanted"] == sent["expected"]
        assert abs(value["milliseconds"] - (value["at"] - sent["at"]) * 1000) < 1e-6
        groups[classify(sent["unix"], value["unix"], intervals)].append(value["milliseconds"])
        life_controls = [ping for ping in controls if ping["life"] == sent["life"]]
        for name, pings in [("icmp", icmp), ("control", life_controls)]:
            ping = nearest(pings, sent["unix"])
            if ping is not None:
                corrected[name].append(value["milliseconds"] - ping["milliseconds"])
    captures = []
    packet_by_frame = {}
    commands_by_life = {}
    ages, gaps, packet_gaps, residuals, packet_residuals, excess, packet_excess = [], [], [], [], [], [], []
    worst = []
    matched = 0
    for life in sorted({value["life"] for value in records if value["type"] == "frame"}):
        frames = [value for value in records if value["type"] == "frame" and value["life"] == life]
        # Primary userspace timing does not depend on packet-capture success.
        user_minimum = math.inf
        prior = None
        for frame in frames:
            if inside(frame):
                value = frame["at"] * 1000 - frame["headerMs"]
                if frame["playing"]:
                    user_minimum = min(user_minimum, value)
                if frame["wasPlaying"] and frame["playing"]:
                    excess.append(value - user_minimum)
                    if prior is not None:
                        gap = (frame["at"] - prior["at"]) * 1000
                        header = frame["headerMs"] - prior["headerMs"]
                        gaps.append(gap)
                        residuals.append(gap - header)
                        worst.append({"life": life, "frame": frame["seq"], "gapMs": gap,
                                      "headerDeltaMs": header, "residualMs": gap - header})
            prior = frame
        log = (directory / f"capture-{life}.log").read_text()
        drop_match = re.search(r"(\d+) packets dropped by kernel", log)
        drops = int(drop_match[1]) if drop_match else None
        if drops != 0:
            captures.append({"life": life, "verified": False, "recordedFrames": len(frames),
                             "droppedPackets": drops, "reason": "capture loss or missing drop census"})
            continue
        try:
            captured, audit = pcap_frames(directory / f"capture-{life}.pcap")
            outgoing, outgoing_audit = pcap_frames(directory / f"capture-{life}.pcap", False, True)
            assert audit["overlappingSegments"] == outgoing_audit["overlappingSegments"] == 0
            assert all(frame["sha256"] == packet["sha256"] for frame, packet in zip(frames, captured))
        except (AssertionError, KeyError, ValueError) as error:
            captures.append({"life": life, "verified": False, "recordedFrames": len(frames),
                             "droppedPackets": drops, "reason": "capture reassembly/identity unavailable: " + str(error)})
            continue
        commands_by_life[life] = [dict(command=int(value["payloadHex"], 16), unix=value["unix"])
                                  for value in outgoing if len(value["payloadHex"]) == 2
                                  and int(value["payloadHex"], 16) <= 250]
        missing_tail = max(0, len(frames) - len(captured))
        minimum = packet_minimum = math.inf
        previous = None
        for frame, packet in zip(frames, captured):
            assert frame["sha256"] == packet["sha256"], "Capture/frame identity mismatch"
            matched += 1
            packet_by_frame[life, frame["seq"]] = packet
            if inside(frame):
                age = (frame["unix"] - packet["unix"]) * 1000
                assert age >= -.1, "Wall clocks stepped or capture matching failed"
                value = frame["at"] * 1000 - frame["headerMs"]
                wire_value = packet["unix"] * 1000 - frame["headerMs"]
                # The stock challenge precedes the game's setup/header epoch.
                # Admission time between challenge and first Playing frame is
                # not delivery excess. Reset the baseline at gameplay, as the
                # direct-login load clients effectively do at their first setup.
                if frame["playing"]:
                    minimum = min(minimum, value)
                    packet_minimum = min(packet_minimum, wire_value)
                if frame["wasPlaying"] and frame["playing"]:
                    ages.append(age)
                    packet_excess.append(wire_value - packet_minimum)
                    if previous is not None:
                        prior_frame, prior_packet = previous
                        gap = (frame["at"] - prior_frame["at"]) * 1000
                        wire_gap = (packet["unix"] - prior_packet["unix"]) * 1000
                        header = frame["headerMs"] - prior_frame["headerMs"]
                        packet_gaps.append(wire_gap)
                        packet_residuals.append(wire_gap - header)
            previous = frame, packet
        captures.append({"life": life, "verified": True, **audit, "recordedFrames": len(frames),
                         "missingTailFrames": missing_tail, "outgoing": outgoing_audit})
    kernel_effect, write_path, echo_read_age, app_minus_kernel = [], [], [], []
    used_commands = set()
    unmatched_capture_echoes = 0
    for echo in echoes:
        sent = sends[echo["id"]]
        response = packet_by_frame.get((echo["life"], echo["frame"]))
        if response is None:
            unmatched_capture_echoes += 1
            continue
        choices = [(index, value) for index, value in enumerate(commands_by_life[echo["life"]])
                   if value["command"] == sent["command"]
                   and sent["unix"] - .0001 <= value["unix"] <= response["unix"]
                   and (echo["life"], index) not in used_commands]
        if len(choices) != 1:
            unmatched_capture_echoes += 1
            continue
        index, command = choices[0]
        used_commands.add((echo["life"], index))
        duration = (response["unix"] - command["unix"]) * 1000
        assert duration >= 0
        kernel_effect.append(duration)
        write_path.append((command["unix"] - sent["unix"]) * 1000)
        echo_read_age.append((echo["unix"] - response["unix"]) * 1000)
        app_minus_kernel.append(echo["milliseconds"] - duration)
    row_clients = []
    previous_report = {}
    for value in clients:
        prior = previous_report.get(value["worker"])
        previous_report[value["worker"]] = value["unix"]
        if prior is not None and start <= prior and value["unix"] <= end:
            row_clients.append(value)
    workers = {worker: [value for value in row_clients if value["worker"] == worker]
               for worker in range(1, 7)} if window["seats"] > 1 else {}
    qualification = []
    for worker, values in workers.items():
        if len(values) < 300:
            qualification.append({"worker": worker, "reports": len(values), "passed": False,
                                  "reason": "insufficient whole-window worker reports"})
            continue
        reports_complete = (values[0]["unix"] - start < 2.5 and end - values[-1]["unix"] < 2.5
                            and max((b["unix"] - a["unix"] for a, b in zip(values, values[1:])), default=0) < 2.5)
        cpu = stats([value["cpuFraction"] for value in values])
        late = max(value["lateP99Nanos"] for value in values) / 1e6
        actions = sum(value["actions"] for value in values)
        boost = sum(value["boostActions"] for value in values) / actions
        qualification.append({"worker": worker, "reports": len(values), "cpuFraction": cpu,
                              "worstInputP99Ms": late, "boostActionShare": boost,
                              "wholeWindowReportCoverage": reports_complete,
                              "passed": reports_complete and not any(value["thin"] for value in values)
                              and cpu["p50"] < .7 and cpu["p99"] < .8 and late < 42 and boost >= .99})
    deferred = sum(value["deferred"] for value in intervals if start <= value["end"] < end)
    enclosing = sum(value["deferred"] for value in intervals if value["start"] <= end and value["end"] >= start)
    return {
        "label": window["label"], "seats": window["seats"], "seconds": end - start,
        "attempts": len(sends), "confirmed": len(echoes), "censored": dict(collections.Counter(v["reason"] for v in censors)),
        "unobservedAtBoundary": boundary, "inputToEffectMs": stats([v["milliseconds"] for v in echoes]),
        "latencyByStatsBucketMs": {key: stats(value) for key, value in groups.items()},
        "rttMs": {kind: stats([v["milliseconds"] for v in records if v["type"] == kind + "Rtt" and inside(v)])
                  for kind in ["control", "game"]} | {"icmp": stats([v["milliseconds"] for v in icmp if inside(v)])},
        "latencyMinusRttMs": {name: stats(values) for name, values in corrected.items()},
        "captureEffectMs": {"outgoingToIncoming": stats(kernel_effect), "sendToOutgoing": stats(write_path),
                            "echoReadAge": stats(echo_read_age), "appMinusKernel": stats(app_minus_kernel),
                            "unmatchedEchoes": unmatched_capture_echoes},
        "sendCallMs": stats([v["callSeconds"] * 1000 for v in sends.values()]),
        "sendLatenessMs": stats([v["lateSeconds"] * 1000 for v in sends.values()]),
        "observedDeferredTicks": deferred,
        "enclosingStatsDeferredTicks": enclosing,
        "generator": qualification,
        "generatorQualified": all(value["passed"] for value in qualification),
        "loadClientFrameMetricsMs": {name: histogram_summary(row_clients, key)
                                     for name, key in [("gap", "gapsMicros"),
                                                       ("gapResidual", "gapResidualMicros"),
                                                       ("excess", "frameExcessMicros")]},
        "loadClientResidualStalls": {str(limit): sum(count for value in row_clients
                                                     for key, count in value["gapResidualMicros"]
                                                     if key > limit * 1000) for limit in [20, 100, 250]},
        "loadWorkerMaxPassMs": stats([value["maxPassNanos"] / 1e6 for value in row_clients]),
        "probeFramesMatched": matched, "captures": captures,
        "captureCoverage": {
            "primaryPlayingFrames": sum(value["type"] == "frame" and inside(value)
                                         and value["wasPlaying"] and value["playing"] for value in records),
            "verifiedPrimaryPlayingFrames": len(ages),
            "verifiedInputEchoes": len(kernel_effect), "unmatchedInputEchoes": unmatched_capture_echoes,
        },
        "frameMetricsMs": {"readAge": stats(ages), "gap": stats(gaps), "packetGap": stats(packet_gaps),
                           "gapResidual": stats(residuals), "packetGapResidual": stats(packet_residuals),
                           "excess": stats(excess), "packetExcess": stats(packet_excess)},
        "stalls": {name: {str(limit): sum(v > limit for v in values) for limit in [20, 100, 250]}
                   for name, values in [("rawGap", gaps), ("packetGap", packet_gaps),
                                        ("gapResidual", residuals), ("packetGapResidual", packet_residuals)]},
        "largestTenGaps": sorted(worst, key=lambda value: value["gapMs"], reverse=True)[:10],
        "minimumSampleQuotaPass": len(echoes) >= 300,
    }


def main():
    root, output = Path(sys.argv[1]), Path(sys.argv[2])
    host = root / "host-extracted"
    host.mkdir(exist_ok=True)
    with tarfile.open(root / "server.tar.gz") as archive:
        archive.extractall(host, filter="data")
    clients = [json.loads(line[7:]) for line in (root / "clients.jsonl").read_text().splitlines()
               if line.startswith("CLIENT ")]
    icmp = icmp_records(root / "icmp.log")
    rows = []
    for label in ["01-solo", "02-80", "03-160", "04-320"]:
        intervals = host_intervals(host / ("solo.jsonl" if label == "01-solo" else "loaded.jsonl"))
        rows.append(summarize_row(root / "rows" / label, intervals, icmp, clients))
    pins = json.loads((root / "pins.json").read_text())
    output.write_text(json.dumps({"pins": pins, "rows": rows}, indent=2) + "\n")
    assert all(row["minimumSampleQuotaPass"] for row in rows), "Insufficient confirmed samples"
    print("All four rows qualified and matched to zero-loss captures")


if __name__ == "__main__":
    main()
