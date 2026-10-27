"""Linux x86-64 small-host study; raw output must be outside the checkout.

python3 reproduce.py build REPOSITORY NEW_OUTPUT --cpu CPU --load-cpu CPU
Then trace, count, ticks, idle, latency, or select with the same arguments.
All validity checks remain active under python -O. No outside servers.
"""
import argparse
import ast
import csv
import gzip
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tarfile
import time

BASE = "4c2471ba9afff3609faae964cdaad49e3209da68"
CANDIDATE = "aee542100496b8cbc031b8ab19bb58e78bd8af2b"
CASES = ["official-1", "official-10", "official-50", "official-100", "battledome-8", "battledome-16", "battledome-32"]
ARMS = ["base", "candidate"]
STUDY = Path(__file__).resolve().parent


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def swap(text, old, new):
    require(text.count(old) == 1, f"fixture anchor: {old!r}")
    return text.replace(old, new)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def edit(path, old, new):
    text = path.read_text()
    require(text.count(old) == 1, f"overlay anchor: {path.name}: {old!r}")
    path.write_text(text.replace(old, new), newline="\n")


def run(command, log, cwd=None, extra=None):
    with log.open("w") as stream:
        subprocess.run(list(map(str, command)), cwd=cwd, env=dict(ENV, **(extra or {})), stdout=stream, stderr=subprocess.STDOUT, check=True)


def binary(arm, name="study_small"):
    return ROOT / arm / "bin" / name


def rehash():
    for arm in ARMS:
        pins = json.loads((ROOT / arm / "pins.json").read_text())
        for name, digest in pins["binaries"].items():
            require(sha(binary(arm, name)) == digest, f"binary changed: {arm}/{name}")


def quantiles(values):
    values = sorted(values)
    require(bool(values), "empty distribution")
    return {"n": len(values), **{name: values[max(0, math.ceil(q * len(values)) - 1)] for name, q in [("p50", .5), ("p99", .99), ("max", 1)]}}


def paired(rows):
    selected = [r for r in rows if r["pair"] >= 0]
    ratios = [r["candidate"]["instructions"] / r["base"]["instructions"] for r in selected]
    return {"baseMedian": statistics.median(r["base"]["instructions"] for r in selected), "candidateMedian": statistics.median(r["candidate"]["instructions"] for r in selected), "ratioMedian": statistics.median(ratios), "ratioRange": [min(ratios), max(ratios)], "pairs": rows}


def counter(path):
    value = json.loads(path.read_text())
    require(value["enabled"] > 0 and value["running"] / value["enabled"] >= .99, "PMU scheduling below 99%")
    return value


def build():
    ROOT.mkdir(parents=True, exist_ok=False)
    shutil.copy2(STUDY / "PLAN.md", ROOT / "PLAN.md")
    run(["cc", "-O2", "-Wall", "-Wextra", STUDY / "pmu.c", "-o", ROOT / "pmu"], ROOT / "pmu-build.log")
    for arm, ref in zip(ARMS, [ARGS.base, ARGS.candidate]):
        out = ROOT / arm
        source = out / "source"
        source.mkdir(parents=True)
        archive = out / "source.tar"
        if ARGS.archives:
            shutil.copy2(ARGS.archives / f"{arm}.tar", archive)
            with tarfile.open(archive) as stream:
                full = stream.pax_headers.get("comment", "")
            require(bool(full) and full.startswith(ref), "archive commit mismatch")
        else:
            full = subprocess.check_output(["git", "-C", str(REPO), "rev-parse", ref], text=True).strip()
            subprocess.run(["git", "-C", str(REPO), "archive", "--format=tar", "-o", str(archive), full], check=True)
        with tarfile.open(archive) as stream:
            stream.extractall(source, filter="data")
        prior = REPO / "research/performance/2026-10-27-host-snake-candidates"
        fixture = (prior / "protocol_fixture.rs").read_text()
        start = fixture.index("    let (count, legacy) = match case {")
        end = fixture.index("    let config =", start)
        fixture = fixture[:start] + '''    let (preset, count) = case.rsplit_once('-').unwrap();
    let count: usize = count.parse().unwrap();
    let legacy = 0;
''' + fixture[end:]
        fixture = swap(fixture, "world: crate::Config::official(count),", 'world: if preset == "battledome" { crate::Config::battledome() } else { crate::Config::official(count) },')
        fixture = swap(fixture, "// Current official-500 configuration; no custom geometry or admission policy.", "// Preserve the host's seat-based preset initialization.")
        fixture = swap(fixture, "    for slot in 0..count {\n        assert!", "    let mut initial_refused = 0;\n    for slot in 0..count {\n        assert!")
        fixture = swap(fixture, '        assert!(sim.connect_waited(slot, profile(slot), 2.).unwrap(), "initial slot {slot}");', '        if !sim.connect_waited(slot, profile(slot), 2.).unwrap() { initial_refused += 1; }')
        fixture = swap(fixture, '"frames":frames,"bytes":bytes,', '"initialRefused":initial_refused,"frames":frames,"bytes":bytes,')
        fixture = swap(fixture, 'if tick == 720 { sim.disconnect(0).unwrap(); }', 'if tick == 720 { sim.disconnect(0).unwrap(); forced = true; }')
        # Battledome can refuse simultaneous admissions at these sizes. Keep its
        # real admission policy and all retry work; report actual population.
        with (source / "src/client/simulation.rs").open("a") as stream:
            stream.write("\npub mod study_small;\n")
        module = source / "src/client/simulation/study_small.rs"
        module.write_text(fixture, newline="\n")
        prepare_memory(source)
        (source / "src/client/simulation/study_small").mkdir()
        shutil.copy2(prior / "tick_counter.rs", source / "src/client/simulation/study_small/tick_counter.rs")
        (source / "examples/study_small.rs").write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_small::run(&a[1],a.get(2).is_some_and(|s|s=="--trace"));}\n')
        prepare_idle(source)
        prepare_latency(source)
        names = ["study_small", "study_idle", "study_host", "host_capacity_load"]
        target = (ARGS.target_root / arm / "target").resolve() if ARGS.target_root else out / "target"
        run(["cargo", "build", "--release", "--locked", *[x for name in names for x in ["--example", name]]], out / "build.log", source, {"CARGO_TARGET_DIR": str(target)})
        (out / "bin").mkdir()
        for name in names:
            shutil.copy2(target / "release/examples" / name, binary(arm, name))
        pins = {"commit": full, "archiveSha256": sha(archive), "binaries": {n: sha(binary(arm, n)) for n in names}}
        (out / "pins.json").write_text(json.dumps(pins, indent=2) + "\n")
        print("BUILT", arm, flush=True)


def prepare_idle(source):
    prior = REPO / "research/performance/2026-10-27-host-socket-readiness"
    server = source / "src/client/server.rs"
    text = server.read_text()
    ready = (source / "src/client/server/readiness.rs").exists()
    indexed = "session_by_slot:" in text
    idle = (prior / "idle.rs").read_text()
    idle = idle.replace("    let n = 500;", '    let n: usize = std::env::args().nth(1).unwrap().parse().unwrap();')
    idle = idle.replace("/* READINESS */", "readiness: readiness::Readiness::default()," if ready else "")
    if indexed:
        idle = idle.replace("        sessions: Vec::new(),", "        sessions: Vec::new(),\n        session_by_slot: vec![None; n],")
    idle = idle.replace("    crate::socket_study::active(true);", "")
    idle = idle.replace("    crate::socket_study::active(false);", "")
    idle = idle.replace('println!("{{\\\"sockets\\\":500,\\\"passes\\\":4000}}");', 'println!("{{\\\"sockets\\\":{n},\\\"passes\\\":4000}}");')
    server.write_text(text + "\npub mod study_idle;\n", newline="\n")
    (source / "src/client/server").mkdir(exist_ok=True)
    (source / "src/client/server/study_idle.rs").write_text(idle, newline="\n")
    (source / "examples/study_idle.rs").write_text('fn main(){slither_sim::client::server::study_idle::run();}\n')


def prepare_latency(source):
    # Same tick busy boundaries as the EPYC recheck, without its CLI/cloud runner.
    server = source / "src/client/server.rs"
    edit(server, "        let dt = self.shared.clock.dt;\n        while !self.shared.stopped() {", '''        let dt = self.shared.clock.dt;
        let study_origin = Instant::now();
        let study_unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
        let mut study_ticks: Vec<(u64,u64)> = Vec::with_capacity(32768);
        while !self.shared.stopped() {''')
    edit(server, "            for _ in 0..due {", "            for _ in 0..due {\n                let study_tick = Instant::now();")
    edit(server, "                self.hand_over()?;", "                self.hand_over()?;\n                study_ticks.push((study_tick.duration_since(study_origin).as_nanos() as u64, study_tick.elapsed().as_nanos() as u64));")
    text = server.read_text()
    start = text.index("impl Simulation {\n    fn run(")
    end = text.index("\n    fn due_ticks(", start)
    body = text[start:end]
    require(body.count("        Ok(())") == 1, "tick exit anchor")
    body = body.replace("        Ok(())", '''        if let Ok(path) = std::env::var("STUDY_TICKS_JSON") {
            std::fs::write(path, serde_json::to_vec(&(study_unix, study_ticks)).unwrap()).unwrap();
        }
        Ok(())''')
    server.write_text(text[:start] + body + text[end:], newline="\n")
    shutil.copy2(REPO / "research/performance/2026-10-26-host-capacity/host_probe.rs", source / "examples/study_host.rs")
    load = source / "examples/host_capacity_load.rs"
    run(["patch", "-o", load, REPO / "research/performance/2026-10-26-host-capacity/host_capacity_load.rs", REPO / "research/performance/2026-10-26-host-known-slots/host-players.patch"], source.parent / "load-patch.log")
    # Reuse the existing legacy handshake overlay, retaining its checks under -O.
    overlay = REPO / "research/performance/2026-10-27-host-socket-readiness/load_overlay.py"
    tree = ast.parse(overlay.read_text())
    class Checks(ast.NodeTransformer):
        def visit_Assert(self, node):
            return ast.copy_location(ast.If(test=ast.UnaryOp(op=ast.Not(), operand=node.test), body=[ast.Raise(exc=ast.Call(func=ast.Name(id="RuntimeError", ctx=ast.Load()), args=[node.msg or ast.Constant("historical overlay check")], keywords=[]), cause=None)], orelse=[]), node)
    overlay_copy = source.parent / "load_overlay.py"
    overlay_copy.write_text(ast.unparse(ast.fix_missing_locations(Checks().visit(tree))) + "\n")
    run(["python3", overlay_copy, load], source.parent / "load-overlay.log")
    # Restore cumulative-header receipt records removed by the drain-only overlay.
    edit(load, '            ctx.count("framesIn");ctx.add("bytesIn",bytes.len() as u64);', '''            if packets.iter().any(|p|p[0]==b'a') { self.server_ms=0; }
            else {
                self.server_ms += (u64::from(bytes[0]) << 8) | u64::from(bytes[1]);
                if ctx.in_window(at) {ctx.frame_records.push((self.id,self.connections,at*1000.,self.server_ms,bytes.len()));}
            }
            ctx.count("framesIn");ctx.add("bytesIn",bytes.len() as u64);''')
    prepare_inputs(source, load)


def prepare_inputs(source, load):
    prior = REPO / "research/performance/2026-10-27-host-tick-handoff/instrument.py"
    spec = importlib.util.spec_from_file_location("input_probe", prior)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    server = source / "src/client/server.rs"
    with server.open("a") as stream:
        stream.write("\npub(crate) mod study_input;\n")
    (source / "src/client/server/study_input.rs").write_text(module.INPUT, newline="\n")
    edit(server, "    fn forward(&self, command: Command) -> Result<(), String> {", '''    fn forward(&self, command: Command) -> Result<(), String> {
        if let Command::Message {slot, generation, bytes, ..} = &command {
            study_input::received(*slot, *generation, bytes);
        }''')
    edit(server, "        self.phase = Phase::Playing(slot);\n        gate.send(Command::Connect {", "        self.phase = Phase::Playing(slot);\n        study_input::register(slot, self.generation, &appearance.nickname);\n        gate.send(Command::Connect {")
    connection = source / "src/client/simulation/connection.rs"
    edit(connection, "            let (_, message) = self.slots[slot].connection.uplink.pop_front().unwrap();", '''            let (_, message) = self.slots[slot].connection.uplink.pop_front().unwrap();
            if let Uplink::Message(bytes) = &message {
                crate::client::server::study_input::ingest(&self.profiles[slot].nickname, bytes, now);
            }''')
    edit(load, "        frame_records: Vec<(u64, u64, f64, u64, usize)>,", "        frame_records: Vec<(u64, u64, f64, u64, usize)>,\n        input_records: Vec<(u64,u64,u8,f64)>,")
    edit(load, "            frame_records: Vec::new(),", "            frame_records: Vec::new(),\n            input_records: Vec::new(),")
    edit(load, "            // Drain-only study load: do not build a client world.", '''            if self.id % 64 == 0 && bytes.len()==1 && bytes[0]<=250 && ctx.in_window(at) {
                ctx.input_records.push((self.id,self.connections,bytes[0],ctx.now()*1000.));
            }
            // Drain-only study load: do not build a client world.''')
    edit(load, '        frames_out.flush().map_err(|e| e.to_string())?;', '''        frames_out.flush().map_err(|e| e.to_string())?;
        std::fs::write(args.output.with_extension("inputs.json"), serde_json::to_vec(&(epoch_unix_ms,&ctx.input_records)).unwrap()).map_err(|e| e.to_string())?;''')


def prepare_memory(source):
    cache = source / "src/food_grid.rs"
    if "struct PelletIndex" in cache.read_text():
        body = '''
        self.pellets.as_ref().map_or(serde_json::json!({"indexPresent":false}), |p| {
            let vector_bytes = p.slots.capacity()*std::mem::size_of::<PelletSlot>()
                + (p.food_slots.capacity()+p.free.capacity())*4
                + p.cells.values().map(|v|v.capacity()*4).sum::<usize>();
            serde_json::json!({"indexPresent":true,"logicalPellets":p.food_slots.len(),
                "slotBytes":std::mem::size_of::<PelletSlot>(),"vectorPayloadBytes":vector_bytes,
                "occupiedCells":p.cells.len(),"mapCapacity":p.cells.capacity(),
                "mapEntryBytes":std::mem::size_of::<(u32,Vec<u32>)>()})
        })'''
    else:
        body = 'serde_json::json!({"indexPresent":false,"vectorPayloadBytes":0})'
    with cache.open("a") as stream:
        stream.write("\nimpl FoodCache { pub(crate) fn study_memory(&self)->serde_json::Value {" + body + "} }\n")
    with (source / "src/simulator.rs").open("a") as stream:
        stream.write('''
impl Simulator {
    pub(crate) fn study_memory(&self)->serde_json::Value {
        serde_json::json!({"foodCount":self.food.len(),"foodCapacity":self.food.capacity(),
            "foodItemBytes":std::mem::size_of::<Food>(),"index":self.food_cache.study_memory()})
    }
}
''')
    fixture = source / "src/client/simulation/study_small.rs"
    edit(fixture, "    drop(record); output.flush().unwrap();", '''    if let Ok(path)=std::env::var("STUDY_MEMORY_JSON") {
        std::fs::write(path,serde_json::to_vec(&sim.backend.world.study_memory()).unwrap()).unwrap();
    }
    drop(record); output.flush().unwrap();''')


def trace():
    out = ROOT / "traces"
    out.mkdir()
    selected = {}
    for case in CASES:
        records = {}
        for arm in ARMS:
            path = out / f"{case}-{arm}.gz"
            with (out / f"{case}-{arm}.log").open("w") as err:
                child = subprocess.Popen([binary(arm), case, "--trace"], env=ENV, stdout=subprocess.PIPE, stderr=err)
                digest = hashlib.sha256()
                size = 0
                with gzip.open(path, "wb", compresslevel=1) as stream:
                    while block := child.stdout.read(1 << 20):
                        digest.update(block)
                        size += len(block)
                        stream.write(block)
                require(child.wait() == 0, f"trace failed: {case}/{arm}")
            summary = json.loads((out / f"{case}-{arm}.log").read_text().splitlines()[-1])
            records[arm] = {"sha256": digest.hexdigest(), "bytes": size, "summary": summary}
        require(records["base"] == records["candidate"], f"trace mismatch: {case}")
        with gzip.open(out / f"{case}-base.gz", "rb") as left, gzip.open(out / f"{case}-candidate.gz", "rb") as right:
            while True:
                x, y = left.read(1 << 20), right.read(1 << 20)
                require(x == y, f"trace bytes differ: {case}")
                if not x:
                    break
        selected[case] = {"equal": True, **records["base"]}
        (out / "selected.json").write_text(json.dumps(selected, indent=2) + "\n")
        print("TRACE", case, flush=True)


def count(idle=False):
    out = ROOT / ("idle" if idle else "instructions")
    out.mkdir()
    selected = {}
    for case in ([0, 1, 2, 5, 10] if idle else CASES):
        rows = []
        for pair in range(-1, 5):
            row = {"pair": pair}
            for arm in (ARMS if pair < 0 or pair % 2 == 0 else ARMS[::-1]):
                prefix = out / f"{case}-{pair}-{arm}"
                run([ROOT / "pmu", prefix.with_suffix(".json"), binary(arm, "study_idle" if idle else "study_small"), case], prefix.with_suffix(".log"))
                row[arm] = counter(prefix.with_suffix(".json"))
                summary = json.loads(prefix.with_suffix(".log").read_text().splitlines()[-1])
                if idle:
                    require(summary == {"sockets": case, "passes": 4000}, "idle workload mismatch")
                else:
                    expected = json.loads((ROOT / "traces/selected.json").read_text())[case]["summary"]
                    require(summary == expected, "counted workload mismatch")
            rows.append(row)
        selected[case] = paired(rows)
        (out / "selected.json").write_text(json.dumps(selected, indent=2) + "\n")
        print("COUNT", "idle" if idle else "fixture", case, selected[case]["ratioMedian"], flush=True)


def ticks():
    out = ROOT / "ticks"
    out.mkdir()
    selected = {}
    for case in CASES:
        selected[case] = {}
        for arm in ARMS:
            path = out / f"{case}-{arm}.json"
            run([binary(arm), case], out / f"{case}-{arm}.log", extra={"STUDY_TICK_COUNTS": str(path)})
            rows = json.loads(path.read_text())
            require(len(rows) == 1080, "missing ticks")
            require(all(r[2] > 0 and r[3] / r[2] >= .99 for r in rows), "tick counter scheduling below 99%")
            selected[case][arm] = {"all": quantiles([r[1] for r in rows]), "postWarmup": quantiles([r[1] for r in rows if r[0] >= 120]), "minimumScheduling": min(r[3] / r[2] for r in rows)}
        (out / "selected.json").write_text(json.dumps(selected, indent=2) + "\n")
        print("TICKS", case, flush=True)


def memory():
    out = ROOT / "memory"
    out.mkdir()
    selected = {}
    for case in CASES:
        selected[case] = {}
        for arm in ARMS:
            path = out / f"{case}-{arm}.json"
            run([binary(arm), case], out / f"{case}-{arm}.log", extra={"STUDY_MEMORY_JSON": str(path)})
            selected[case][arm] = json.loads(path.read_text())
        (out / "selected.json").write_text(json.dumps(selected, indent=2) + "\n")
        print("MEMORY", case, flush=True)


def controls():
    """Research-only ablations; immutable production binaries remain untouched."""
    out = ROOT / "controls"
    out.mkdir()
    source = out / "source"
    shutil.copytree(ROOT / "candidate/source", source)
    server = source / "src/client/server.rs"
    text = server.read_text()
    start = text.index('        #[cfg(target_os = "linux")]\n        {\n            use std::os::fd::AsRawFd;')
    end = text.index("        for (index, session) in self.sessions.iter_mut().enumerate() {", start)
    text = text[:start] + text[end:]
    start = text.index('            #[cfg(target_os = "linux")]\n            let readable = self.readiness.readable(index);')
    end = text.index("            session.service(", start)
    text = text[:start] + "            let _ = index;\n            let readable = true;\n" + text[end:]
    server.write_text(text, newline="\n")
    # Move only the lazy pellet-index build before the measured first tick.
    # It stays in process setup, not outside the fixture's total work.
    with (source / "src/simulator.rs").open("a") as stream:
        stream.write("\nimpl Simulator {pub(crate) fn study_warm_pellets(&mut self){let _=self.food_cache.pellets(&self.food);}}\n")
    edit(source / "src/client/simulation/study_small.rs", "    let mut tick_counter = tick_counter::Counter::from_env();", "    sim.backend.world.study_warm_pellets();\n    let mut tick_counter = tick_counter::Counter::from_env();")
    target = ROOT / "control-target"
    # Task-owned existing dependency cache; original executable copies are pinned.
    if ARGS.target_root:
        target = (ARGS.target_root / "candidate/target").resolve()
    run(["cargo", "build", "--release", "--locked", "--example", "study_idle", "--example", "study_small"], out / "build.log", source, {"CARGO_TARGET_DIR": str(target)})
    for name in ["study_idle", "study_small"]:
        shutil.copy2(target / "release/examples" / name, out / name)
    result = {"description": "main without readiness snapshot for idle; main with pellet index built in setup for tick attribution", "binaries": {n: sha(out / n) for n in ["study_idle", "study_small"]}}
    rows = []
    for pair in range(-1, 5):
        row = {"pair": pair}
        for arm in (ARMS if pair < 0 or pair % 2 == 0 else ARMS[::-1]):
            prefix = out / f"idle0-{pair}-{arm}"
            executable = binary("candidate", "study_idle") if arm == "base" else out / "study_idle"
            run([ROOT / "pmu", prefix.with_suffix(".json"), executable, "0"], prefix.with_suffix(".log"))
            row[arm] = counter(prefix.with_suffix(".json"))
        rows.append(row)
    result["idle0ReadinessOffVersusMain"] = paired(rows)
    result["warmIndexTicks"] = {}
    for case in CASES[:4]:
        path = out / f"{case}.ticks.json"
        run([out / "study_small", case], out / f"{case}.log", extra={"STUDY_TICK_COUNTS": str(path)})
        rows = json.loads(path.read_text())
        require(len(rows) == 1080 and all(r[2] > 0 and r[3] / r[2] >= .99 for r in rows), "control tick counter gate")
        result["warmIndexTicks"][case] = {"all": quantiles([r[1] for r in rows]), "firstTick": rows[0][1]}
    # Exact output parity is required for the ablation that changes setup.
    path = out / "official-10.trace.gz"
    with (out / "trace.log").open("w") as err:
        child = subprocess.Popen([out / "study_small", "official-10", "--trace"], env=ENV, stdout=subprocess.PIPE, stderr=err)
        with gzip.open(path, "wb", compresslevel=1) as stream:
            shutil.copyfileobj(child.stdout, stream)
        require(child.wait() == 0, "control trace failed")
    with gzip.open(path, "rb") as left, gzip.open(ROOT / "traces/official-10-candidate.gz", "rb") as right:
        while True:
            x, y = left.read(1 << 20), right.read(1 << 20)
            require(x == y, "prebuilt index changed output")
            if not x:
                break
    result["official10TraceEqual"] = True
    (out / "selected.json").write_text(json.dumps(result, indent=2) + "\n")
    print("CONTROLS", json.dumps(result), flush=True)


def empty_control():
    """Separate the remaining #208 empty-pass bookkeeping after readiness."""
    out = ROOT / "empty-control"
    out.mkdir()
    source = out / "source"
    shutil.copytree(ROOT / "controls/source", source)
    edit(source / "src/client/server.rs", "        self.retain_sessions();", "        self.sessions.retain(|session| !matches!(session.stage, Stage::Closed));")
    target = (ARGS.target_root / "candidate/target").resolve() if ARGS.target_root else ROOT / "control-target"
    run(["cargo", "build", "--release", "--locked", "--example", "study_idle"], out / "build.log", source, {"CARGO_TARGET_DIR": str(target)})
    shutil.copy2(target / "release/examples/study_idle", out / "study_idle")
    rows = []
    for pair in range(-1, 5):
        row = {"pair": pair}
        for arm in (ARMS if pair < 0 or pair % 2 == 0 else ARMS[::-1]):
            prefix = out / f"idle0-{pair}-{arm}"
            executable = ROOT / "controls/study_idle" if arm == "base" else out / "study_idle"
            run([ROOT / "pmu", prefix.with_suffix(".json"), executable, "0"], prefix.with_suffix(".log"))
            row[arm] = counter(prefix.with_suffix(".json"))
        rows.append(row)
    result = {"description": "both readiness and delivery-index maintenance off versus readiness off; zero sockets only", "binarySha256": sha(out / "study_idle"), "comparison": paired(rows)}
    (out / "selected.json").write_text(json.dumps(result, indent=2) + "\n")
    print("EMPTY CONTROL", json.dumps(result), flush=True)


def latency():
    out = ROOT / "latency"
    out.mkdir()
    selected = {}
    for case, n, legacy in [("stock10", 10, "none"), ("stock50", 50, "none"), ("legacy10", 10, "all")]:
        rows = []
        for pair in range(3):
            row = {"pair": pair}
            for arm in (ARMS if pair % 2 == 0 else ARMS[::-1]):
                d = out / f"{case}-{pair}-{arm}"
                d.mkdir()
                warmup, hold = 10, 30
                port = ARGS.port
                config = {"bind": f"127.0.0.1:{port}", "seed": 7, "players": {"max": n}, "bots": {"count": 0, "threads": 0}}
                (d / "config.json").write_text(json.dumps(config) + "\n")
                epoch = time.time() + 2
                extra = {"STUDY_TICKS_JSON": str(d / "ticks.json"), "STUDY_INPUT_PREFIX": str(d / "input"), "STUDY_LEGACY": legacy, "STUDY_SERVER_IP": "127.0.0.1", "STUDY_EPOCH_UNIX_S": str(epoch)}
                children = []
                with (d / "host.log").open("w") as hostlog, (d / "load.log").open("w") as loadlog:
                    try:
                        host = subprocess.Popen(["taskset", "-c", str(ARGS.cpu), binary(arm, "study_host"), str(d / "config.json"), str(warmup), str(hold), str(d / "host.json"), str(epoch)], env=dict(ENV, **extra), stdout=hostlog, stderr=subprocess.STDOUT)
                        children.append(host)
                        time.sleep(.3)
                        require(host.poll() is None, "host exited before load")
                        load = subprocess.Popen(["taskset", "-c", str(ARGS.load_cpu), binary("candidate", "host_capacity_load"), str(n), str(port), str(hold), str(warmup), str(d / "load.json"), "0"], env=dict(ENV, **extra), stdout=loadlog, stderr=subprocess.STDOUT)
                        children.append(load)
                        for child in children:
                            require(child.wait(timeout=warmup + hold + 30) == 0, "loopback child failed")
                    finally:
                        for child in children:
                            if child.poll() is None:
                                child.terminate()
                                child.wait(timeout=10)
                h = json.loads((d / "host.json").read_text())
                load = json.loads((d / "load.json").read_text())
                require(not load["errors"], f"generator errors: {load['errors']}")
                origin, tickrows = json.loads((d / "ticks.json").read_text())
                busy = [r[1] / 1e6 for r in tickrows if h["holdStartUnix"] <= origin + r[0] / 1e9 < h["holdEndUnix"]]
                groups = {}
                with (d / "load.frames.csv").open() as stream:
                    for r in csv.DictReader(stream):
                        groups.setdefault((r["client"], r["connection"]), []).append(float(r["receipt_ms"]) - float(r["server_ms"]))
                excess = [v - min(values) for values in groups.values() for v in values]
                boundaries = [r for r in h["rows"] if r["boundary"] in ["start", "end"]]
                require(len(boundaries) == 2, "missing hold boundaries")
                row[arm] = {"tickBusyMs": quantiles(busy), "frameExcessMs": quantiles(excess), "deferred": boundaries[1]["deferred"] - boundaries[0]["deferred"], "clients": load["clients"], "generatorBusyPercent": 100 * load["generator"]["busySecondsInWindow"] / hold, "traffic": load["traffic"]}
                sample_epoch, samples = json.loads((d / "load.inputs.json").read_text())
                events = [e for path in d.glob("input-slither-*.json") for e in json.loads(path.read_text())]
                spec = importlib.util.spec_from_file_location("input_reduce", REPO / "research/performance/2026-10-27-host-tick-handoff/analyze_inputs.py")
                module = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(module)
                inputs = module.match_input([(i,c,b,(sample_epoch+t)/1000) for i,c,b,t in samples], [e for e in events if e[0]==0], [e for e in events if e[0]==1], [origin+r[0]/1e9 for r in tickrows])
                require(inputs["sentSamples"] > 0 and inputs["matchedSamples"] == inputs["sentSamples"], "unmatched sampled inputs")
                row[arm]["input"] = inputs
                print("HOLD", case, pair, arm, row[arm]["tickBusyMs"], row[arm]["frameExcessMs"], flush=True)
            rows.append(row)
            selected[case] = rows
            (out / "selected.json").write_text(json.dumps(selected, indent=2) + "\n")


def select():
    shared = ["2026-10-27-host-snake-candidates/protocol_fixture.rs", "2026-10-27-host-snake-candidates/tick_counter.rs", "2026-10-26-host-capacity/host_capacity_load.rs", "2026-10-26-host-capacity/host_probe.rs", "2026-10-26-host-known-slots/host-players.patch", "2026-10-27-host-socket-readiness/idle.rs", "2026-10-27-host-socket-readiness/load_overlay.py", "2026-10-27-host-tick-handoff/instrument.py", "2026-10-27-host-tick-handoff/analyze_inputs.py"]
    result = {"schema": "small-server-check-v1", "pins": {arm: json.loads((ROOT / arm / "pins.json").read_text()) for arm in ARMS}, "harness": {p.name: sha(p) for p in STUDY.iterdir() if p.suffix in [".py", ".c"]}, "sharedHarness": {name: sha(REPO / "research/performance" / name) for name in shared}, "pmuBinarySha256": sha(ROOT / "pmu")}
    for name in ["traces", "instructions", "ticks", "idle", "memory", "latency", "controls", "empty-control"]:
        result[name] = json.loads((ROOT / name / "selected.json").read_text())
    for name in ["traces", "instructions", "ticks", "memory"]:
        require(set(result[name]) == set(CASES), f"incomplete {name} cases")
    for name in ["instructions", "idle"]:
        for value in result[name].values():
            require([r["pair"] for r in value["pairs"]] == list(range(-1, 5)), f"incomplete {name} pairs")
    require(set(result["latency"]) == {"stock10", "stock50", "legacy10"}, "incomplete socket cases")
    require(all([r["pair"] for r in rows] == [0, 1, 2] for rows in result["latency"].values()), "incomplete socket pairs")
    for case, arms in result["ticks"].items():
        for arm, value in arms.items():
            rows = json.loads((ROOT / "ticks" / f"{case}-{arm}.json").read_text())
            value["maximumTick"] = max(rows, key=lambda r: r[1])[0]
            value["firstTick"] = rows[0][1]
    (ROOT / "selected-results.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["build", "trace", "count", "ticks", "idle", "memory", "latency", "controls", "empty-control", "select"])
    parser.add_argument("repository", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--cpu", type=int, required=True)
    parser.add_argument("--load-cpu", type=int, required=True)
    parser.add_argument("--port", type=int, default=18779)
    parser.add_argument("--base", default=BASE)
    parser.add_argument("--candidate", default=CANDIDATE)
    parser.add_argument("--archives", type=Path, help="Optional base.tar/candidate.tar, Git archive commit headers verified")
    parser.add_argument("--target-root", type=Path, help="Optional task-owned base/target and candidate/target Cargo caches")
    ARGS = parser.parse_args()
    ROOT, REPO = ARGS.output.resolve(), ARGS.repository.resolve()
    require(ROOT != REPO and REPO not in ROOT.parents, "raw output must be outside Git checkout")
    os.sched_setaffinity(0, {ARGS.cpu})
    ENV = dict(os.environ, CARGO_BUILD_JOBS="1", RAYON_NUM_THREADS="1", RUST_TEST_THREADS="1", RUSTUP_TOOLCHAIN="1.99.0", CARGO_PROFILE_RELEASE_DEBUG="1")
    ENV.pop("STUDY_TICK_COUNTS", None)
    if ARGS.mode != "build":
        rehash()
    {"build": build, "trace": trace, "count": count, "ticks": ticks, "idle": lambda: count(True), "memory": memory, "latency": latency, "controls": controls, "empty-control": empty_control, "select": select}[ARGS.mode]()
    if ARGS.mode != "build":
        rehash()
