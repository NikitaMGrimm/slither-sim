"""Worked movement/steering study on SYNTHETIC local-simulator data.

    python tools/fidelity/worked_study.py --output <fresh-dir> [--plan plan.json]

Drives the `slither-sim serve` simulator with a frozen open-loop steering
schedule, encodes what a protocol-19 client would receive and send, then runs
the imported capture audit, replay exporter and Rust replay on that archive.
Every artifact is labelled local-simulator. Passing checks establish only that
this pipeline round-trips known simulator motion within wire precision; they
say nothing about official-server fidelity.
"""

import argparse
import base64
import gzip
import hashlib
import importlib.util
import json
import math
import subprocess
import sys
import tempfile
import uuid
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import capture_quality  # noqa: E402

SPEC = importlib.util.spec_from_file_location(
    "prepare_fidelity_replay", HERE / "prepare-fidelity-replay.py"
)
prepare = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(prepare)

TAU = math.tau
DATA_ORIGIN = "local-simulator"
LABEL = "SYNTHETIC local-simulator data; not an official-server capture"
CLAIM = (
    "Pipeline correctness only: known simulator motion survives protocol-19 encoding, "
    "integrity audit, fixture export and Rust replay. No official-server fidelity is established."
)
ARCHIVE = "raw/movement.jsonl.gz"

# Frozen generator parameters. They are part of this file's digest, which the
# capture header pins as collectorSha256.
SEED = 1
RADIUS = 5000.0
SIM_CONFIG = {
    "radius": RADIUS,
    "foodCount": 0,
    "ambientReplenishPerSecond": 0.0,
    "prey": {"count": 0},
    "kiosk": {"enabled": False},
}
TICK_MS = 7
TICKS = 857  # 5.999 s of server time after the setup packet
SELF_ID = 77
CENTER = 21_000
UPLINK_MS = 45.0
DOWNLINK_MS = 30.0
# Commands and pings reach the server this long before a tick boundary; the
# command takes effect in the step starting at that boundary and the pong
# leaves in that boundary's frame.
LEAD_MS = 2.5
CONTROL_EVERY_TICKS = 4
PING_EVERY_TICKS = 36
LOCAL_ORIGIN_MS = 1000.0
# Open-loop wanted heading by server time: hold a value or ramp from one.
SCHEDULE = [
    {"from": 0.0, "hold": math.pi},
    {"from": 0.5, "ramp": math.pi, "rate": 2.0},
    {"from": 1.2, "hold": 1.0},
    {"from": 1.6, "ramp": 1.0, "rate": -6.0},
    {"from": 2.6, "hold": 3.0},
    {"from": 3.4, "ramp": 3.0, "rate": 6.0},
    {"from": 4.2, "hold": 5.5},
    {"from": 5.0, "ramp": 5.5, "rate": -2.0},
]
EXPORT = {"connection": 1, "start_seconds": 2.0, "duration_seconds": 3.0}
# Pass criteria, fixed while developing this pipeline test (not blind).
WIRE_TOLERANCE = 1e-6  # world units / ms for exact-arithmetic round trips
KNOWN_STATE_ANCHOR_TOLERANCE = 0.01
NEGATIVE_CONTROL_MIN_ERROR = 0.5
NEGATIVE_CONTROL_MIN_RATIO = 100.0

PARAMETERS = {
    "seed": SEED, "simulatorConfig": SIM_CONFIG, "tickMs": TICK_MS, "ticks": TICKS,
    "selfId": SELF_ID, "wireCenter": CENTER, "uplinkMs": UPLINK_MS, "downlinkMs": DOWNLINK_MS,
    "arrivalLeadMs": LEAD_MS, "controlEveryTicks": CONTROL_EVERY_TICKS,
    "pingEveryTicks": PING_EVERY_TICKS, "localOriginMs": LOCAL_ORIGIN_MS,
    "schedule": SCHEDULE, "export": EXPORT,
}

DEFAULT_PLAN = {
    "schemaVersion": 1,
    "studyId": "worked-movement-pipeline",
    "title": "Worked movement and steering pipeline test (synthetic)",
    "kind": "pipeline-test",
    "dataOrigin": DATA_ORIGIN,
    "question": (
        "Does a known local-simulator steering trajectory survive protocol-19 encoding, "
        "capture integrity audit, bounded fixture export and Rust replay within wire precision?"
    ),
    "hypotheses": [
        "With the exact simulator state at the window start, replaying exported controls "
        "reproduces the transmitted anchors within 0.01 units.",
        "Shifting controls by one 7 ms tick or the initial heading by 0.05 rad raises the "
        "replay anchor error above 0.5 units and 100 times the known-state error.",
        "A tampered or truncated archive is quarantined by the integrity audit.",
    ],
    "method": (
        "tools/fidelity/worked_study.py drives slither-sim serve (seed 1, radius 5000, no food, "
        "prey or kiosk) for 857 ticks with a frozen open-loop schedule of held and ramped wanted "
        "headings, sent as 251-step bytes every 4 ticks with 45 ms uplink, 30 ms downlink and "
        "pings every 36 ticks. It encodes setup, spawn, E rotation, G anchor and pong packets, "
        "audits the archive, exports 2-5 s after setup with anchor and reported initial headings, "
        "replays free, and compares a separately labelled known-state control plus timing and "
        "heading perturbations."
    ),
    "sampling": {
        "unit": "one deterministic generator invocation",
        "attempts": 1,
        "selection": "One predeclared seed, schedule and export window; no retries or selection.",
        "independence": (
            "Deterministic synthetic data; a rerun reproduces identical bytes and adds no evidence."
        ),
    },
    "exclusions": [
        "Do not export from an archive whose integrity audit is not verified.",
        "Report failed checks; do not tune the schedule or window to make them pass.",
    ],
    "validation": (
        "Pipeline test only. Results cannot validate or refute any official-server rule; "
        "mechanics claims need separately planned official captures."
    ),
    "limitations": [
        "The simulator is both the data source and the replay model, so agreement is circular "
        "for mechanics and only tests encoding, clock mapping and export.",
        "Constant one-way delays, no jitter, loss or reordering; pongs and commands share one "
        "phase within the tick, which makes the RTT-based clock mapping exact by construction.",
        "Heading reports are sent every tick, denser than official traffic.",
        "One newborn two-segment snake without food, prey, boost, growth, opponents or boundary contact.",
    ],
}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def file_sha256(path):
    return sha256(Path(path).read_bytes())


def utc_now():
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def canonical(value):
    return json.dumps(value, indent=2, allow_nan=False) + "\n"


def wrap(angle):
    return math.atan2(math.sin(angle), math.cos(angle))


# Protocol-19 encoders. Field layouts follow slither_protocol.World decoding.
def u16(value):
    return int(value).to_bytes(2, "big")


def u24(value):
    return int(value).to_bytes(3, "big")


def angle24(radians):
    return u24(round(radians % TAU / TAU * 16777215))


def position(value):
    return u24(round((value + CENTER) * 5))


def setup_packet():
    return (
        b"a" + u24(CENTER) + u16(430) + u16(300) + u16(7) + bytes([48]) + u16(539)
        + u16(40) + u16(1400) + u16(33) + u16(29) + u16(430) + bytes([19, 42])
        + u16(SELF_ID) + u24(round(RADIUS))
    )


def spawn_packet(snake):
    tail_first = list(reversed(snake["body"]))
    packet = (
        b"s" + u16(SELF_ID) + angle24(snake["heading"]) + bytes([4])
        + angle24(snake["heading"]) + u16(round(snake["speed"] * 32))
        + u24(round(snake["fam"] * 16777215)) + bytes([4])
        + position(snake["head"]["x"]) + position(snake["head"]["y"]) + bytes([0, 0, 0])
        + position(tail_first[0]["x"]) + position(tail_first[0]["y"])
    )
    for previous, current in zip(tail_first[0:-2], tail_first[1:-1]):
        packet += bytes([round((current["x"] - previous["x"]) * 2) + 127,
                         round((current["y"] - previous["y"]) * 2) + 127])
    previous, current = tail_first[-2:]
    return packet + anchor_angle(previous, current)


def anchor_angle(previous, current):
    # Simulator anchors lie exactly 42 units along a 16-bit quantized angle.
    angle = math.atan2(current["y"] - previous["y"], current["x"] - previous["x"])
    return u16(round(angle % TAU / TAU * 65536) % 65536)


def rotation_packet(snake, wanted):
    byte = lambda angle: round(angle % TAU / TAU * 256) % 256
    return b"E" + u16(SELF_ID) + bytes(
        [byte(snake["heading"]), byte(wanted), round(snake["speed"] * 0.032 * 18)])


def frame(elapsed_ms, packets):
    if len(packets) == 1:
        return u16(elapsed_ms) + packets[0]
    return u16(elapsed_ms) + b"".join(u16(len(packet)) + packet for packet in packets)


def wanted_heading(seconds):
    phase = [row for row in SCHEDULE if row["from"] <= seconds + 1e-12][-1]
    if "hold" in phase:
        return phase["hold"]
    return phase["ramp"] + phase["rate"] * (seconds - phase["from"])


def command_byte(heading):
    return math.floor(heading % TAU / TAU * 251) % 251


def command_heading(byte):
    return (byte + 1e-8) / 251 * TAU  # the exporter's reconstruction


class Simulator:
    def __init__(self, executable):
        self.process = subprocess.Popen(
            [str(executable), "serve"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            text=True, encoding="utf-8")

    def request(self, value):
        self.process.stdin.write(json.dumps(value) + "\n")
        self.process.stdin.flush()
        reply = json.loads(self.process.stdout.readline())
        if "error" in reply:
            raise RuntimeError(f"simulator: {reply['error']}")
        return reply["result"]

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=30)
        self.process.stdout.close()


def simulate(executable):
    """Return per-tick true states and the commands the server applied."""
    sim = Simulator(executable)
    try:
        reset = sim.request({"op": "reset", "seed": SEED, "agents": 1, "decisionTicks": 1,
                             "config": SIM_CONFIG})
        states = [reset["observations"][0]["self"]]
        first_command = math.floor((UPLINK_MS + DOWNLINK_MS + LEAD_MS) / TICK_MS) + 1
        commands, held, action = [], None, states[0]["heading"]
        for k in range(TICKS):
            if k >= first_command and k % CONTROL_EVERY_TICKS == 0:
                byte = command_byte(wanted_heading(k * TICK_MS / 1000))
                if byte != held:
                    held, action = byte, command_heading(byte)
                    commands.append({"effectiveServerMs": k * TICK_MS, "byte": byte})
            result = sim.request({"op": "step", "actions": [{"heading": action, "boost": False}]})
            snake = result["observations"][0]["self"]
            if not snake["alive"] or result["deaths"]:
                raise RuntimeError("synthetic snake died; schedule left the safe arena")
            states.append(snake)
        return states, commands, first_command
    finally:
        sim.close()


def build_records(states, commands, first_command, provenance):
    timed = []

    def incoming(server_ms, packets, elapsed):
        receipt = LOCAL_ORIGIN_MS + server_ms + DOWNLINK_MS
        data = frame(elapsed, packets)
        timed.append((receipt, ["i", receipt, base64.b64encode(data).decode()]))

    def outgoing(effective_ms, data):
        send = LOCAL_ORIGIN_MS + effective_ms - LEAD_MS - UPLINK_MS
        timed.append((send, ["o", send, base64.b64encode(data).decode()]))

    incoming(0, [setup_packet()], 0)
    incoming(0, [spawn_packet(states[0])], 0)
    for command in commands:
        outgoing(command["effectiveServerMs"], bytes([command["byte"]]))
    pings = {k for k in range(first_command, TICKS + 1) if k % PING_EVERY_TICKS == 0}
    for k in sorted(pings):
        outgoing(k * TICK_MS, b"\xfb")
    wanted = states[0]["heading"]
    by_tick = {c["effectiveServerMs"] // TICK_MS: c["byte"] for c in commands}
    anchors = []
    for tick in range(1, TICKS + 1):
        if tick - 1 in by_tick:
            wanted = command_heading(by_tick[tick - 1])
        snake = states[tick]
        packets = [rotation_packet(snake, wanted)]
        if snake["body"][0] != states[tick - 1]["body"][0]:
            packets.append(b"G" + anchor_angle(states[tick - 1]["body"][0], snake["body"][0]))
            anchors.append({"serverMs": tick * TICK_MS, "anchor": snake["body"][0]})
        if tick in pings:
            packets.append(b"p")
        incoming(tick * TICK_MS, packets, TICK_MS)
    # Stable sort keeps the setup before the spawn frame at the same receipt time.
    timed.sort(key=lambda row: row[0])
    start = LOCAL_ORIGIN_MS - 10.0
    header = {"type": "header", "schema": 1, "t": start, "wall": 0,
              "server": {"ip": "local-simulator", "port": 1}, "dataOrigin": DATA_ORIGIN,
              "synthetic": True, "label": LABEL, "captureProvenance": provenance}
    connect = {"type": "meta", "event": "connect", "t": start,
               "server": {"ip": "local-simulator", "port": 1}}
    return [header, connect] + [record for _, record in timed], anchors


def write_archive(path, records, collector_sha):
    lines = [(json.dumps(r, separators=(",", ":"), allow_nan=False) + "\n").encode() for r in records]
    counts = {"i": {"frames": 0, "bytes": 0}, "o": {"frames": 0, "bytes": 0}}
    for record in records:
        if isinstance(record, list):
            counts[record[0]]["frames"] += 1
            counts[record[0]]["bytes"] += len(base64.b64decode(record[2]))
    payload = b"".join(lines)
    terminal = {"type": "meta", "event": "capture-end", "t": records[-1][1],
                "frames": counts["i"]["frames"], "joins": 1, "collectorEndSha256": collector_sha,
                "integrity": {"format": "sha256-jsonl-prefix-v1", "sha256": sha256(payload),
                              "lines": len(lines), "payloadBytes": len(payload), "counts": counts}}
    payload += (json.dumps(terminal, separators=(",", ":")) + "\n").encode()
    with path.open("xb") as raw, gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as gz:
        gz.write(payload)


def source_digests():
    names = ["Cargo.toml", "Cargo.lock", "examples/fidelity_replay.rs",
             *sorted(p.relative_to(ROOT).as_posix() for p in (ROOT / "src").rglob("*.rs")),
             *(f"tools/fidelity/{name}" for name in ("worked_study.py", "slither_protocol.py",
                                                   "capture_quality.py", "prepare-fidelity-replay.py"))]
    files = {name: file_sha256(ROOT / name) for name in names}
    combined = json.dumps(files, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    return {"files": files, "combinedSha256": sha256(combined)}


def git_identity():
    def git(*args):
        result = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
        return result.stdout.strip() if result.returncode == 0 else None
    status = git("status", "--porcelain")
    return {"commit": git("rev-parse", "HEAD"), "dirty": bool(status) if status is not None else None}


def build_executables():
    result = subprocess.run(
        ["cargo", "build", "--locked", "-j", "2", "--quiet", "--bin", "slither-sim",
         "--example", "fidelity_replay", "--message-format=json"],
        cwd=ROOT, capture_output=True, text=True, check=True)
    found = {}
    for line in result.stdout.splitlines():
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message.get("executable"):
            found[message["target"]["name"]] = Path(message["executable"])
    return found["slither-sim"], found["fidelity_replay"]


def replay(executable, fixture, path, log):
    path.write_text(canonical(fixture), encoding="utf-8", newline="\n")
    result = subprocess.run([str(executable), str(path), "free"], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"replay failed for {path.name}: {result.stderr.strip()}")
    log.write_text(result.stdout, encoding="utf-8", newline="\n")
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    comparisons = [r["distance"] for r in rows if r["kind"] == "anchor_comparison" and r["distance"] is not None]
    summary = rows[-1]
    return {"fixture": path.as_posix(), "anchorEventsObserved": summary["anchor_events_observed"],
            "anchorEventsSimulated": summary["anchor_events_simulated"],
            "anchorMeanError": summary["anchor_mean_error"],
            "anchorMaxError": max(comparisons) if comparisons else None,
            "firstDivergences": summary["first_divergences"]}


def verified_export(archive, name, **options):
    """Export only from an integrity-verified archive; the exporter itself does not check."""
    audit = capture_quality.audit_capture(archive)
    if audit["status"] != "verified":
        raise ValueError(f"archive not verified ({audit['status']}): {', '.join(audit['issues'])}")
    fixture = prepare.export(archive, EXPORT["connection"], EXPORT["start_seconds"],
                             EXPORT["duration_seconds"], name, **options)
    fixture["provenance"]["source"] = ARCHIVE  # the exporter records the absolute path
    return fixture


def tamper_controls(archive, scratch):
    """Audit a frame-tampered and a truncated copy; both must be quarantined."""
    data = gzip.decompress(archive.read_bytes())
    lines = data.splitlines(keepends=True)
    # Flip one bit of a mid-capture incoming frame; the record stays well formed.
    frames = [i for i, line in enumerate(lines) if line.startswith(b'["i"')]
    index = frames[len(frames) // 2]
    record = json.loads(lines[index])
    payload = bytearray(base64.b64decode(record[2]))
    payload[-1] ^= 1
    record[2] = base64.b64encode(bytes(payload)).decode()
    lines[index] = (json.dumps(record, separators=(",", ":")) + "\n").encode()
    tampered = scratch / "tampered.jsonl.gz"
    with tampered.open("xb") as raw, gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as gz:
        gz.write(b"".join(lines))
    truncated = scratch / "truncated.jsonl.gz"
    raw = archive.read_bytes()
    truncated.write_bytes(raw[: len(raw) // 2])
    results = {}
    for label, path in (("tamperedFrame", tampered), ("truncatedGzip", truncated)):
        audit = capture_quality.audit_capture(path)
        try:
            prepare.export(path, EXPORT["connection"], EXPORT["start_seconds"],
                           EXPORT["duration_seconds"], label)
            exporter = "accepted"
        except (ValueError, EOFError, OSError) as error:
            exporter = f"rejected:{type(error).__name__}"
        results[label] = {"auditStatus": audit["status"], "auditIssues": audit["issues"],
                          "exporterWithoutAuditGate": exporter}
    return results


def check(checks, name, passed, observed, criterion):
    checks.append({"name": name, "passed": bool(passed), "observed": observed, "criterion": criterion})


def load_plan(path):
    data = canonical(DEFAULT_PLAN).encode() if path is None else Path(path).read_bytes()
    plan = json.loads(data)
    if plan.get("schemaVersion") != 1 or plan.get("kind") != "pipeline-test" or plan.get("dataOrigin") != DATA_ORIGIN:
        raise ValueError("plan must be a schemaVersion 1 pipeline-test with dataOrigin local-simulator")
    if not isinstance(plan.get("studyId"), str) or plan.get("sampling", {}).get("attempts", 0) < 1:
        raise ValueError("plan needs a studyId and at least one planned attempt")
    return data, plan


def append_event(study, event):
    with (study / "attempts.jsonl").open("a", encoding="utf-8", newline="\n") as stream:
        stream.write(json.dumps(event, separators=(",", ":"), allow_nan=False) + "\n")


def run(output, plan_path=None, command=None):
    output = Path(output)
    plan_bytes, plan = load_plan(plan_path)
    if output.exists():
        raise FileExistsError(f"output exists; choose a fresh directory: {output}")
    slither_sim, fidelity_replay = build_executables()
    output.mkdir(parents=True)
    for folder in ("raw", "analysis/replay", "fixtures"):
        (output / folder).mkdir(parents=True)
    (output / "plan.json").write_bytes(plan_bytes)
    plan_sha = sha256(plan_bytes)
    (output / "plan.sha256").write_text(plan_sha + "\n", encoding="ascii", newline="\n")
    (output / "attempts.jsonl").touch()
    collector_sha = file_sha256(__file__)
    sources = source_digests()
    base = {"schemaVersion": 1, "attemptId": "attempt-1", "studyId": plan["studyId"], "planSha256": plan_sha}
    append_event(output, {**base, "event": "started", "time": utc_now(),
                          "command": command or ["python", "tools/fidelity/worked_study.py", "--output", str(output)]})
    archive = output / ARCHIVE
    try:
        states, commands, first_command = simulate(slither_sim)
        identity = f"slither-sim-worked-study:{plan_sha}:{collector_sha}:{sources['combinedSha256']}"
        provenance = {
            "version": 1, "captureId": str(uuid.uuid5(uuid.NAMESPACE_URL, identity + ":capture")),
            "captureBatchId": str(uuid.uuid5(uuid.NAMESPACE_URL, identity + ":batch")),
            "collectorSha256": collector_sha, "planSha256": plan_sha, "timeOrigin": LOCAL_ORIGIN_MS,
            "botIndex": 0, "botCount": 1, "dataOrigin": DATA_ORIGIN, "synthetic": True,
            "options": {"bots": 1, "scenario": "movement", "seconds": TICKS * TICK_MS / 1000,
                        "generator": "tools/fidelity/worked_study.py", "parameters": PARAMETERS},
            "sources": sources,
        }
        records, true_anchors = build_records(states, commands, first_command, provenance)
        write_archive(archive, records, file_sha256(__file__))
    except BaseException as error:
        append_event(output, {**base, "event": "finished", "time": utc_now(), "status": "failed",
                              "exitCode": 1, "captures": [ARCHIVE] if archive.exists() else [],
                              "error": f"{type(error).__name__}: {error}"})
        raise
    append_event(output, {**base, "event": "finished", "time": utc_now(), "status": "completed",
                          "exitCode": 0, "captures": [ARCHIVE]})
    results = analyse(output, archive, states, commands, true_anchors, fidelity_replay, plan_sha, sources)
    (output / "analysis/results.json").write_text(canonical(results), encoding="utf-8", newline="\n")
    (output / "REPORT.md").write_text(report(plan, results), encoding="utf-8", newline="\n")
    return results


def analyse(output, archive, states, commands, true_anchors, fidelity_replay, plan_sha, sources):
    checks = []
    audit = capture_quality.audit_capture(archive)
    audit["path"] = ARCHIVE
    check(checks, "archive_integrity_verified",
          audit["status"] == "verified" and not audit["issues"] and audit["integrityVerified"]
          and audit["completeGzip"] and audit["provenance"]["planSha256"] == plan_sha,
          {"status": audit["status"], "issues": audit["issues"], "warnings": audit["warnings"]},
          "capture_quality status verified, no issues, terminal receipt and gzip complete, plan pinned")
    with tempfile.TemporaryDirectory() as temp:
        tamper = tamper_controls(archive, Path(temp))
    check(checks, "tampered_inputs_quarantined",
          all(row["auditStatus"] == "quarantine" for row in tamper.values()), tamper,
          "frame-tampered and truncated copies are quarantined by the audit")
    results = {"schemaVersion": 1, "dataOrigin": DATA_ORIGIN, "synthetic": True, "label": LABEL,
               "claimScope": CLAIM, "planSha256": plan_sha,
               "archive": {"path": ARCHIVE, "sha256": file_sha256(archive)},
               "sources": sources, "git": git_identity(), "generator": PARAMETERS,
               "trace": {"ticks": len(states) - 1, "commands": len(commands),
                         "anchors": len(true_anchors), "segments": states[-1]["segments"]},
               "audit": audit, "tamperControls": tamper, "checks": checks}
    if audit["status"] != "verified":
        results["outcome"] = "failed"
        results["stoppedBecause"] = "archive not verified; export refused"
        return results

    primary = verified_export(archive, "synthetic-movement-anchors3")
    variants = {"anchors3": primary,
                "anchors4": verified_export(archive, "synthetic-movement-anchors4", heading_history=4),
                "reported": verified_export(archive, "synthetic-movement-reported", heading_source="reported")}
    provenance = primary["provenance"]
    start = provenance["start_server_ms"]
    true = states[start // TICK_MS]
    # Exact mapping: every exported control should land on its true effective boundary.
    in_window = [c for c in commands if start < c["effectiveServerMs"] <= start + EXPORT["duration_seconds"] * 1000]
    control_errors = [abs(c["time"] * 1000 - (t["effectiveServerMs"] - start))
                      for c, t in zip(primary["controls"], in_window)]
    heading_errors = [abs(wrap(c["action"]["heading"] - command_heading(t["byte"])))
                      for c, t in zip(primary["controls"], in_window)]
    check(checks, "clock_and_control_mapping_exact",
          abs(provenance["clock_slope"] - 1) < 1e-9 and provenance["clock_residual_p95_ms"] < WIRE_TOLERANCE
          and len(primary["controls"]) == len(in_window) and max(control_errors) < WIRE_TOLERANCE
          and max(heading_errors) < 1e-9,
          {"slope": provenance["clock_slope"], "interceptMs": provenance["clock_intercept_ms"],
           "residualP95Ms": provenance["clock_residual_p95_ms"], "rttMedianMs": provenance["ping_rtt_median_ms"],
           "pingSamples": provenance["ping_samples"], "controlsExported": len(primary["controls"]),
           "controlsTrue": len(in_window), "maxControlTimeErrorMs": max(control_errors),
           "maxControlHeadingError": max(heading_errors)},
          f"slope within 1e-9 of 1; residual and control time errors below {WIRE_TOLERANCE} ms")
    origin = true["body"][0]
    anchor_by_ms = {row["serverMs"]: row["anchor"] for row in true_anchors}
    decode_errors = []
    for row in primary["observations"]:
        if "anchor" in row:
            ms = start + round(row["time"] * 1000)
            truth = anchor_by_ms[ms]
            decode_errors.append(math.hypot(row["anchor"]["x"] - (truth["x"] - origin["x"]),
                                            row["anchor"]["y"] - (truth["y"] - origin["y"])))
    expected = sum(start < ms <= start + EXPORT["duration_seconds"] * 1000 for ms in anchor_by_ms)
    check(checks, "decoded_anchors_match_truth",
          len(decode_errors) == expected and max(decode_errors) < WIRE_TOLERANCE,
          {"anchors": len(decode_errors), "expected": expected, "maxError": max(decode_errors)},
          f"every true anchor in the window decoded within {WIRE_TOLERANCE} units")

    shift = lambda p: {"x": p["x"] - origin["x"], "y": p["y"] - origin["y"]}
    known = json.loads(json.dumps(primary))
    known["name"] = "CONTROL-known-state"
    known["initial"]["snake"] = {**true, "id": "recorded-self", "head": shift(true["head"]),
                                 "body": [shift(p) for p in true["body"]]}
    known["provenance"]["control"] = (
        "Known-state control: initial snake replaced by the true simulator state at the window "
        "start. Unavailable from any real capture.")
    controls = {"known-state": known}
    for label, offset in (("timing-plus7ms", 7.0), ("timing-minus7ms", -7.0)):
        fixture = json.loads(json.dumps(known))
        fixture["name"] = f"NEGATIVE-CONTROL-{label}"
        fixture["options"]["input_time_offset_ms"] = offset
        fixture["provenance"]["control"] = f"Negative control: known state, controls shifted {offset:+} ms."
        controls[label] = fixture
    fixture = json.loads(json.dumps(known))
    fixture["name"] = "NEGATIVE-CONTROL-heading-plus50mrad"
    fixture["initial"]["snake"]["heading"] += 0.05
    fixture["provenance"]["control"] = "Negative control: known state, initial heading +0.05 rad."
    controls["heading-plus50mrad"] = fixture

    replays = {}
    files = {"anchors3": "replay.json", "anchors4": "replay-anchors4.json",
             "reported": "replay-reported.json", "known-state": "control-known-state.json",
             "timing-plus7ms": "control-timing-plus7ms.json",
             "timing-minus7ms": "control-timing-minus7ms.json",
             "heading-plus50mrad": "control-heading-plus50mrad.json"}
    for label, fixture in {**variants, **controls}.items():
        summary = replay(fidelity_replay, fixture, output / "fixtures" / files[label],
                         output / "analysis/replay" / f"{label}.jsonl")
        summary["fixture"] = f"fixtures/{files[label]}"
        summary["initialHeading"] = fixture["initial"]["snake"]["heading"]
        summary["initialHeadingError"] = abs(wrap(fixture["initial"]["snake"]["heading"] - true["heading"]))
        summary["initialSpeedError"] = fixture["initial"]["snake"]["speed"] - true["speed"]
        replays[label] = summary
    baseline = replays["known-state"]["anchorMeanError"]
    check(checks, "known_state_replay_reproduces_anchors",
          replays["known-state"]["anchorEventsObserved"] == expected
          and replays["known-state"]["anchorMaxError"] < KNOWN_STATE_ANCHOR_TOLERANCE,
          {"meanError": baseline, "maxError": replays["known-state"]["anchorMaxError"]},
          f"max anchor error below {KNOWN_STATE_ANCHOR_TOLERANCE} units")
    for label in ("timing-plus7ms", "timing-minus7ms", "heading-plus50mrad"):
        error = replays[label]["anchorMeanError"]
        check(checks, f"negative_control_detected:{label}",
              error > NEGATIVE_CONTROL_MIN_ERROR and error > NEGATIVE_CONTROL_MIN_RATIO * max(baseline, 1e-12),
              {"meanError": error, "knownStateMeanError": baseline},
              f"mean anchor error above {NEGATIVE_CONTROL_MIN_ERROR} units and "
              f"{NEGATIVE_CONTROL_MIN_RATIO:g} x known-state")
    results["export"] = {key: provenance[key] for key in (
        "start_server_ms", "start_after_init_seconds", "clock_slope", "clock_intercept_ms",
        "clock_residual_p95_ms", "clock_fit_frames", "ping_rtt_median_ms", "ping_samples")}
    results["export"]["limitations"] = provenance["limitations"]
    results["initialState"] = {
        "trueHeading": true["heading"], "trueSpeed": true["speed"], "estimates": {
            label: {"method": f["provenance"]["initial_heading_method"],
                    "heading": f["initial"]["snake"]["heading"],
                    "absoluteError": abs(wrap(f["initial"]["snake"]["heading"] - true["heading"])),
                    "speedError": f["initial"]["snake"]["speed"] - true["speed"]}
            for label, f in variants.items()},
        "note": ("Exported fixtures reconstruct the initial heading causally from wire data; the "
                 "errors above are the reconstruction uncertainty this pipeline carries into replay. "
                 "Only the separately labelled known-state control removes it."),
    }
    results["replays"] = replays
    results["outcome"] = "pipeline-checks-passed" if all(c["passed"] for c in checks) else "failed"
    return results


def report(plan, results):
    lines = [f"# {plan['title']}", "",
             f"> **{LABEL.upper()}.** {CLAIM}", "",
             f"Data origin: {DATA_ORIGIN}. Scientific status: unreviewed. Outcome: **{results['outcome']}**.",
             "", "## Checks", "", "| Check | Result | Criterion |", "| --- | --- | --- |"]
    lines += [f"| `{c['name']}` | {'pass' if c['passed'] else 'FAIL'} | {c['criterion']} |" for c in results["checks"]]
    if "replays" in results:
        initial = results["initialState"]
        lines += ["", "## Initial-state reconstruction", "",
                  f"True heading at the window start: {initial['trueHeading']:.6f} rad; true speed "
                  f"{initial['trueSpeed']:.4f} units/s.", "",
                  "| Exported fixture | Method | Heading error (rad) | Speed error (units/s) |",
                  "| --- | --- | ---: | ---: |"]
        lines += [f"| {label} | {row['method']} | {row['absoluteError']:.6f} | {row['speedError']:+.4f} |"
                  for label, row in initial["estimates"].items()]
        lines += ["", initial["note"], "", "## Replays (free mode)", "",
                  "| Replay | Fixture | Anchors | Mean anchor error | Max anchor error |",
                  "| --- | --- | ---: | ---: | ---: |"]
        for label, row in results["replays"].items():
            kind = "control" if label in ("known-state",) else "negative control" if label.startswith(("timing", "heading")) else "exported"
            lines.append(f"| {label} ({kind}) | `{row['fixture']}` | {row['anchorEventsObserved']} | "
                         f"{row['anchorMeanError']:.6g} | {row['anchorMaxError']:.6g} |")
        lines += ["", "Exported-fixture replay errors include the initial heading and wire speed "
                  "reconstruction above; they are reported, not pass/fail criteria."]
    tamper = results["tamperControls"]
    lines += ["", "## Tamper controls", "",
              "| Copy | Audit | Issues | Exporter without the audit gate |", "| --- | --- | --- | --- |"]
    lines += [f"| {label} | {row['auditStatus']} | {', '.join(row['auditIssues'])} | {row['exporterWithoutAuditGate']} |"
              for label, row in tamper.items()]
    lines += ["", "The exporter does not verify integrity, so this study exports only after the audit "
              "returns verified."]
    lines += ["", "## Artifacts", "",
              f"- `plan.json` (SHA-256 `{results['planSha256']}`, also in `plan.sha256` and the capture header)",
              f"- `{ARCHIVE}` (SHA-256 `{results['archive']['sha256']}`): deterministic schema-1 archive",
              "- `attempts.jsonl`: attempt ledger", "- `analysis/results.json`: audit, checks and replay summaries",
              "- `analysis/replay/*.jsonl`: raw replay output", "- `fixtures/replay.json`: exporter default fixture; "
              "other `fixtures/*.json` are labelled variants and controls", "",
              "## Reproduce", "", "```sh", "python tools/fidelity/worked_study.py --output <fresh-dir>", "```", "",
              "## Reuse", "", "Copyright (c) 2026 Nikita M. Grimm. Synthetic data, generated fixtures "
              "and this report are available under the MIT license in the project LICENSE file "
              "(software/LICENSE in a packaged bundle).", "", "## Limitations", ""]
    lines += [f"- {item}" for item in plan["limitations"]]
    lines += [f"- Exporter: {item}" for item in results.get("export", {}).get("limitations", [])]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path, required=True, help="fresh directory to create")
    parser.add_argument("--plan", type=Path, help="pipeline-test plan to freeze byte-for-byte (default: built-in)")
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output exists; choose a fresh directory")
    results = run(args.output, args.plan, command=[Path(sys.executable).name, *sys.argv])
    print(json.dumps({"outcome": results["outcome"], "output": str(args.output),
                      "checks": {c["name"]: c["passed"] for c in results["checks"]}}, indent=2))
    return 0 if results["outcome"] == "pipeline-checks-passed" else 1


if __name__ == "__main__":
    sys.exit(main())
