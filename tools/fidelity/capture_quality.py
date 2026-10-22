import base64
import binascii
import gzip
import hashlib
import json
import math
from pathlib import Path
import uuid
import zlib
from slither_protocol import PROTOCOL_VERSION, split as packets, split_payload


class DigestReader:
    def __init__(self, stream):
        self.stream = stream
        self.digest = hashlib.sha256()

    def read(self, size=-1):
        data = self.stream.read(size)
        self.digest.update(data)
        return data


def finite_number(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return False
    try:
        return math.isfinite(value)
    except OverflowError:
        return False


def reject_constant(value):
    raise ValueError(f"non-finite JSON constant: {value}")


def finite_json_float(value):
    number = float(value)
    if not math.isfinite(number):
        raise ValueError("non-finite JSON number")
    return number


def sha256_string(value):
    return isinstance(value, str) and len(value) == 64 and all(char in "0123456789abcdef" for char in value)


def setup_signature(packet):
    if len(packet) < 25:
        raise ValueError("short setup packet")
    return {
        "protocolVersion": packet[23],
        "grd": int.from_bytes(packet[1:4], "big"),
        "mscps": int.from_bytes(packet[4:6], "big"),
        "sectorSize": int.from_bytes(packet[6:8], "big"),
        "cstWire": int.from_bytes(packet[21:23], "big"),
        "msl": packet[24],
        "remainingStaticSetupHex": packet[8:21].hex(),
    }


class Distribution:
    def __init__(self):
        self.count = 0
        self.total = 0.0
        self.minimum = None
        self.maximum = None
        self.edges = [1, 5, 10, 25, 50, 100, 250, 500, 1000, 5000]
        self.histogram = [0] * (len(self.edges) + 1)

    def add(self, value):
        self.count += 1
        self.total += value
        self.minimum = value if self.minimum is None else min(self.minimum, value)
        self.maximum = value if self.maximum is None else max(self.maximum, value)
        self.histogram[sum(value > edge for edge in self.edges)] += 1

    def summary(self):
        return {"count": self.count, "mean": self.total / self.count if self.count else None,
                "min": self.minimum, "max": self.maximum,
                "binUpperBounds": self.edges + [None], "binCounts": self.histogram}


def audit_capture(file):
    source = Path(file)
    issues, warnings = [], []
    header, terminal = None, None
    payload_digest = hashlib.sha256()
    payload_bytes, lines = 0, 0
    counts = {"i": {"frames": 0, "bytes": 0}, "o": {"frames": 0, "bytes": 0}}
    previous_time, previous_arrival = None, None
    connections, setups, joins = 0, [], 0
    raw_framing, connection_frames, initialized = False, 0, False
    active_server = None
    observed_servers = []
    arrival, elapsed, mismatch, rtt = [Distribution() for _ in range(4)]
    zero_elapsed, outstanding, ping_requests, ping_censored, unsolicited = 0, None, 0, 0, 0
    ping_ambiguous, ambiguous_ping_requests = False, 0
    server_elapsed_total = 0
    integrity_verified = False
    complete_payload = False
    with source.open("rb") as raw:
        reader = DigestReader(raw)
        try:
            with gzip.GzipFile(fileobj=reader, mode="rb") as archive:
                while True:
                    line = archive.readline(1048577)
                    if not line:
                        complete_payload = True
                        break
                    if len(line) > 1048576:
                        raise ValueError("record exceeds 1 MiB")
                    if not line.endswith(b"\n"):
                        raise ValueError("unterminated JSONL record")
                    record = json.loads(line, parse_constant=reject_constant, parse_float=finite_json_float)
                    if terminal is not None:
                        raise ValueError("records after terminal receipt")
                    if isinstance(record, dict):
                        timestamp = record.get("t")
                    elif isinstance(record, list) and len(record) == 3:
                        timestamp = record[1]
                    else:
                        raise ValueError("invalid record shape")
                    if not finite_number(timestamp) or not 0 <= timestamp <= 9007199254740991:
                        raise ValueError("invalid monotonic timestamp")
                    if previous_time is not None and timestamp < previous_time:
                        raise ValueError("record clock regressed")
                    if isinstance(record, dict):
                        if record.get("type") == "header":
                            if lines or header is not None:
                                raise ValueError("header must be the first record")
                            header = record
                            if type(record.get("schema")) is not int or record.get("schema") != 1:
                                raise ValueError("unsupported capture schema")
                            active_server = record.get("server")
                        elif record.get("type") != "meta":
                            raise ValueError("unknown metadata type")
                        event = record.get("event")
                        timestamp = record.get("t")
                        if event == "connect":
                            if outstanding is not None:
                                ping_censored += 1
                            outstanding = None
                            ping_ambiguous = False
                            connections += 1
                            raw_framing, initialized, connection_frames = False, False, 0
                            previous_arrival = None
                            active_server = record.get("server", active_server)
                            if active_server not in observed_servers:
                                observed_servers.append(active_server)
                        if event == "close":
                            if outstanding is not None:
                                ping_censored += 1
                            outstanding = None
                            ping_ambiguous = False
                            initialized = False
                            previous_arrival = None
                        if event == "unframed-challenge":
                            raw_framing = True
                        if event in ("capture-end", "capture-interrupted"):
                            terminal = record
                            integrity = record.get("integrity")
                            if integrity is not None:
                                expected = {"format": "sha256-jsonl-prefix-v1",
                                            "sha256": payload_digest.hexdigest(), "lines": lines,
                                            "payloadBytes": payload_bytes, "counts": counts}
                                if not isinstance(integrity, dict) or any(
                                        integrity.get(key) != value for key, value in expected.items()):
                                    issues.append("terminal_integrity_mismatch")
                                else:
                                    integrity_verified = True
                            if record.get("frames") != counts["i"]["frames"]:
                                issues.append("incoming_frame_count_mismatch")
                            if record.get("joins") != joins:
                                issues.append("join_count_mismatch")
                    elif isinstance(record, list) and len(record) == 3:
                        direction, timestamp, encoded = record
                        if direction not in counts or not isinstance(encoded, str):
                            raise ValueError("invalid frame record")
                        data = base64.b64decode(encoded, validate=True)
                        if not data:
                            raise ValueError("empty frame")
                        counts[direction]["frames"] += 1
                        counts[direction]["bytes"] += len(data)
                        if direction == "o" and data == b"\xfb":
                            ping_requests += 1
                            if ping_ambiguous:
                                ambiguous_ping_requests += 1
                            elif outstanding is not None:
                                warnings.append("overlapping_game_pings")
                                ping_ambiguous = True
                                ambiguous_ping_requests += 2
                                outstanding = None
                            else:
                                outstanding = timestamp
                        if direction == "i":
                            connection_frames += 1
                            if connection_frames == 1 and data[0] == 54:
                                raw_framing = True
                            decoded = split_payload(data) if raw_framing else packets(data)
                            setup_packets = [packet for packet in decoded if packet[:1] == b"a"]
                            for packet in setup_packets:
                                signature = setup_signature(packet)
                                joins += 1
                                if signature not in setups:
                                    setups.append(signature)
                            if initialized and not setup_packets and not raw_framing:
                                header_ms = int.from_bytes(data[:2], "big")
                                gap = timestamp - previous_arrival
                                arrival.add(gap)
                                elapsed.add(header_ms)
                                mismatch.add(abs(gap - header_ms))
                                zero_elapsed += header_ms == 0
                                server_elapsed_total += header_ms
                            if setup_packets:
                                initialized = True
                            if initialized:
                                previous_arrival = timestamp
                            for packet in decoded:
                                if packet[:1] == b"p":
                                    if outstanding is None or ping_ambiguous:
                                        unsolicited += 1
                                    else:
                                        rtt.add(timestamp - outstanding)
                                        outstanding = None
                    else:
                        raise ValueError("invalid record shape")
                    if header is None:
                        raise ValueError("missing header")
                    previous_time = timestamp
                    payload_digest.update(line)
                    payload_bytes += len(line)
                    lines += 1
        except (ValueError, TypeError, KeyError, EOFError, OSError, binascii.Error, zlib.error, RecursionError) as error:
            issues.append(f"invalid_archive:{error}")
        while reader.read(1048576):
            pass
        archive_sha256 = reader.digest.hexdigest()
    if outstanding is not None:
        ping_censored += 1
    if terminal is None:
        issues.append("missing_terminal_receipt")
    elif terminal.get("event") != "capture-end":
        issues.append("interrupted_capture")
    if not counts["i"]["frames"]:
        issues.append("no_incoming_frames")
    if not setups:
        issues.append("no_gameplay_setup")
    for signature in setups:
        if signature["protocolVersion"] != PROTOCOL_VERSION:
            issues.append(f"unsupported_protocol_version:{signature['protocolVersion']}")
    if len(setups) > 1:
        issues.append("setup_signature_changed_within_capture")
    if len(observed_servers) > 1:
        issues.append("server_changed_within_capture")
    if header:
        server = header.get("server")
        if not isinstance(server, dict) or not isinstance(server.get("ip"), str) or not server.get("ip") or type(server.get("port")) is not int or not 1 <= server.get("port") <= 65535:
            issues.append("invalid_server_identity")
        if not finite_number(header.get("wall")):
            issues.append("invalid_wall_clock")
        if not connections:
            issues.append("missing_connection_identity")
    provenance = (header or {}).get("captureProvenance")
    provenance_valid = isinstance(provenance, dict)
    if provenance_valid:
        try:
            if provenance.get("version") != 1:
                raise ValueError("unsupported provenance version")
            for key in ("captureId", "captureBatchId"):
                if not isinstance(provenance[key], str):
                    raise ValueError(f"invalid {key}")
                uuid.UUID(provenance[key])
            for key in ("collectorSha256",):
                value = provenance[key]
                if not sha256_string(value):
                    raise ValueError(f"invalid {key}")
            if provenance.get("planSha256") is not None and not sha256_string(provenance.get("planSha256")):
                raise ValueError("invalid plan digest")
            if not finite_number(provenance["timeOrigin"]):
                raise ValueError("invalid time origin")
            if not isinstance(provenance["botIndex"], int) or isinstance(provenance["botIndex"], bool):
                raise ValueError("invalid bot index")
            if not isinstance(provenance["botCount"], int) or isinstance(provenance["botCount"], bool):
                raise ValueError("invalid bot count")
            if not 0 <= provenance["botIndex"] < provenance["botCount"] <= 3:
                raise ValueError("invalid bot membership")
            if not isinstance(provenance["options"], dict):
                raise ValueError("missing acquisition options")
            if provenance["options"].get("bots") != provenance["botCount"]:
                raise ValueError("inconsistent acquisition bot count")
        except (KeyError, TypeError, ValueError) as error:
            provenance_valid = False
            issues.append(f"invalid_provenance:{error}")
    if provenance_valid and not integrity_verified:
        issues.append("provenance_without_valid_integrity")
    if not provenance_valid:
        warnings.append("legacy_or_unknown_provenance")
    if raw_framing:
        warnings.append("raw_community_framing_no_elapsed_clock")
    if connections > 1:
        warnings.append("reconnected_split_lives_not_continuous_play")
    if terminal and provenance_valid and terminal.get("collectorEndSha256") != provenance.get("collectorSha256"):
        warnings.append("collector_source_changed_on_disk")
    status = "quarantine" if issues else "verified" if provenance_valid and integrity_verified else "legacy_unknown"
    wall_start = (header or {}).get("wall")
    monotonic_start = (header or {}).get("t")
    wall_end = wall_start + previous_time - monotonic_start if all(
        finite_number(value) for value in (wall_start, previous_time, monotonic_start)) else None
    receipt_integrity = (terminal or {}).get("integrity")
    if not isinstance(receipt_integrity, dict):
        receipt_integrity = {}
    return {"path": str(source), "archiveSha256": archive_sha256,
            "payloadSha256": payload_digest.hexdigest() if complete_payload else None,
            "status": status, "issues": sorted(set(issues)), "warnings": sorted(set(warnings)),
            "integrityVerified": integrity_verified, "completeGzip": complete_payload,
            "header": header, "provenance": provenance, "provenanceValid": provenance_valid,
            "connections": connections, "joins": joins, "counts": counts,
            "setupSignatures": setups, "observedServers": observed_servers,
            "wallStartMs": wall_start, "wallEndMs": wall_end,
            "durationMs": previous_time - monotonic_start if finite_number(previous_time) and finite_number(monotonic_start) else None,
            "network": {"arrivalGapMs": arrival.summary(), "elapsedHeaderMs": elapsed.summary(),
                        "arrivalHeaderAbsoluteDifferenceMs": mismatch.summary(), "zeroElapsedHeaders": zero_elapsed,
                        "receivedServerElapsedMs": server_elapsed_total, "gamePingRttMs": rtt.summary(),
                        "pingRequests": ping_requests, "censoredPings": ping_censored,
                        "ambiguousPingRequests": ambiguous_ping_requests, "unmatchedPongs": unsolicited},
            "collectorTiming": (terminal or {}).get("collectorTiming"),
            "recorderQueue": {key: receipt_integrity.get(key)
                              for key in ("backpressureWrites", "maxQueuedBytes")},
            "limits": "Integrity is not server ground truth. RTT includes server/client scheduling; no one-way latency, packet-loss rate, hidden tick or global ecology inferred."}


def audit_captures(files):
    captures = [audit_capture(file) for file in files]
    for key in ("archiveSha256", "payloadSha256"):
        groups = {}
        for capture in captures:
            value = capture[key]
            if value is not None:
                groups.setdefault(value, []).append(capture)
        for group in groups.values():
            if len(group) > 1:
                for capture in group:
                    capture["status"] = "quarantine"
                    capture["issues"] = sorted(set(capture["issues"] + [f"duplicate_{key}"]))
    identities = {}
    for capture in captures:
        provenance = capture.get("provenance")
        if capture["provenanceValid"]:
            identities.setdefault(provenance["captureId"], []).append(capture)
    for group in identities.values():
        if len(group) > 1:
            for capture in group:
                capture["status"] = "quarantine"
                capture["issues"] = sorted(set(capture["issues"] + ["duplicate_capture_id"]))
    batches = {}
    for capture in captures:
        provenance = capture.get("provenance")
        batch_id = provenance.get("captureBatchId") if capture["provenanceValid"] else None
        if batch_id:
            batches.setdefault(batch_id, []).append(capture)
    batch_reports = []
    for batch_id, group in batches.items():
        members = [capture["provenance"] for capture in group]
        expected = members[0].get("botCount")
        complete = all(member.get("botCount") == expected for member in members) and len(group) == expected and sorted(member.get("botIndex", -1) for member in members) == list(range(expected or 0))
        matched = all(member.get("collectorSha256") == members[0].get("collectorSha256") and
                      member.get("timeOrigin") == members[0].get("timeOrigin") and
                      member.get("planSha256") == members[0].get("planSha256") and
                      member.get("options") == members[0].get("options") and
                      capture["header"].get("server") == group[0]["header"].get("server") and
                      capture["observedServers"] == group[0]["observedServers"] == [group[0]["header"].get("server")] and
                      capture["setupSignatures"] == group[0]["setupSignatures"] for member, capture in zip(members, group))
        shared_interval = all(finite_number(capture["durationMs"]) for capture in group) and max(
            capture["header"]["t"] for capture in group) < min(capture["header"]["t"] + capture["durationMs"] for capture in group)
        batch_reports.append({"captureBatchId": batch_id, "members": [capture["archiveSha256"] for capture in group],
                              "completeMembership": complete, "matchedProvenance": matched,
                              "sharedAcquisitionInterval": shared_interval,
                              "eligibleForPlannedInference": complete and matched and shared_interval and bool(members[0].get("planSha256")) and
                              all(capture["status"] == "verified" and not set(capture["warnings"]) &
                                  {"raw_community_framing_no_elapsed_clock", "collector_source_changed_on_disk", "overlapping_game_pings"} for capture in group),
                              "statisticalUnit": "one acquisition batch, not one bot/frame/life"})
    return {"schema": 1, "analyzerSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "captures": captures, "batches": batch_reports,
            "exclusions": [{"path": capture["path"], "archiveSha256": capture["archiveSha256"],
                            "reasons": capture["issues"]} for capture in captures if capture["status"] == "quarantine"]}
