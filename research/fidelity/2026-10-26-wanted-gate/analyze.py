"""Offline conditional own-motion gate analysis; serial, no network modules.

Arguments locate retained inputs; --output must name a new external directory.
Outputs contain neutral aliases, never paths,
endpoints, raw packet bytes or round-trip measurements. Clock bounds locate SEND,
not server ingress. The report must retain that distinction.
"""
import argparse
import bisect
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import sys
import publication


def load(p):
    return json.loads(p.read_text(encoding="utf-8"))


def sha(p):
    with p.open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def quantile(xs, q):
    return sorted(xs)[max(0, math.ceil(len(xs) * q) - 1)] if xs else None


def scan(path, sp, form):
    frames = defaultdict(list)
    pings = defaultdict(list)
    motions = defaultdict(list)
    commands = {}
    frame_flags = {}
    epoch = 0
    own = None
    pending_ping = None
    state = {}
    direction = None
    active = False
    for line, item in enumerate(sp.read(path), 1):
        if isinstance(item, dict):
            if item.get("event") == "connect":
                own = None
                pending_ping = None
                state = {}
                active = False
            continue
        if isinstance(item, sp.Out):
            if len(item.data) == 1:
                byte = item.data[0]
                if byte == 251:
                    pending_ping = item.t
                elif byte <= 250 or byte in (253, 254):
                    commands[line] = dict(t=item.t, byte=byte, priorWanted=state.get("wanted"),
                                          priorSpeed=state.get("speed"), active=active,
                                          connection=item.conn, epoch=epoch)
            continue
        if any(p[:1] == b"a" for p in item.packets):
            epoch += 1
            own = None
            state = {}
            direction = None
            active = True
        key = (item.conn, epoch)
        if not epoch:
            continue
        frames[key].append((item.t, item.server, line))
        flags = set()
        for pi, packet in enumerate(item.packets):
            code = chr(packet[0])
            if code == "p" and pending_ping is not None:
                pings[key].append((pending_ping, item.t, item.server))
                pending_ping = None
            if code == "s" and len(packet) > 7 and own is None:
                own = int.from_bytes(packet[1:3], "big")
                state = dict(heading=int.from_bytes(packet[3:6], "big") * 256 / 16777215,
                             wanted=int.from_bytes(packet[7:10], "big") * 256 / 16777215,
                             speed=int.from_bytes(packet[10:12], "big") / 1000)
                direction = packet[6] - 48
            if code == "s" and len(packet) <= 7 and int.from_bytes(packet[1:3], "big") == own:
                active = False
                state = {}
            if code == "v":
                active = False
            if code in "nN+=gG":
                implicit = code in "GN" or (code == "=" and len(packet) == 7) or (code == "+" and len(packet) == 10)
                sid = own if implicit else int.from_bytes(packet[1:3], "big")
                if sid == own and code in "nN+":
                    flags.add("growth")
            if code == "r" and int.from_bytes(packet[1:3], "big") == own:
                flags.add("tail")
            mf = form(packet, own)
            if mf and mf["sid"] == own and own is not None:
                before = dict(state)
                vals = dict(mf["values"])
                if "speed" in vals:
                    vals["speed"] /= 18
                state.update(vals)
                new_direction = mf["direction"] if mf["direction"] is not None else direction
                changed = [k for k in ("heading", "wanted", "speed") if state.get(k) != before.get(k)]
                motions[key].append(dict(line=line, packetIndex=pi, serverMs=item.server,
                    receiptMs=item.t, before=before, after=dict(state), fields=mf["mask"],
                    directionChanged=new_direction != direction, directionOnly=not changed and new_direction != direction,
                    changedFields=changed, packetDigest=hashlib.sha256(packet).hexdigest()))
                direction = new_direction
        frame_flags[line] = sorted(flags)
    return dict(frames=frames, pings=pings, motions=motions, commands=commands, flags=frame_flags,
                frameTimes={k:[f[0] for f in v] for k,v in frames.items()},
                frameLines={k:[f[2] for f in v] for k,v in frames.items()},
                pingTimes={k:[f[1] for f in v] for k,v in pings.items()},
                motionTimes={k:[m['serverMs'] for m in v] for k,v in motions.items()},
                motionPositions={k:{(m['line'],m['packetIndex']):i for i,m in enumerate(v)} for k,v in motions.items()})


def clock_fit(raw):
    slopes = {}
    for key, frames in raw["frames"].items():
        bins = {}
        for t, s, _ in frames:
            k = int(s // 5000)
            if k not in bins or t - s < bins[k][0] - bins[k][1]:
                bins[k] = (t, s)
        points = list(bins.values())
        if len(points) < 2:
            continue
        mt = sum(t for t, s in points) / len(points)
        ms = sum(s for t, s in points) / len(points)
        den = sum((s - ms) ** 2 for t, s in points)
        if den:
            slopes[key] = sum((s - ms) * (t - mt) for t, s in points) / den
    return slopes


def clock_bounds(raw, slopes, key, q, reflection):
    b = slopes.get(key)
    if b is None or b <= 0:
        return None
    times=raw['frameTimes'][key]
    l=bisect.bisect_left(times,q-5000);h=bisect.bisect_right(times,q+5000)
    uppers = [t - b * s for t, s, _ in raw["frames"][key][l:h]]
    times=raw['pingTimes'].get(key,[])
    l=bisect.bisect_left(times,q-5000);h=bisect.bisect_right(times,q+5000)
    lowers = [sent - b * s for sent, received, s in raw["pings"][key][l:h]]
    if not uppers or not lowers or max(lowers) > min(uppers):
        return None
    low = (q - min(uppers)) / b
    high = (q - max(lowers)) / b
    return dict(sendServerLower=low, sendServerUpper=high,
                reflectionDelayLower=reflection - high, reflectionDelayUpper=reflection - low)


def evaluate(alias, ordinal, split, kind, command_line, sent, bounds, reflection,
             key, raw, enabled=None):
    lo = bounds["sendServerLower"]
    hi = bounds["sendServerUpper"]
    s = reflection["serverMs"]
    motion = raw["motions"][key]
    # A reflection at the same timestamp is still excluded by packet order.
    limit=raw['motionPositions'][key][(reflection['jsonlLine'],reflection['packetIndex'])]
    ts = raw['motionTimes'][key]
    i = bisect.bisect_left(ts, lo,0,limit) - 1
    j = bisect.bisect_left(ts, hi,0,limit) - 1
    prior = motion[i] if i >= 0 else None
    state = "no-prior"
    bucket = "no-prior"
    ticks = None
    if prior is not None:
        if i != j:
            state = bucket = "report-in-clock-interval"
        elif hi < prior["serverMs"] + 42:
            state = "armed"
            low_ticks = max(1, math.ceil((prior["serverMs"] + 42 - hi) / 7))
            high_ticks = max(1, math.ceil((prior["serverMs"] + 42 - lo) / 7))
            ticks = [low_ticks, high_ticks]
            bucket = str(max(1, math.ceil((prior["serverMs"] + 42 - (lo + hi) / 2) / 7)))
        elif lo >= prior["serverMs"] + 42:
            state = bucket = "open"
        else:
            state = bucket = "reopen-boundary"
    row = dict(archiveAlias=alias, ordinal=ordinal, split=split, kind=kind,
               commandLine=command_line, connection=key[0], epoch=key[1],
               reflectionLine=reflection["jsonlLine"], reflectionPacketIndex=reflection["packetIndex"],
               sendServerLower=lo, sendServerUpper=hi, clockWidthMs=hi-lo,
               arrivalCausalLower=lo, arrivalCausalUpper=s,
               reflectionServerMs=s, reflectionDelayLower=s-hi, reflectionDelayUpper=s-lo,
               state=state, bucket=bucket, ticksRemainingRange=ticks, boostEnabled=enabled)
    # Without inbound timestamps, ingress is only causally bracketed by the
    # earliest feasible send and reflection. Keep this separate from send bins.
    arrival_history = motion[max(0,i):limit]
    row['arrivalPreviousMotionServerCandidates'] = [m['serverMs'] for m in arrival_history]
    if prior is None:
        row['arrivalGateState'] = 'unknown-initial-history'
    else:
        checkpoints = [m['serverMs'] for m in arrival_history]
        continuously_armed = all(b-a <= 42 for a,b in zip(checkpoints,checkpoints[1:])) and s-checkpoints[-1] < 42
        definitely_open = len(checkpoints)==1 and lo>=checkpoints[0]+42
        row['arrivalGateState'] = 'definitely-armed' if continuously_armed else 'definitely-open' if definitely_open else 'mixed'
    raw_command = raw["commands"][command_line]
    assert abs(raw_command["t"] - sent) < 1e-8
    actual = motion[limit]
    # Exact desired byte needs to change the causal received value at send AND
    # the value immediately preceding the observed reflecting packet.
    if kind == "wanted":
        target = math.floor(raw_command["byte"] * 256 / 251)
        changed = raw_command["priorWanted"] is not None and abs(target - raw_command["priorWanted"]) > 1e-7
        row["encodedWantedChangesAtSend"] = changed
        row["wantedActuallyChangesAtReflection"] = "wanted" in actual["changedFields"]
        assert abs(actual["after"]["wanted"] - target) < 1e-8
    else:
        row["speedActuallyChangesAtReflection"] = "speed" in actual["changedFields"]
    row["commandActiveAtSend"] = raw_command["active"]
    row["reflectionDirectionOnly"] = actual["directionOnly"]
    row["reflectionChangesWantedAndSpeed"] = "wanted" in actual["changedFields"] and "speed" in actual["changedFields"]
    row["reflectionGrowthTail"] = raw["flags"][actual["line"]]
    row["previousGrowthTail"] = raw["flags"][prior["line"]] if prior else []
    row["previousDirectionOnly"] = prior["directionOnly"] if prior else None
    prev_index=bisect.bisect_left(raw['frameLines'][key],actual['line'])-1
    prev_frame = raw['frames'][key][prev_index] if prev_index>=0 else None
    row["reflectionFrameGapMs"] = s-prev_frame[1] if prev_frame else None
    row["nextEmittedFrameAfterSendDefinite"] = bool(prev_frame and prev_frame[1] <= lo and hi <= s)
    row["nextEmittedFrameAfterSendPossible"] = bool(prev_frame and prev_frame[1] <= hi and lo <= s)
    row["previousMotionServerMs"] = prior["serverMs"] if prior else None
    row["previousMotionLine"] = prior["line"] if prior else None
    row["previousMotionPacketIndex"] = prior["packetIndex"] if prior else None
    row["contradictionUpperBound"] = False
    row["contradictionAbsolutePaired"] = False
    row["contradictionPlus2ms"] = False
    row["contradiction35msFloor"] = False
    if state == "armed":
        p = prior["serverMs"]
        row["earliestAllowedDelayLower"] = p+42-hi
        row["marginMs"] = p+42-s-(hi-lo)
        row["contradictionUpperBound"] = s-lo < p+42-hi
        row["contradictionAbsolutePaired"] = s < p+42
        row["contradictionPlus2ms"] = s-lo+2 < p+42-hi
        row["contradiction35msFloor"] = s-lo < p+35-hi
        row["wireGapFromPreviousMotionMs"] = s-p
    return row


def summarize(rows):
    out = dict(n=len(rows))
    for endpoint in ("Lower", "Upper"):
        xs = [r["reflectionDelay"+endpoint] for r in rows]
        out[endpoint.lower()] = {str(int(q*100)): quantile(xs, q) for q in (.5, .9, .95)}
    armed = [r for r in rows if r["state"] == "armed"]
    out["armed"] = len(armed)
    for name in ("contradictionUpperBound", "contradictionAbsolutePaired", "contradictionPlus2ms", "contradiction35msFloor"):
        out[name] = sum(r[name] for r in armed)
    out["tickRangeUnambiguous"] = sum(r["ticksRemainingRange"] is not None and len(set(r["ticksRemainingRange"])) == 1 for r in rows)
    return out


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--p2", type=Path, required=True)
    p.add_argument("--decoder-dir", type=Path, required=True)
    p.add_argument("--older", type=Path)
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()
    out = args.output
    out.mkdir(parents=True, exist_ok=False)
    sys.path.insert(0, str(args.decoder_dir))
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "2026-10-26-movement-scheduler"))
    import slither_protocol as sp
    import analyze_p2 as a
    rows = []
    pins = dict(decoderSha256=sha(args.decoder_dir / "slither_protocol.py"), archives={}, analyses={})
    checks = []
    for n in range(1, 7):
        alias = f"p2-ordinal-{n:02d}"
        analysis_path = args.p2 / "analysis" / f"ordinal-{n:02d}-p2-analysis-final.json"
        d = load(analysis_path)
        integrity = load(args.p2 / "analysis" / f"ordinal-{n:02d}-integrity.json")
        path = args.p2 / integrity["members"][0]["capture"]
        assert sha(path) == d["archiveSha256"]
        pins["archives"][alias] = d["archiveSha256"]
        pins["analyses"][alias] = sha(analysis_path)
        raw = scan(path, sp, a.motion_form)
        raw_lookup = {(m["line"], m["packetIndex"]): m for group in raw["motions"].values() for m in group}
        own = [m for m in d["packetRows"]["motion"] if m["own"]]
        assert len(own) == len(raw_lookup)
        for m in own:
            r = raw_lookup[(m["jsonlLine"], m["packetIndex"])]
            assert r["serverMs"] == m["serverMs"] and r["receiptMs"] == m["receiptMs"]
            assert r["packetDigest"] == hashlib.sha256(bytes.fromhex(m["packetHex"])).hexdigest()
        slopes = clock_fit(raw)
        counts = Counter()
        for kind, commands, qualified, effect in (
                ("wanted", d["headings"], "isolatedQualified", "firstReflection"),
                ("boost", d["boosts"], "qualifiedObservedTransition", "firstDirectionalSpeedReflection")):
            for c in commands:
                if not c.get(qualified):
                    continue
                counts[kind+"/qualified"] += 1
                key = (c["connection"], c["epoch"])
                b = clock_bounds(raw, slopes, key, c["sentReceiptMs"], c[effect]["serverMs"])
                if b is None:
                    counts[kind+"/missingClock"] += 1
                    continue
                if kind == "wanted":
                    for k in b:
                        assert abs(b[k]-c["clockBounds"][k]) < 1e-6, (n, c["line"], k)
                rows.append(evaluate(alias, n, "development" if n<=3 else "heldout", kind,
                    c["line"], c["sentReceiptMs"], b, c[effect], key, raw, c.get("enabled")))
        checks.append(dict(ordinal=n, rawOwnMotionRowsVerified=len(own), counts=dict(counts)))
        print(alias, dict(counts), flush=True)
        del d, raw
    if args.older:
        run_older(args.older, sp, a.motion_form, rows, pins, checks)
    pins['p2StudyCommit'] = publication.P2_COMMIT
    for name,value in (("selected-results.json",publication.select(rows)),("source-pins.json",pins),("checks.json",checks)):
        publication.write_json(out/name,value)
    (out/'TABLES.md').write_text(publication.tables(rows),encoding='utf-8',newline='\n')


def run_older(root, sp, form, rows, pins, checks):
    # Read existing ledgers/archives only. Do not execute old cache builders.
    matches = defaultdict(list)
    for line in (root / "input-matches.jsonl").read_text(encoding="utf-8").splitlines():
        c = json.loads(line)
        if c["clean"]:
            matches[c["file"]].append(c)
    inventory = {r["relative"]:r for r in load(root / "inventory.json")}
    manifest = {r['relative']:r['sha256'] for r in load(root/'manifest.json')['captures']}
    pins["olderMatchesSha256"] = sha(root / "input-matches.jsonl")
    counts = Counter()
    for index, (name, commands) in enumerate(sorted(matches.items()),1):
        alias = f"older-{index:02d}"
        path = Path(inventory[name]["path"])
        if not path.is_file():
            raise FileNotFoundError(f"{alias}: older archive is missing")
        pins["archives"][alias] = sha(path)
        assert pins['archives'][alias] == manifest[name]
        raw = scan(path,sp,form)
        cmd_by_t = {(v["connection"],v["t"]):(line,v) for line,v in raw["commands"].items()}
        for c in commands:
            counts[c["kind"]+"/clean"] += 1
            match = cmd_by_t.get((c["conn"],c["command_t"]))
            if match is None:
                counts["commandNotFound"] += 1
                continue
            line,raw_command = match
            key = (c["conn"],raw_command["epoch"])
            reflected = [m for m in raw["motions"][key] if m["line"]==c["effect_line"]]
            if len(reflected)!=1:
                counts["ambiguousReflection"] += 1
                continue
            m = reflected[0]
            if c["kind"]=="steer" and ("wanted" not in m["changedFields"] or
                    raw_command["priorWanted"] is None or abs(math.floor(c["target"])-raw_command["priorWanted"])<=1e-7):
                counts["unchangedWantedExcluded"] += 1
                continue
            bounds=dict(sendServerLower=(c["command_t"]-c["offset_upper"])/c["clock_b"],
                        sendServerUpper=(c["command_t"]-c["offset_lower"])/c["clock_b"])
            reflection=dict(serverMs=c["effect_server"],jsonlLine=m["line"],packetIndex=m["packetIndex"])
            rows.append(evaluate(alias,index,"older","wanted" if c["kind"]=="steer" else "boost",
                line,c["command_t"],bounds,reflection,key,raw,c["target"]==253 if c["kind"]=="boost" else None))
        print(alias,len(commands),flush=True)
    checks.append(dict(older=dict(counts), archives=len(matches)))


if __name__ == "__main__":
    main()
