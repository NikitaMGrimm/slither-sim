"""Offline protocol-19 recipient packet counts, cadence and known-prey exposure.

Example: python packet_census.py recordings/capture.jsonl.gz --output census.json
"""
import argparse
import base64
from collections import Counter
import gzip
import hashlib
import json
import math
from pathlib import Path
import sys
import zlib

import slither_protocol as sp


MAX_LINE_BYTES = 2 * 1024 * 1024
MAX_SEGMENTS = 128
MAX_FAMILIES = 4096
MAX_GAP_VALUES = 1_000_000
MAX_ENTITY_EPOCHS = 65536
MAX_ENTITY_ROWS = 256
STATES = ("unknown_before_setup", "unknown_own", "active", "post_own_death",
          "unknown_after_own_removal")


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def finite(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def reject_constant(value):
    raise ValueError(f"non-finite JSON constant: {value}")


class Budget:
    def __init__(self):
        self.gap_values = 0
        self.families = 0
        self.entity_epochs = 0


class Gaps:
    """Weighted quantiles at capture precision, with bounded distinct values."""
    def __init__(self, budget):
        self.budget = budget
        self.values = Counter()
        self.n = 0

    def add(self, value):
        if value < 0 or not math.isfinite(value):
            raise ValueError("clock regressed or became non-finite")
        # Recorder timestamps have three decimal places in milliseconds.
        # Remove subtraction noise before it consumes the distinct-value budget.
        value = round(value, 3)
        if value not in self.values:
            if self.budget.gap_values >= MAX_GAP_VALUES:
                raise ValueError("distinct gap value limit exceeded")
            self.budget.gap_values += 1
        self.values[value] += 1
        self.n += 1

    def summary(self):
        if not self.n:
            return dict(n=0, min=None, median=None, p95=None, max=None)
        ordered = sorted(self.values.items())
        targets = [(self.n - 1) // 2, self.n // 2, math.ceil(.95 * self.n) - 1]
        selected, cumulative = {}, 0
        for value, count in ordered:
            for target in targets:
                if cumulative <= target < cumulative + count:
                    selected[target] = value
            cumulative += count
        return dict(n=self.n, min=ordered[0][0],
                    median=(selected[targets[0]] + selected[targets[1]]) / 2,
                    p95=selected[targets[2]], max=ordered[-1][0])


class Packets:
    def __init__(self, budget):
        self.frames = self.packets = self.bytes = 0
        self.states = Counter()
        self.server, self.receipt = Gaps(budget), Gaps(budget)
        self.previous_server = self.previous_receipt = None

    def add(self, packet, server, receipt, state):
        self.packets += 1
        self.bytes += len(packet)
        self.states[state] += 1
        if server is not None and self.previous_server is not None:
            self.server.add(server - self.previous_server)
        if self.previous_receipt is not None:
            self.receipt.add(receipt - self.previous_receipt)
        self.previous_server, self.previous_receipt = server, receipt

    def summary(self):
        return dict(frames=self.frames, packets=self.packets, bytes=self.bytes,
                    byOwnState={state: self.states[state] for state in STATES},
                    gapsMs=dict(server=self.server.summary(), receipt=self.receipt.summary()))


class Segment:
    """One connection/setup epoch; no cadence or prey episode crosses it."""
    def __init__(self, connection, setup_index, raw_framing, budget):
        self.connection, self.setup_index = connection, setup_index
        self.raw_framing, self.budget = raw_framing, budget
        self.setup_at = self.last_receipt = self.last_server = None
        self.frames = self.bytes = 0
        self.commands, self.families = {}, {}
        self.own_id, self.own_state = None, "unknown_before_setup"
        self.entities, self.episodes = {}, {}
        self.prey_counts = Counter()
        self.prey_server_gaps, self.prey_receipt_gaps = Gaps(budget), Gaps(budget)
        self.map_server_gaps, self.map_receipt_gaps = Gaps(budget), Gaps(budget)
        self.previous_map_server = self.previous_map_receipt = None

    def finish_episode(self, prey_id, reason, server, receipt):
        start_server, start_receipt, _, _ = self.episodes.pop(prey_id)
        entity = self.entities[prey_id]
        if start_server is not None and server is not None:
            entity["serverExposureMs"] += server - start_server
        entity["receiptExposureMs"] += receipt - start_receipt
        entity[reason] += 1

    def prey_packet(self, packet, server, receipt):
        command, length = packet[0], len(packet)
        if command not in (ord("y"), ord("j")):
            return
        if length < 3:
            raise ValueError("short prey packet")
        prey_id = sp.u16(packet, 1)
        if command == ord("y"):
            if length not in (3, 5, 20):
                raise ValueError(f"unsupported prey y length {length}")
            if length == 20:
                if self.own_state == "unknown_before_setup":
                    self.prey_counts["entriesBeforeSetup"] += 1
                    return
                if prey_id in self.episodes:
                    self.finish_episode(prey_id, "censoredReplacement", server, receipt)
                if prey_id not in self.entities:
                    if self.budget.entity_epochs >= MAX_ENTITY_EPOCHS:
                        raise ValueError("prey ID/setup epoch limit exceeded")
                    self.budget.entity_epochs += 1
                    self.entities[prey_id] = Counter()
                entity = self.entities[prey_id]
                entity["entries"] += 1
                self.prey_counts["recipientEntries"] += 1
                self.episodes[prey_id] = (server, receipt, None, None)
            else:
                reason = "genericRemovals" if length == 3 else "eatenRemovals"
                self.prey_counts[reason] += 1
                if prey_id in self.episodes:
                    self.finish_episode(prey_id, reason, server, receipt)
                else:
                    self.prey_counts["unknown" + reason[0].upper() + reason[1:]] += 1
        else:
            if length not in (7, 9, 10, 11, 12, 13, 14, 16):
                raise ValueError(f"unsupported prey j length {length}")
            if prey_id not in self.episodes:
                self.prey_counts["unknownUpdates"] += 1
                return
            start_server, start_receipt, prior_server, prior_receipt = self.episodes[prey_id]
            self.entities[prey_id]["knownUpdates"] += 1
            self.prey_counts["knownUpdates"] += 1
            if prior_server is not None and server is not None:
                self.prey_server_gaps.add(server - prior_server)
            if prior_receipt is not None:
                self.prey_receipt_gaps.add(receipt - prior_receipt)
            self.episodes[prey_id] = (start_server, start_receipt, server, receipt)

    def packet(self, packet, server, receipt):
        command, length = packet[0], len(packet)
        if command in (ord("M"), ord("V")):
            if self.previous_map_server is not None and server is not None:
                self.map_server_gaps.add(server - self.previous_map_server)
            if self.previous_map_receipt is not None:
                self.map_receipt_gaps.add(receipt - self.previous_map_receipt)
            self.previous_map_server, self.previous_map_receipt = server, receipt
        family = (command, length)
        if family not in self.families:
            if self.budget.families >= MAX_FAMILIES:
                raise ValueError("command/length family limit exceeded")
            self.budget.families += 1
            self.families[family] = Packets(self.budget)
        if command not in self.commands:
            self.commands[command] = Packets(self.budget)
        self.commands[command].add(packet, server, receipt, self.own_state)
        self.families[family].add(packet, server, receipt, self.own_state)
        # Count the packet in the state BEFORE applying its transition. In a
        # bundle, packets after v (or a known own death) are post-own-death.
        if command == ord("a"):
            self.own_state = "unknown_own"
        elif command == ord("s"):
            if length < 3:
                raise ValueError("short snake packet")
            snake_id = sp.u16(packet, 1)
            if length > 7 and self.own_state != "unknown_before_setup":
                # Same first-full-appearance convention as World.snake_packet.
                if self.own_id is None:
                    self.own_id = snake_id
                if snake_id == self.own_id and self.own_state != "post_own_death":
                    self.own_state = "active"
            elif snake_id == self.own_id:
                if length > 3 and packet[3] == 1:
                    self.own_state = "post_own_death"
                elif self.own_state != "post_own_death":
                    self.own_state = "unknown_after_own_removal"
        elif command == ord("v"):
            self.own_state = "post_own_death"
        self.prey_packet(packet, server, receipt)

    def finish(self, reason):
        for prey_id in list(self.episodes):
            self.finish_episode(prey_id, "censored" + reason, self.last_server, self.last_receipt)

    def summary(self, entity_rows_left):
        rows, totals = [], Counter()
        for prey_id, entity in sorted(self.entities.items()):
            totals.update(entity)
            if len(rows) < entity_rows_left:
                rows.append(dict(preyId=prey_id, **exposure(entity, self.raw_framing)))
        def label(command):
            return chr(command) if 32 <= command < 127 else f"0x{command:02x}"
        duration = dict(server=None, receipt=None)
        if self.setup_at is not None:
            duration = dict(server=self.last_server,
                            receipt=self.last_receipt - self.setup_at)
        return dict(connection=self.connection, setupIndex=self.setup_index,
                    wireVersion=19 if self.setup_at is not None else None,
                    framing="header_free" if self.raw_framing else "elapsed_header",
                    setupReceiptMs=self.setup_at, durationMs=duration,
                    incomingFrames=self.frames, incomingBytes=self.bytes,
                    ownId=self.own_id, finalOwnState=self.own_state,
                    commands=[dict(command=label(c), **v.summary()) for c, v in sorted(self.commands.items())],
                    families=[dict(command=label(c), length=n, **v.summary())
                              for (c, n), v in sorted(self.families.items())],
                    minimapUpdateGapsMs=dict(server=self.map_server_gaps.summary(),
                                             receipt=self.map_receipt_gaps.summary()),
                    prey=dict(counts=dict(self.prey_counts), knownEpisodes=exposure(totals, self.raw_framing),
                              updateGapsMs=dict(server=self.prey_server_gaps.summary(),
                                                receipt=self.prey_receipt_gaps.summary()),
                              entities=rows, omittedEntityRows=len(self.entities)-len(rows)))


def exposure(counts, header_free):
    server = None if header_free else counts["serverExposureMs"] / 1000
    receipt = counts["receiptExposureMs"] / 1000
    return dict(entries=counts["entries"], knownUpdates=counts["knownUpdates"],
                genericRemovals=counts["genericRemovals"], eatenRemovals=counts["eatenRemovals"],
                censoredEpisodes=sum(v for k, v in counts.items() if k.startswith("censored")),
                exposureSeconds=dict(server=server, receipt=receipt),
                updatesPerKnownPreySecond=dict(
                    server=counts["knownUpdates"] / server if server else None,
                    receipt=counts["knownUpdates"] / receipt if receipt else None))


def census(source):
    source = Path(source)
    archive_sha = sha256(source)
    budget, segments = Budget(), []
    connection = setup_index = frames = raw_bytes = 0
    current, server_clock, previous_receipt = None, None, None
    raw_framing, first_incoming, terminal, header = False, True, None, False

    def close(reason):
        nonlocal current
        if current is not None:
            current.finish(reason)
            if current.frames:
                segments.append(current)
            current = None

    def start():
        if len(segments) >= MAX_SEGMENTS:
            raise ValueError("connection/setup segment limit exceeded")
        return Segment(connection, setup_index, raw_framing, budget)

    with gzip.open(source, "rb") as archive:
        while True:
            line = archive.readline(MAX_LINE_BYTES + 1)
            if not line:
                break  # Full consumption verifies gzip CRC/footer.
            if len(line) > MAX_LINE_BYTES:
                raise ValueError("JSONL record size limit exceeded")
            if not line.endswith(b"\n"):
                raise ValueError("unterminated JSONL record")
            record = json.loads(line, parse_constant=reject_constant)
            if terminal is not None:
                raise ValueError("records after capture-end")
            timestamp = None
            if isinstance(record, dict):
                timestamp = record.get("t")
            elif isinstance(record, list) and len(record) == 3:
                timestamp = record[1]
            if not finite(timestamp) or (previous_receipt is not None and timestamp < previous_receipt):
                raise ValueError("invalid or regressing receipt clock")
            previous_receipt = timestamp
            if isinstance(record, dict):
                if record.get("type") == "header":
                    if (header or frames or connection or type(record.get("schema")) is not int
                            or record["schema"] != 1):
                        raise ValueError("invalid capture header/schema")
                    header = True
                elif not header or record.get("type") != "meta":
                    raise ValueError("missing header or invalid metadata")
                event = record.get("event")
                if event == "connect":
                    close("Reconnect")
                    connection += 1
                    setup_index = 0
                    raw_framing, first_incoming, server_clock = False, True, None
                elif event in ("close", "connection-failed"):
                    close("Close")
                    server_clock = None
                elif event == "unframed-challenge":
                    raw_framing = True
                elif event == "capture-interrupted":
                    raise ValueError("capture-interrupted: no completed census")
                elif event == "capture-end":
                    terminal = record
                continue
            if not header or not isinstance(record, list) or len(record) != 3:
                raise ValueError("missing header or invalid frame record")
            direction, receipt, encoded = record
            if direction not in ("i", "o") or not isinstance(encoded, str):
                raise ValueError("invalid frame record")
            data = base64.b64decode(encoded, validate=True)
            if not data:
                raise ValueError("empty frame")
            if direction == "o":
                continue
            if first_incoming and data[0] == ord("6"):
                raw_framing = True
            first_incoming = False
            packets = sp.split_payload(data) if raw_framing else sp.split(data)
            setups = [packet for packet in packets if packet[0] == ord("a")]
            if len(setups) > 1:
                raise ValueError("multiple setup packets in one frame")
            if setups:
                setup = setups[0]
                if len(setup) < 25 or setup[23] != 19:
                    raise ValueError("expected literal wire protocol 19 setup")
                close("Setup")
                setup_index += 1
                server_clock = None if raw_framing else 0
                current = start()
                current.setup_at = receipt
            elif current is None:
                current = start()
            elif current.setup_at is not None and not raw_framing:
                server_clock += int.from_bytes(data[:2], "big")
            current.frames += 1
            current.bytes += len(data)
            current.last_server, current.last_receipt = server_clock, receipt
            frame_commands, frame_families = set(), set()
            for packet in packets:
                current.packet(packet, server_clock, receipt)
                frame_commands.add(packet[0])
                frame_families.add((packet[0], len(packet)))
            for command in frame_commands:
                current.commands[command].frames += 1
            for family in frame_families:
                current.families[family].frames += 1
            frames += 1
            raw_bytes += len(data)
    if terminal is None:
        raise ValueError("missing capture-end: no completed census")
    if "frames" in terminal and terminal["frames"] != frames:
        raise ValueError("capture-end incoming frame count mismatch")
    close("CaptureEnd")
    if not any(segment.setup_at is not None for segment in segments):
        raise ValueError("missing literal wire protocol 19 setup")
    if sha256(source) != archive_sha:
        raise ValueError("source archive changed during census")
    reports, rows_left = [], MAX_ENTITY_ROWS
    for segment in segments:
        report = segment.summary(rows_left)
        rows_left -= len(report["prey"]["entities"])
        reports.append(report)
    return dict(schemaVersion=1, status="complete", wireVersion=19,
                archiveSha256=archive_sha, decoderSha256=sha256(Path(sp.__file__)),
                censusSha256=sha256(Path(__file__)), incomingFrames=frames,
                incomingPackets=sum(v.packets for s in segments for v in s.commands.values()),
                incomingBytes=raw_bytes, connections=reports,
                limits=dict(maxRecordBytes=MAX_LINE_BYTES, maxConnectionSetupSegments=MAX_SEGMENTS,
                            maxCommandLengthFamilies=MAX_FAMILIES, maxDistinctGapValues=MAX_GAP_VALUES,
                            maxPreyIdsPerSegment=65536, maxPreyIdEpochs=MAX_ENTITY_EPOCHS,
                            maxReportedEntityRows=MAX_ENTITY_ROWS,
                            onLimit="fail without publishing a completed census"),
                definitions=dict(
                    clocks=("Milliseconds. Server elapsed headers restart at each setup; header-free "
                            "server time is unavailable. Receipt time is capture-local, not server or wall time."),
                    duration=("Setup through last incoming frame in that epoch; "
                              "no extrapolation to close/end metadata."),
                    packetBytes=("Logical command bytes including command byte; excludes frame timing "
                                 "and bundle length prefixes. incomingBytes includes those prefixes."),
                    gaps=("Consecutive occurrences of the command or command/length family within one "
                          "connection/setup epoch; same-frame occurrences have zero gap. "
                          "Gaps rounded to 0.001 ms capture precision; integer server gaps unchanged. "
                          "Median exact over those values, p95 nearest rank."),
                    ownState=("State before each packet. First full s (length >7) after setup identifies "
                              "own as in World, never assumes ID 0. v is counted before its transition; "
                              "following bundled packets are post-own-death. Mid-life captures cannot "
                              "verify this identity convention."),
                    prey=("y20 is recipient entry, not world birth. y3/y5 end known episodes as "
                          "generic/eaten. Repeated y20 censors and restarts. j gaps use consecutive "
                          "known j within the same episode only. Missing packets imply no absence."),
                    exposure=("Each known prey episode contributes entry-to-explicit-removal or "
                              "last-incoming-frame censor time. Sum known-prey seconds is the denominator "
                              "for known j counts; zero exposure yields null rates. Includes active and "
                              "post-death receipt periods, with no rates over unknown episodes."),
                    entityRows=("At most 256 per-ID/connection/setup aggregate rows, ordered by epoch then "
                                "ID; omitted rows still contribute to that epoch's knownEpisodes totals. "
                                "Reused IDs start separate episodes, never joined gaps."),
                    validation=("Complete means full JSONL/framing parse, protocol-19 setup, capture-end "
                                "and intact gzip, with stable archive SHA. Footer integrity/provenance "
                                "validation and ping/network summaries belong to "
                                "capture_quality.audit_capture; they are not recomputed here."),
                    inference=("Descriptive recipient counts and exposure-conditioned rates do not "
                               "establish parity, world birth/death rates or independent samples.")))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="one complete protocol-19 .jsonl.gz capture")
    parser.add_argument("--output", type=Path, help="write JSON to a new file; existing files are refused")
    args = parser.parse_args(argv)
    try:
        if args.output is not None and args.output.exists():
            raise FileExistsError(f"output already exists: {args.output}")
        result = census(args.archive)
        serialized = json.dumps(result, indent=2, allow_nan=False)
        if args.output is None:
            print(serialized)
        else:
            with args.output.open("x", encoding="utf-8", newline="\n") as stream:
                stream.write(serialized + "\n")
    except (ValueError, TypeError, IndexError, KeyError, OSError, EOFError,
            OverflowError, RecursionError, zlib.error) as error:
        print(f"packet census failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
