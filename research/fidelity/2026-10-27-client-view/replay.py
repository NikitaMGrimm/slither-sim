"""Offline capture adapter and summaries; protocol decoding belongs to slither-client.

Uses only the Python standard library. Full inputs and per-packet results stay outside Git.
"""
import argparse
import base64
import ctypes
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess


def limit_cpus():
    if os.name == "nt":
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.GetProcessAffinityMask.argtypes = [ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
        kernel.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        current, system = ctypes.c_size_t(), ctypes.c_size_t()
        handle = kernel.GetCurrentProcess()
        if not kernel.GetProcessAffinityMask(handle, ctypes.byref(current), ctypes.byref(system)):
            raise ctypes.WinError(ctypes.get_last_error())
        bits = [1 << i for i in range(64) if current.value & (1 << i)][:1]
        if not kernel.SetProcessAffinityMask(handle, sum(bits)):
            raise ctypes.WinError(ctypes.get_last_error())
    else:
        os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:1])
    os.environ.update(CARGO_BUILD_JOBS="1", RAYON_NUM_THREADS="1", RUST_TEST_THREADS="1")


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def dump(path, value):
    Path(path).write_text(json.dumps(value, indent=2, allow_nan=False) + "\n",
                          encoding="utf-8", newline="\n")


def export(capture, target, seconds):
    """Select an outcome-independent receipt window from the first incoming frame.

    No payload inspection or simulator decoder. Keep connection and framing metadata.
    The pinned client identifies challenge/setup and validates protocol and packets.
    """
    first = None
    connection = 0
    unframed = False
    previous = None
    emitted = 0
    terminal = None
    opener = gzip.open if capture.suffix == ".gz" else open
    with opener(capture, "rt", encoding="utf-8") as source, target.open("w", encoding="utf-8", newline="\n") as out:
        for index, line in enumerate(source, 1):
            row = json.loads(line)
            if isinstance(row, dict):
                if row.get("event") == "connect":
                    connection += 1
                    unframed = False
                if row.get("event") == "unframed-challenge":
                    unframed = True
                if row.get("event") in ("capture-end", "capture-interrupted"):
                    terminal = row["event"]
                continue
            direction, receipt, encoded = row
            if not math.isfinite(receipt) or (previous is not None and receipt < previous):
                raise ValueError("invalid/regressing capture clock")
            previous = receipt
            if direction not in ("i", "o"):
                raise ValueError("unsupported capture direction")
            if direction != "i":
                continue
            if first is None:
                first = receipt
            if receipt > first + seconds * 1000:
                continue
            raw = base64.b64decode(encoded, validate=True)
            out.write(json.dumps(dict(line=index, connection=connection, t_ms=receipt,
                                      bytes=list(raw), unframed=unframed), separators=(",", ":")) + "\n")
            emitted += 1
    if not emitted:
        raise ValueError("no incoming frames in selected capture")
    return dict(source_sha256=sha(capture), window_seconds=seconds,
                window_origin="first incoming receipt; includes every reconnect within window",
                first_incoming_ms=first, input_frames=emitted, terminal=terminal)


def quantiles(values):
    values = sorted(values)
    if not values:
        return dict(n=0, p50=None, p95=None, p99=None, maximum=None, mean=None)
    return dict(n=len(values), **{f"p{q}": values[math.ceil(q / 100 * len(values)) - 1]
        for q in (50, 95, 99)}, maximum=values[-1], mean=math.fsum(values) / len(values))


def summarize(rows):
    groups = {}
    audit = None
    for line in Path(rows).open(encoding="utf-8"):
        row = json.loads(line)
        kind = row["kind"]
        if kind == "audit":
            audit = row
            continue
        names = [kind + "/all_received"]
        if row["own_alive"]:
            names.append(kind + "/own_alive")
        if kind == "prey_motion" and row["own_alive"] and row["center_in_viewport"]:
            names.append(kind + "/center_in_viewport_own_alive")
        if kind.startswith("snake") and row["own_alive"]:
            s = row["stratum"]
            if s["visible"]:
                names.append(kind + "/visible_own_alive")
                if 100 <= s["segments"] <= 169 and s["speed_per_32ms"] < 9:
                    names.append(kind + "/visible_100_169_cruise")
        metrics = {k: v for k, v in row.items() if k in {
            "head_innovation", "head_immediate_display_shift", "body_raw_contraction_max",
            "body_immediate_display_shift_max", "body_new_front_correction", "heading_error_rad",
            "heading_immediate_display_shift_rad", "speed_error_per_32ms", "innovation",
            "immediate_display_shift"}}
        if "stratum" in row:
            metrics.update(segments=row["stratum"]["segments"],
                           speed_per_32ms=row["stratum"]["speed_per_32ms"])
        for name in names:
            group = groups.setdefault(name, {})
            for key, value in metrics.items():
                if not math.isfinite(value):
                    raise ValueError("nonfinite metric")
                group.setdefault(key, []).append(value)
    if audit is None:
        raise ValueError("missing terminal harness audit; incomplete replay")
    return dict(audit=audit, groups={g: {k: quantiles(v) for k, v in m.items()}
                                   for g, m in sorted(groups.items())})


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("capture", type=Path)
    p.add_argument("--binary", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--label", required=True)
    p.add_argument("--seconds", type=float, default=120)
    p.add_argument("--clock", choices=("receipt", "server"), default="receipt")
    p.add_argument("--tick-ms", type=float, default=1000 / 60)
    a = p.parse_args()
    if not math.isfinite(a.seconds) or a.seconds <= 0:
        p.error("seconds must be positive and finite")
    limit_cpus()
    a.output.mkdir(parents=True, exist_ok=False)
    provenance = export(a.capture, a.output / "input.jsonl", a.seconds)
    with (a.output / "input.jsonl").open("rb") as source, (a.output / "rows.jsonl").open("wb") as out, (a.output / "stderr.txt").open("wb") as err:
        result = subprocess.run([str(a.binary.resolve()), a.clock, str(a.tick_ms)],
                                 stdin=source, stdout=out, stderr=err, check=False)
    dump(a.output / "attempt.json", dict(label=a.label, **provenance, exit_code=result.returncode,
        binary_sha256=sha(a.binary), clock=a.clock, tick_ms=a.tick_ms))
    if result.returncode:
        raise RuntimeError(f"replay failed ({result.returncode}); retained stderr and partial rows")
    summary = dict(label=a.label, provenance=provenance, **summarize(a.output / "rows.jsonl"))
    dump(a.output / "summary.json", summary)
    print(json.dumps(dict(label=a.label, frames=summary["audit"]["frames"],
                          groups=list(summary["groups"]))))


if __name__ == "__main__":
    main()
