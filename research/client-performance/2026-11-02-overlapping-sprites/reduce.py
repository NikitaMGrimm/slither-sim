"""Reduce retained authored diagnostics; no replay, GPU work or third-party modules."""
import argparse
import ast
from collections import Counter, defaultdict
import csv
from datetime import datetime
import hashlib
import json
import math
from pathlib import Path
import re
from statistics import mean, median
import zlib

STAGES = ("food-outlines", "world-before-glows", "food-glows-coarse", "food-glows-fine",
          "food-glows-half", "world-after-glows", "world", "overlay", "msaa-resolve-only")
VARIANTS = ("no-body", "no-boost", "no-shadow", "no-snakes", "no-food", "resolve-only", "msaa1")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_csv(path):
    with path.open(encoding="utf-8-sig", newline="") as stream:
        return list(csv.DictReader(stream))


def write_csv(path, rows):
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def quantile(values, fraction):
    return sorted(values)[math.ceil(len(values) * fraction) - 1]


def profile(root, output):
    runs, coverage = [], []
    for zoom in ("0", "-120"):
        name = f"profile-native-{zoom}"
        log = (root / (name + ".log")).read_text(encoding="utf-8-sig")
        receipt = json.loads((root / (name + "-receipt.json")).read_text(encoding="utf-8-sig"))
        require(receipt["exit"] == 0 and "complete=true,messages=0," in log and "skips=0" in log, "incomplete profile")
        for token in ("device: 5056", "device_type: IntegratedGpu", "backend: Dx12", 'driver: "32.0.21001.9024"'):
            require(token in log, "profile adapter differs")
        rows = read_csv(root / (name + ".csv"))
        expected = Counter((str(f), str(p), d, side, str(i)) for f in (60, 120, 180)
                           for p in range(1, 6) for d in VARIANTS for side in ("before", "after") for i in range(10))
        require(Counter(tuple(r[k] for k in ("frame", "pair", "diagnostic", "side", "iteration")) for r in rows) == expected, "incomplete/duplicate profile samples")
        stages = defaultdict(dict)
        order = []
        cell, iteration = None, None
        for line in log.splitlines():
            if line.startswith("cell="):
                cell = tuple(line[5:].split(","))
                order.append(cell)
            elif line.startswith("iteration="):
                iteration = int(line.split("=")[1]) - 5
            elif line.startswith("pass ") and iteration is not None and iteration >= 0:
                stage, value = line[5:].split("=")
                require(stage in STAGES, "unknown profile stage")
                key = (*cell, str(iteration))
                require(stage not in stages[key], "duplicate pass sample")
                stages[key][stage] = float(value) / 1e6
            elif line.startswith("coverage "):
                match = re.fullmatch(r"coverage frame=(\d+) quads=(\[.*\]) areas=(\[.*\])", line)
                require(match is not None, "invalid coverage record")
                counts, areas = ast.literal_eval(match[2]), ast.literal_eval(match[3])
                names = ("food-body", "food-outline", "food-glow", "snake-body", "snake-boost", "snake-shadow", "snake-death", "other")
                coverage.append(dict(zoom=zoom, frame=match[1], **{n + "_quads": counts[i] for i, n in enumerate(names)},
                                     **{n + "_screen_area": f"{areas[i]:.12f}" for i, n in enumerate(names)}))
        for i in range(0, len(order), 2):
            a, b = order[i:i+2]
            require(a[:3] == b[:3] and (a[3], b[3]) == (("before", "after") if int(a[1]) % 2 else ("after", "before")), "profile arms not matched/alternating")
        for r in rows:
            key = tuple(r[k] for k in ("frame", "pair", "diagnostic", "side", "iteration"))
            require(key in stages and abs(sum(stages[key].values()) - float(r["gpu_ns"]) / 1e6) <= .00001, "pass sum disagrees with GPU total")
        for f in ("60", "120", "180"):
            for p in range(1, 6):
                for d in VARIANTS:
                    for side in ("before", "after"):
                        selected = [r for r in rows if (r["frame"], r["pair"], r["diagnostic"], r["side"]) == (f, str(p), d, side)]
                        values = [float(r["gpu_ns"]) / 1e6 for r in selected]
                        runs.append(dict(run_id=f"profile-{zoom}-{f}-{p}-{d}-{side}", zoom=zoom, frame=f, pair=p, diagnostic=d, side=side,
                                         samples=10, gpu_median_ms=f"{median(values):.9f}", gpu_p95_ms=f"{quantile(values,.95):.9f}",
                                         **{stage + "_mean_ms": f"{mean(stages[(f,str(p),d,side,str(i))].get(stage,0) for i in range(10)):.9f}" for stage in STAGES},
                                         binary_sha256=receipt["binarySha256"].lower(), raw_csv_sha256=sha(root/(name+".csv")),
                                         raw_log_sha256=sha(root/(name+".log")), process_log_sha256=sha(root/(name+"-processes.json")),
                                         load_min=receipt["loadMin"], load_max=receipt["loadMax"]))
    write_csv(output / "profile-runs.csv", runs)
    write_csv(output / "coverage.csv", coverage)


def history(root, output, label):
    receipts = list(root.glob(label + "-*-receipt.json"))
    require(len(receipts) == 20, "incomplete history receipt set")
    ordered = []
    for path in receipts:
        receipt = json.loads(path.read_text(encoding="utf-8-sig"))
        name = receipt["id"]
        log = (root / (name + ".log")).read_text(encoding="utf-8-sig")
        require(receipt["exit"] == 0 and "complete=true,messages=0," in log and "skips=0" in log, "failed/incomplete history")
        observations = json.loads((root / (name + "-processes.json")).read_text(encoding="utf-8-sig"))
        ordered.append((datetime.fromisoformat(observations[0]["utc"]), receipt, observations, path))
    ordered.sort(key=lambda item: item[0])
    expected = [(pair, zoom, side) for pair in range(1, 6) for zoom in (0, -120)
                for side in (("before", "after") if pair % 2 else ("after", "before"))]
    require([(r["pair"], r["zoom"], r["side"]) for _, r, _, _ in ordered] == expected,
            "incomplete/mislabeled/nonalternating history cells")
    results = []
    for index, (start, receipt, observations, path) in enumerate(ordered, 1):
        name = receipt["id"]
        require(name == f"{label}-{receipt['pair']}-{receipt['zoom']}-{receipt['side']}", "history id differs from labels")
        raw = read_csv(root / (name + ".csv"))
        require(Counter(r["frame"] for r in raw) == Counter(str(i) for i in range(181)), "incomplete/duplicate history callbacks")
        rows = [r for r in raw if int(r["frame"]) >= 40]
        result = dict(run_id=name, pair=receipt["pair"], zoom=receipt["zoom"], side=receipt["side"],
                      launch_index=index, relative_start_s=f"{(start-ordered[0][0]).total_seconds():.6f}",
                      callbacks=len(raw), samples=len(rows), world_samples=receipt["samples"],
                      binary_sha256=receipt["binarySha256"].lower(), exit=receipt["exit"],
                      load_min=receipt["loadMin"], load_max=receipt["loadMax"], compile_overlap=int(receipt["compileOverlap"]),
                      processes_max=max(len(o["processes"]) for o in observations),
                      browsers_max=max(sum(p["ProcessName"].lower() in {"chrome", "msedge", "firefox"} for p in o["processes"]) for o in observations),
                      node_max=max(sum(p["ProcessName"].lower() == "node" for p in o["processes"]) for o in observations),
                      raw_csv_sha256=sha(root/(name+".csv")), raw_log_sha256=sha(root/(name+".log")),
                      process_log_sha256=sha(root/(name+"-processes.json")), receipt_sha256=sha(path))
        for metric in ("gpu", "sync", "submit"):
            values = [float(r[metric + "_ns"]) / 1e6 for r in rows]
            result[metric + "_median_ms"] = f"{median(values):.9f}"
            result[metric + "_p95_ms"] = f"{quantile(values,.95):.9f}"
        for metric in ("draws", "body_quads", "outline_quads", "glow_quads", "other_quads"):
            result[metric + "_peak"] = max(int(r[metric]) for r in rows)
        if label.startswith('selected-'):
            # Older exploratory probes queried the renderer after the overlay
            # and could report zero. Only the final driver counts both worlds.
            result['gpu_calls_median'] = median(int(r['calls']) for r in rows)
            result['gpu_calls_peak'] = max(int(r['calls']) for r in rows)
        results.append(result)
    write_csv(output / (label + "-runs.csv"), results)


def sparse(root, output):
    results = []
    starts = {}
    for pair in range(1, 6):
        for side in ('before', 'after'):
            name = f'sparse-{pair}-{side}'
            observations = json.loads((root/(name+'-processes.json')).read_text(encoding='utf-8-sig'))
            starts[name] = datetime.fromisoformat(observations[0]['utc'])
    ordered = sorted(starts, key=starts.get)
    require(ordered == [f'sparse-{p}-{side}' for p in range(1,6)
                        for side in (('before','after') if p % 2 else ('after','before'))], 'nonalternating sparse arms')
    for pair in range(1, 6):
        for side in ('before', 'after'):
            name = f'sparse-{pair}-{side}'
            receipt = json.loads((root/(name+'-receipt.json')).read_text(encoding='utf-8-sig'))
            require(receipt['exit'] == 0 and 'complete=true' in (root/(name+'.log')).read_text(encoding='utf-8-sig'), 'failed sparse run')
            raw = read_csv(root/(name+'.csv'))
            expected = Counter((size, case, str(i)) for size in ('1920-original', '1920-native', '3840-native')
                               for case in ('body', 'low-quality', 'one-glow') for i in range(30))
            require(Counter((r['size'], r['case'], r['iteration']) for r in raw) == expected, 'incomplete sparse samples')
            for size in ('1920-original', '1920-native', '3840-native'):
                for case in ('body', 'low-quality', 'one-glow'):
                    selected = [r for r in raw if (r['size'], r['case']) == (size, case)]
                    results.append(dict(run_id=f'{name}-{size}-{case}', pair=pair, side=side, size=size, case=case, samples=30,
                                        launch_index=ordered.index(name)+1,relative_start_s=f"{(starts[name]-starts[ordered[0]]).total_seconds():.6f}",
                                        passes_min=min(int(r['passes']) for r in selected), passes_max=max(int(r['passes']) for r in selected),
                                        gpu_median_ms=f"{median(float(r['gpu_ns'])/1e6 for r in selected):.9f}",
                                        gpu_p95_ms=f"{quantile([float(r['gpu_ns'])/1e6 for r in selected],.95):.9f}",
                                        submit_median_ms=f"{median(float(r['submit_ns'])/1e6 for r in selected):.9f}",
                                        binary_sha256=receipt['binarySha256'].lower(),raw_csv_sha256=sha(root/(name+'.csv')),
                                        process_log_sha256=sha(root/(name+'-processes.json')),load_min=receipt['loadMin'],load_max=receipt['loadMax']))
    write_csv(output/'sparse-runs.csv', results)


def parity(root, output):
    results, snapshots = [], []
    for policy in ('native', 'original'):
        for zoom in ('0', '-120'):
            names = [f'exact-{policy}-{zoom}-{side}' for side in ('before', 'after')]
            logs = [(root/(name+'.log')).read_text(encoding='utf-8-sig') for name in names]
            digests = [re.search(r'complete=true,messages=0,digest=([0-9a-f]+),skips=0', log) for log in logs]
            require(all(digests) and digests[0][1] == digests[1][1], 'CPU/resource/send digest mismatch')
            receipts = [json.loads((root/(name+'-receipt.json')).read_text(encoding='utf-8-sig')) for name in names]
            require(all(r['exit'] == 0 for r in receipts), 'failed exactness run')
            hashes = [hashlib.sha256(), hashlib.sha256()]
            changed_frames, changed_bytes, worst, alpha, first = 0, 0, 0, 0, ''
            for frame in range(181):
                data = [zlib.decompress((root/(name+f'.all.{frame}.rgba.z')).read_bytes()) for name in names]
                require(all(len(d) == 1920*1080*4 for d in data), 'incomplete RGBA')
                for h, d in zip(hashes, data):
                    h.update(d)
                if data[0] != data[1]:
                    changed_frames += 1
                    first = first or str(frame)
                    for index, (a, b) in enumerate(zip(*data)):
                        if a != b:
                            changed_bytes += 1
                            worst = max(worst, abs(a-b))
                            alpha += index % 4 == 3
                if frame in (60, 120, 180):
                    snapshots.append(dict(policy=policy,zoom=zoom,frame=frame,before_sha256=hashlib.sha256(data[0]).hexdigest(),after_sha256=hashlib.sha256(data[1]).hexdigest()))
            results.append(dict(policy=policy,zoom=zoom,frames=181,bytes_per_frame=1920*1080*4,changed_frames=changed_frames,changed_bytes=changed_bytes,
                                max_byte_difference=worst,alpha_changed_bytes=alpha,first_changed_frame=first,cpu_digest=digests[0][1],
                                before_rgba_sha256=hashes[0].hexdigest(),after_rgba_sha256=hashes[1].hexdigest(),
                                before_binary_sha256=receipts[0]['binarySha256'].lower(),after_binary_sha256=receipts[1]['binarySha256'].lower()))
    write_csv(output/'parity.csv', results)
    write_csv(output/'pixels.csv', snapshots)


def frozen(root, output):
    results=[]
    starts={}
    for pair in range(1,6):
        for side in ('before','after'):
            name=f'frozen-{pair}-{side}'
            receipt=json.loads((root/(name+'-receipt.json')).read_text(encoding='utf-8-sig'))
            observations=json.loads((root/(name+'-processes.json')).read_text(encoding='utf-8-sig'))
            starts[name]=datetime.fromisoformat(observations[0]['utc'])
            require(receipt['exit']==0,'failed frozen run')
            raw=read_csv(root/(name+'.csv'))
            require(Counter((r['frame'],r['repeat']) for r in raw)==Counter((str(f),str(i)) for f in (60,120,180) for i in range(10)),'incomplete frozen samples')
            samples=[]
            current={}
            for line in (root/(name+'.log')).read_text(encoding='utf-8-sig').splitlines():
                if line.startswith('pass '):
                    stage,value=line[5:].split('=')
                    require(stage in STAGES and stage not in current,'duplicate/unknown frozen pass')
                    current[stage]=float(value)/1e6
                    if stage=='overlay':
                        samples.append(current)
                        current={}
            require(len(samples)==45 and not current,'incomplete frozen pass timestamps')
            for i,frame in enumerate((60,120,180)):
                selected=samples[i*15+5:(i+1)*15]
                values=[r for r in raw if r['frame']==str(frame)]
                require(all(abs(sum(s.values())-float(r['gpu_ns'])/1e6)<.00001 for s,r in zip(selected,values)),'frozen pass sum disagrees')
                results.append(dict(run_id=f'{name}-{frame}',pair=pair,side=side,frame=frame,samples=10,
                                    **{stage+'_mean_ms':f"{mean(s.get(stage,0) for s in selected):.9f}" for stage in STAGES},
                                    gpu_median_ms=f"{median(float(r['gpu_ns'])/1e6 for r in values):.9f}",
                                    binary_sha256=receipt['binarySha256'].lower(),raw_csv_sha256=sha(root/(name+'.csv')),
                                    process_log_sha256=sha(root/(name+'-processes.json')),load_min=receipt['loadMin'],load_max=receipt['loadMax']))
    require(sorted(starts,key=starts.get)==[f'frozen-{p}-{s}' for p in range(1,6) for s in (('before','after') if p%2 else ('after','before'))],'nonalternating frozen processes')
    write_csv(output/'frozen-runs.csv',results)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("--output", type=Path, default=Path(__file__).parent)
    parser.add_argument("--series", default="profile", help="profile or the retained history journal label")
    args = parser.parse_args()
    if args.series == 'frozen':
        frozen(args.raw, args.output)
    elif args.series == 'sparse':
        sparse(args.raw, args.output)
    elif args.series == 'parity':
        parity(args.raw, args.output)
    elif args.series == "profile":
        profile(args.raw, args.output)
    else:
        history(args.raw, args.output, args.series)
    print("Reduced complete paired profile; identifying host observations remain local.")
