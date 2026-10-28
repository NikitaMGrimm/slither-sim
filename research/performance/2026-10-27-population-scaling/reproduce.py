"""Linux x86-64 offline study. Source modifications exist only in archived copies.

python3 reproduce.py build REPOSITORY NEW_OUTPUT --cpu 6
python3 reproduce.py warm REPOSITORY OUTPUT --case 500-common
python3 reproduce.py count REPOSITORY OUTPUT --case 500-common
python3 reproduce.py profile REPOSITORY OUTPUT --case 500-common
python3 reproduce.py bots REPOSITORY OUTPUT
Every invocation is affinity-limited to one logical CPU; Cargo/Rayon use one.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import tarfile

STUDY = Path(__file__).resolve().parent
RUNTIME = "ef866d73259e35fe8659a1996d7b97c36d511e0b"
# The maintaining-agent-censored 10k control runs only through cliff_probe.py's CPU cap.
CASES = ["500-common", "1000-common", "2000-common", "5000-capped", "5000-uncapped", "10000-capped"]


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def swap(path, old, new):
    source = path.read_text()
    require(source.count(old) == 1, f"ambiguous overlay anchor in {path}: {old!r}")
    path.write_text(source.replace(old, new), newline="\n")


def run(command, log, cwd=None, build=False, original_import=False, prepared_admission=False):
    cpus = str(ARGS.cpu)
    def memory_limit():
        ceiling = 5 * 1024**3 // 2
        resource.setrlimit(resource.RLIMIT_AS, (ceiling, ceiling))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    env = dict(os.environ, RAYON_NUM_THREADS="1", CARGO_BUILD_JOBS="1", RUST_TEST_THREADS="1",
               RUSTFLAGS="", CARGO_ENCODED_RUSTFLAGS="")
    env.pop("SLITHER_STUDY_ORIGINAL_IMPORT", None)
    env.pop("SCALING_FAST_ADMISSION", None)
    env.pop("SCALING_ORIGINAL_WARM", None)
    if prepared_admission:
        env["SCALING_FAST_ADMISSION"] = "1"
    if original_import:
        env["SLITHER_STUDY_ORIGINAL_IMPORT"] = "1"
    with log.open("x") as stream:
        subprocess.run(["taskset", "-c", cpus, *map(str, command)], cwd=cwd,
                       env=env,
                       stdout=stream, stderr=subprocess.STDOUT, check=True, preexec_fn=memory_limit)


def checkpoint_preparation(source):
    """Keep every import check; shorten only the temporary JSON tree's lifetime.

    The production method is untouched. This study-only copy also verifies a
    complete checkpoint round trip before any cache prefix or counted tick.
    """
    path = source / "src/simulator/checkpoint.rs"
    text = path.read_text()
    original = text.split("    pub fn from_checkpoint(", 1)[1].split("\n    /// Build and validate a complete candidate", 1)[0]
    original = "    pub fn from_checkpoint(" + original
    method = original.replace("pub fn from_checkpoint(", "pub(crate) fn study_from_checkpoint(")
    require(method.count("let raw: Value = decode(bytes)?;") == 1, "checkpoint tree anchor")
    method = method.replace("let raw: Value = decode(bytes)?;", '''if std::env::var_os("SLITHER_STUDY_ORIGINAL_IMPORT").is_some() {
            return Self::from_checkpoint(bytes, options);
        }
        let mut raw: Value = study_checkpoint_shape(bytes)?;''')
    anchor = "        let envelope: Envelope = decode(bytes)?;"
    require(method.count(anchor) == 1, "checkpoint typed anchor")
    method = method.replace(anchor, '        let raw_config = raw.get_mut("config").unwrap().take();\n        drop(raw);\n' + anchor)
    method = method.replace('&raw["config"]', '&raw_config')
    anchor = "        envelope.state.into_world(envelope.config, options)"
    require(method.count(anchor) == 1, "checkpoint constructor anchor")
    method = method.replace(anchor, '''        let producer = envelope.producer.clone();
        let world = envelope.state.into_world(envelope.config, options)?;
        drop(raw_config);
        let mut roundtrip = world.checkpoint()?;
        roundtrip.0.producer = producer;
        let encoded = serde_json::to_vec(&roundtrip).unwrap();
        require(encoded == bytes, Constraint::InconsistentState, "study.checkpointRoundTrip")?;
        Ok(world)''')
    shape = (STUDY / "checkpoint_shape.rs").read_text()
    path.write_text(text + "\n" + shape + "\n// Research preparation only; the production importer above is unchanged.\nimpl Simulator {\n" + method + "\n}\n", newline="\n")
    module = source / "src/client/simulation/study_scaling.rs"
    text = module.read_text()
    require(text.count("Simulator::from_checkpoint(") == 2, "checkpoint harness callers")
    module.write_text(text.replace("Simulator::from_checkpoint(", "Simulator::study_from_checkpoint("), newline="\n")
    return {"sourceMethodSha256": hashlib.sha256(original.encode()).hexdigest(),
            "studyMethodSha256": hashlib.sha256(method.encode()).hexdigest(),
            "shapeSourceSha256": sha(STUDY / "checkpoint_shape.rs"),
            "fullValidation": True, "fullCheckpointRoundTrip": True, "originalImporterControl": True,
            "note": "Producer metadata is restored in the comparison copy; authoritative state/configuration and all encoded bytes must match."}


def reuse_prepared(donor):
    require(not (donor / "original-references.json").exists(),
            "reuse requires an original-search donor; prepared-admission counts cannot be relabelled as references")
    old = json.loads((donor / "pins.json").read_text())
    new = json.loads((ROOT / "pins.json").read_text())
    require(old["runtime"] == new["runtime"] and old["archiveSha256"] == new["archiveSha256"], "fixture runtime drift")
    for name in ["counter.rs", "bots.rs"]:
        require(old["harness"][name] == new["harness"][name], "fixture/controller drift")
    bounds = (STUDY / "warm_bounds.rs").read_text()
    if old["harness"]["warm_bounds.rs"] != new["harness"]["warm_bounds.rs"]:
        bounds = bounds.replace("// Research preparation acceleration of the copied host admission predicates.\n// Original admission references keep the original search. No physics is changed.",
            "// Warmup-only acceleration of the copied host admission predicates.\n// Counted windows always call the original host search. No physics is changed.")
        require(hashlib.sha256(bounds.encode()).hexdigest() == old["harness"]["warm_bounds.rs"], "admission preparer drift")
    text = (STUDY / "physics.rs").read_text().replace(
        '((!count || std::env::var_os("SCALING_FAST_ADMISSION").is_some()) &&', '(!count &&')
    radius_amendment = old["harness"]["physics.rs"] != hashlib.sha256(text.encode()).hexdigest()
    if radius_amendment:
        # Reconstruct the exact earlier template: only event timestamps and
        # their uncounted plan-export helper changed. No physics/driver edits.
        start = text.index("fn capped_radius_updates()")
        end = text.index("fn save(", start)
        text = text[:start] + text[end:]
        text = text.replace('    if mode=="radius-plan" {\n        fs::write(root,serde_json::to_vec(&capped_radius_updates()).unwrap()).unwrap();\n        return;\n    }\n', '')
        text = text.replace("config.radius_updates=capped_radius_updates();",
                            "config.radius_updates=(0..=260000).map(|t|crate::RadiusUpdate {time:t as f64*config.dt,radius:r}).collect();")
        require(hashlib.sha256(text.encode()).hexdigest() == old["harness"]["physics.rs"],
                "reuse changed more than radius event timing")
    manifest = {"source": str(donor), "runtime": old["runtime"], "warmups": {}}
    if (donor / "reused-inputs.json").is_file():
        manifest["priorReuse"] = json.loads((donor / "reused-inputs.json").read_text())
    for case in ["500-common", "5000-uncapped", "10000-capped"]:
        require(json.loads((donor / case / "summary.json").read_text())["settled"], "incomplete donor warmup")
        shutil.copytree(donor / case, ROOT / case, copy_function=os.link)
        if radius_amendment and case == "10000-capped":
            manifest["radiusEventAmendment"] = retime_capped_checkpoint(ROOT / case,
                json.loads((ROOT / case / "summary.json").read_text())["ticks"] + 1100)
        manifest["warmups"][case] = {name: sha(ROOT / case / name) for name in ["world.json", "driver.json", "warmup.json", "summary.json"]}
    for name in ["warmup-oracles.json", "environment.json", "environment-one-cpu.json", "uncapped-status.json"]:
        os.link(donor / name, ROOT / name)
    shutil.copytree(donor / "cliff-probe", ROOT / "cliff-probe", copy_function=os.link)
    bot_pins = donor / "bot-pins.json" if (donor / "bot-pins.json").is_file() else donor / "pins.json"
    os.link(bot_pins, ROOT / "bot-pins.json")
    bot_binaries = donor / "bot-binaries" if (donor / "bot-binaries").is_dir() else donor
    for arm in ["plain", "profile"]:
        dest = ROOT / "bot-binaries" / arm
        dest.mkdir(parents=True)
        os.link(bot_binaries / arm / "study_bots", dest / "study_bots")
    for repeat in ["plain", -1, 0, 1, 2, 3, 4]:
        name = f"bots-{repeat}"
        shutil.copytree(donor / name, ROOT / name, copy_function=os.link)
        os.link(donor / f"{name}.log", ROOT / f"{name}.log")
    dest = ROOT / "public-import-oracle"
    dest.mkdir()
    retained_oracle = donor / "public-import-oracle"
    oracle_input = retained_oracle if retained_oracle.is_dir() else donor / "500-common-count-0"
    for name in ["world.json", "driver.json", "rows.json"]:
        os.link(oracle_input / name, dest / name)
    source = (json.loads((retained_oracle / "source.json").read_text()) if retained_oracle.is_dir() else
              {"runtime": old["runtime"], "source": str(oracle_input),
               "binarySha256": old["arms"]["plain"]["binaries"]["study_scaling"],
               "rowsSha256": sha(dest / "rows.json"), "importer": "original production public API"})
    (dest / "source.json").write_text(json.dumps(source, indent=2) + "\n")
    (ROOT / "reused-inputs.json").write_text(json.dumps(manifest, indent=2) + "\n")
    references = {"origins": {}, "cases": {}}
    origin = retain_reference_binary(references, old, donor / "plain/study_scaling")
    for case, repeats in [("500-common", range(5)), ("5000-uncapped", range(5)), ("10000-capped", [-1])]:
        references["cases"][case] = []
        for index, repeat in enumerate(repeats):
            source = donor / f"{case}-count-{repeat}"
            require((source / "rows.json").is_file(), "complete original admission reference missing")
            dest = ROOT / f"{case}-original-{index}"
            shutil.copytree(source, dest, copy_function=os.link)
            os.link(donor / f"{case}-count-{repeat}.log", ROOT / f"{case}-original-{index}.log")
            references["cases"][case].append({"folder": dest.name, "origin": origin,
                "source": str(source), "sourceRepeat": repeat})
    (ROOT / "original-references.json").write_text(json.dumps(references, indent=2) + "\n")


def retain_reference_binary(references, pins, source):
    digest = pins["arms"]["plain"]["binaries"]["study_scaling"]
    require(sha(source) == digest, "original admission binary drift")
    binaries = ROOT / "reference-binaries"
    binaries.mkdir(exist_ok=True)
    dest = binaries / digest
    if not dest.exists():
        os.link(source, dest)
    references["origins"][digest] = {"pins": pins, "binary": str(dest.relative_to(ROOT)),
        "admission": "complete original host search; no fast-admission environment"}
    return digest


def retime_capped_checkpoint(folder, through_tick):
    plan = ROOT / "capped-radius-plan.json"
    run([ROOT / "plain/study_scaling", "radius-plan", plan], ROOT / "radius-plan.log")
    # Only the small radius array is decoded. Stream the potentially large food
    # state unchanged, and never mutate the donor's hard-linked checkpoint.
    path = folder / "world.json"
    prior_hash = sha(path)
    temporary = folder / "world.retimed.json"
    marker = b'"radiusUpdates":'
    with path.open("rb") as source, temporary.open("xb") as dest:
        prefix = bytearray()
        while marker not in prefix:
            block = source.read(65536)
            require(bool(block) and len(prefix) < 1024*1024, "radius configuration header missing")
            prefix.extend(block)
        offset = prefix.index(marker) + len(marker)
        require(prefix[offset:offset+1] == b"[", "radius plan is not an array")
        source.seek(offset)
        previous = bytearray()
        while b"]" not in previous:
            block = source.read(65536)
            require(bool(block) and len(previous) < 32*1024*1024, "radius array end missing")
            previous.extend(block)
        previous = previous[:previous.index(b"]")+1]
        old_updates, new_updates = json.loads(previous), json.loads(plan.read_bytes())
        require(len(old_updates) == len(new_updates) == 260001, "radius plan extent drift")
        require(all(a["radius"] == b["radius"] == 32000 and a["time"] == i*.007
                    for i, (a,b) in enumerate(zip(old_updates,new_updates))), "unexpected donor radius plan")
        validate_radius_events(old_updates, through_tick)
        validate_radius_events(new_updates, 241100)
        dest.write(prefix[:offset])
        dest.write(plan.read_bytes())
        source.seek(offset+len(previous))
        unchanged = hashlib.sha256()
        for block in iter(lambda: source.read(1024*1024), b""):
            unchanged.update(block)
            dest.write(block)
    os.replace(temporary, path)
    return {"sourceSha256": prior_hash, "retimedSha256": sha(path),
            "sameEventIndicesThroughTick": through_tick,
            "newPlanValidatedThroughTick": 241100,
            "unchangedCheckpointSuffixSha256": unchanged.hexdigest(),
            "note": "Only radius-update timestamps changed; complete event-consumption equality was checked through the counted endpoint."}


def validate_radius_events(updates, through_tick):
    clock, index = 0., 1
    for tick in range(1, through_tick+1):
        clock += .007
        while index < len(updates) and updates[index]["time"] <= clock+1e-9:
            index += 1
        require(index == tick+1, f"radius event timing failed at tick {tick}")


def build():
    ROOT.mkdir(parents=True, exist_ok=False)
    archive = ROOT / "runtime.tar"
    subprocess.run(["git", "-C", ARGS.repository, "archive", RUNTIME, "-o", str(archive)], check=True)
    with tarfile.open(archive) as stream:
        originals = {m.name: stream.extractfile(m).read() for m in stream.getmembers() if m.isfile() and m.name.endswith(".rs")}
    pins = {"runtime": subprocess.check_output(["git", "-C", ARGS.repository, "rev-parse", RUNTIME], text=True).strip(),
            "archiveSha256": sha(archive), "harness": {p.name: sha(p) for p in STUDY.iterdir() if p.suffix in {".py", ".rs", ".md"}},
            "cpu": ARGS.cpu, "perfEventParanoid": Path("/proc/sys/kernel/perf_event_paranoid").read_text().strip(),
            "buildFlags": {"rustflags": [], "profile": "release", "debug": 1, "lto": "thin", "codegenUnits": 1},
            "rustc": subprocess.check_output([shutil.which("rustc") or str(Path.home() / ".cargo/bin/rustc"), "+1.99.0", "-Vv"], text=True), "arms": {}}
    for arm in ["plain", "profile"]:
        source = ROOT / arm / "source"
        source.mkdir(parents=True)
        with tarfile.open(archive) as stream:
            stream.extractall(source, filter="data")
        original = (source / "src/client/simulation.rs").read_text()
        pose = original.split("    fn spawn_pose_waited(", 1)[1].split("\n    /// Drive non-controlled", 1)[0]
        # Keep the entire candidate search, safety predicates and fallback intact.
        pose = "fn host_pose(world: &Simulator, random: &mut Rng, slot: usize, waited:" + pose.split("waited:", 1)[1]
        pose = pose.replace("self.world", "world").replace("self.spawn_random", "random").replace("self\n                .world", "world").replace("self\n                    .world", "world")
        require("self" not in pose, "unprojected host admission receiver")
        fast = pose.replace("fn host_pose(", "fn host_pose_fast(").replace("waited: f64)", "waited: f64, bounds: &[Option<crate::physics::Bounds>])")
        fast = fast.replace("world\n                    .body_is_clear_excluding(&body, spawn::CLEARANCE, Some(slot))", "warm_clear(world, bounds, &body, spawn::CLEARANCE, Some(slot))")
        fast = fast.replace("world\n                .spawn_margin_bounded_excluding(&body, best, Some(slot))", "warm_margin(world, bounds, &body, best, Some(slot))")
        require("warm_clear(" in fast and "warm_margin(" in fast, "warmup adapter anchors")
        (source / "src/client/simulation/study_scaling.rs").write_text((STUDY / "physics.rs").read_text() + "\n" + (STUDY / "warm_bounds.rs").read_text() + "\n" + pose + "\n" + fast, newline="\n")
        (source / "src/client/simulation/study_bots.rs").write_text((STUDY / "bots.rs").read_text(), newline="\n")
        (source / "src/study_counter.rs").write_text((STUDY / "counter.rs").read_text(), newline="\n")
        with (source / "src/lib.rs").open("a") as stream:
            stream.write("\npub mod study_counter;\n")
        with (source / "src/client/simulation.rs").open("a") as stream:
            stream.write("\npub mod study_scaling;\npub mod study_bots;\n")
        (source / "examples/study_scaling.rs").write_text("fn main(){slither_sim::client::simulation::study_scaling::run();}\n")
        (source / "examples/study_bots.rs").write_text("fn main(){slither_sim::client::simulation::study_bots::run();}\n")
        (source / "examples/study_checkpoint_checks.rs").write_text('fn main(){println!("{}",slither_sim::Simulator::study_checkpoint_checks());}\n')
        preparation = checkpoint_preparation(source)
        if "checkpointPreparation" in pins:
            require(pins["checkpointPreparation"] == preparation, "arm preparation drift")
        pins["checkpointPreparation"] = preparation
        if arm == "profile":
            instrument(source)
        target = Path(os.environ.get("SCALING_TARGET_ROOT", ROOT)) / arm / "target"
        run([shutil.which("cargo") or Path.home() / ".cargo/bin/cargo", "+1.99.0", "build", "--offline", "--release", "--locked", "-j", "1", "--config", 'profile.release.debug=1',
             "--target-dir", target, "--example", "study_scaling", "--example", "study_bots", "--example", "study_checkpoint_checks"], ROOT / f"{arm}-build.log", source, build=True)
        binary_pins = {}
        for name in ["study_scaling", "study_bots", "study_checkpoint_checks"]:
            dest = ROOT / arm / name
            shutil.copy2(target / "release/examples" / name, dest)
            binary_pins[name] = sha(dest)
        pins["arms"][arm] = {"binaries": binary_pins, "overlay": {str(p.relative_to(source)): sha(p) for p in source.rglob("*.rs") if p.read_bytes() != originals.get(p.relative_to(source).as_posix())}}
    (ROOT / "pins.json").write_text(json.dumps(pins, indent=2) + "\n")


def instrument(source):
    path = source / "src/simulator.rs"
    swap(path, "        let mut clock = if PROFILE", "        crate::study_counter::begin();\n        let mut clock = if PROFILE")
    for stage, anchor in [(0, "        // Stable-slot indexing"), (1, "        candidates.clear();\n        dead.clear();"),
                          (2, "        self.time += c.dt;"), (3, "        // Without food there is nothing"),
                          (4, "        self.advance_prey(previous, prey_buffers);"), (5, "        let dt = self.config.dt;\n        let target"),
                          (6, "        for (snake, kiosk) in self.snakes.iter_mut().zip(&self.kiosks)")]:
        swap(path, anchor, f"        crate::study_counter::mark({stage});\n" + anchor)
    # Rotating 1/64 sample includes every stable slot over 64 ticks. Broad query
    # and candidate bounds/exact tests stay separate; multiply sums by 64.
    swap(path, "            let indexed = index.as_ref().is_some_and(|(index, longest)| {", "            let sampled = i % 64 == ((self.time / self.config.dt).round() as usize % 64);\n            let query_start = sampled.then(crate::study_counter::start).flatten();\n            let indexed = index.as_ref().is_some_and(|(index, longest)| {")
    swap(path, "            let test = |j: usize", "            crate::study_counter::add(7, query_start);\n            let test_start = sampled.then(crate::study_counter::start).flatten();\n            let test = |j: usize")
    swap(path, "            if killer.is_some() {", "            crate::study_counter::add(8, test_start);\n            if killer.is_some() {")
    swap(path, "                let elapsed = state.clock + c.dt;", "                let boost_start = (i % 64 == ((self.time / c.dt).round() as usize % 64)).then(crate::study_counter::start).flatten();\n                let elapsed = state.clock + c.dt;")
    swap(path, "            } else {\n                state.clock = 0.0;", "                crate::study_counter::add(19, boost_start);\n            } else {\n                state.clock = 0.0;")
    path = source / "src/client/simulation/broadcast.rs"
    swap(path, "        self.broadcast_with(early, hand_over)?;", "        let broadcast_start = crate::study_counter::start();\n        self.broadcast_with(early, hand_over)?;\n        crate::study_counter::add(9, broadcast_start);")
    path = source / "src/client/simulation/connection.rs"
    swap(path, "                    s.state.receive_frame(&bytes, at)?;", "                    let start = crate::study_counter::start();\n                    s.state.receive_frame(&bytes, at)?;\n                    crate::study_counter::add(10, start);")
    swap(path, "            let bytes = group.frame(elapsed as u16)?;", "            let start = crate::study_counter::start();\n            let bytes = group.frame(elapsed as u16)?;\n            crate::study_counter::add(11, start);")
    path = source / "src/client/simulation.rs"
    swap(path, "                    let frame = self.frame_for(slot)?;", "                    let start = crate::study_counter::start();\n                    let frame = self.frame_for(slot)?;\n                    crate::study_counter::add(12, start);\n                    if let Some(obs) = &frame.observation {\n                        crate::study_counter::record(14, obs.opponents.len() as u64);\n                        crate::study_counter::record(15, obs.food.len() as u64);\n                        crate::study_counter::record(16, obs.prey.len() as u64);\n                    }\n                    let start = crate::study_counter::start();")
    swap(path, "                        self.send_to(slot, action)?;\n                    }", "                        crate::study_counter::add(13, start);\n                        self.send_to(slot, action)?;\n                    }")
    swap(path, "                    let _ = self.reconnect_slot(slot);", "                    let result = self.reconnect_slot(slot);\n                    crate::study_counter::record(17, 1);\n                    crate::study_counter::record(18, u64::from(result.is_err()));")


def main():
    if ARGS.mode == "build":
        build()
        return
    pins = json.loads((ROOT / "pins.json").read_text())
    for arm, info in pins["arms"].items():
        for name, digest in info["binaries"].items():
            require(sha(ROOT / arm / name) == digest, "binary identity drift")
    if ARGS.mode == "reuse":
        require(ARGS.prepared_from is not None, "supply --prepared-from")
        reuse_prepared(Path(ARGS.prepared_from).resolve())
        return
    if ARGS.mode == "cleanup":
        for arm in ["plain", "profile"]:
            target = (ROOT / arm / "target").resolve()
            require(ROOT in target.parents and target.name == "target", "cache outside study output")
            if target.exists():
                shutil.rmtree(target)
        return
    if ARGS.mode == "import-oracle":
        dest = ROOT / "public-import-oracle"
        run([ROOT / "plain/study_scaling", "count", ROOT / "500-common", dest],
            ROOT / "public-import-oracle.log", original_import=True)
        (dest / "source.json").write_text(json.dumps({"runtime": pins["runtime"],
            "binarySha256": pins["arms"]["plain"]["binaries"]["study_scaling"],
            "rowsSha256": sha(dest / "rows.json"), "importer": "original production public API"}, indent=2) + "\n")
        return
    if ARGS.mode == "preparation-checks":
        checks = {}
        for arm in ["plain", "profile"]:
            log = ROOT / f"checkpoint-checks-{arm}.log"
            run([ROOT / arm / "study_checkpoint_checks"], log)
            checks[arm] = json.loads(log.read_text().splitlines()[-1])
            require(checks[arm] == {"validRoundTrip": True, "originalImporterCompared": True, "rejectedShapes": 8},
                    "checkpoint preparation checks failed")
        (ROOT / "checkpoint-checks.json").write_text(json.dumps(checks, indent=2) + "\n")
        plan = ROOT / "capped-radius-plan.json"
        if not plan.is_file():
            run([ROOT / "plain/study_scaling", "radius-plan", plan], ROOT / "radius-plan.log")
        updates = json.loads(plan.read_bytes())
        require(len(updates) == 260001 and all(u["radius"] == 32000 for u in updates), "cap plan extent/radius drift")
        validate_radius_events(updates, 241100)
        (ROOT / "radius-plan-check.json").write_text(json.dumps({"validThroughTick": 241100,
            "radius": 32000, "planSha256": sha(plan)}, indent=2) + "\n")
        return
    if ARGS.mode == "reference":
        manifest_path = ROOT / "original-references.json"
        references = (json.loads(manifest_path.read_text()) if manifest_path.is_file()
                      else {"origins": {}, "cases": {}})
        if ARGS.case in references["cases"]:
            return
        if references["origins"]:
            require(len(references["origins"]) == 1, "original reference cohort is ambiguous")
            origin, info = next(iter(references["origins"].items()))
            binary = ROOT / info["binary"]
            require(sha(binary) == origin and info["pins"]["runtime"] == pins["runtime"], "reference cohort drift")
        else:
            origin = retain_reference_binary(references, pins, ROOT / "plain/study_scaling")
            binary = ROOT / "plain/study_scaling"
        dest = ROOT / f"{ARGS.case}-original-0"
        run([binary, "count", ROOT / ARGS.case, dest],
            ROOT / f"{dest.name}.log")
        references["cases"][ARGS.case] = [{"folder": dest.name, "origin": origin,
            "source": "fresh original-search reference", "sourceRepeat": None}]
        manifest_path.write_text(json.dumps(references, indent=2) + "\n")
        return
    if ARGS.mode == "bots":
        # Reuse only complete, validated bot components; collection stays serial.
        if all((ROOT / f"bots-{repeat}/world.json").exists() for repeat in range(-1, 5)):
            for repeat in range(-1, 5):
                dest = ROOT / f"bots-{repeat}"
                rows = json.loads((dest / "rows.json").read_text())
                require(len(rows) == 200 and [r["decision"] for r in rows] == list(range(1000, 1200)), "incomplete retained bot run")
                require((dest / "world.json").read_bytes() == (ROOT / "bots-plain/world.json").read_bytes(), "retained bot continuation changed")
            return
        run([ROOT / "plain/study_bots", ROOT / "bots-plain"], ROOT / "bots-plain.log")
        for repeat in range(-1, 5):
            dest = ROOT / f"bots-{repeat}"
            run([ROOT / "profile/study_bots", dest], ROOT / f"bots-{repeat}.log")
            require((dest / "world.json").read_bytes() == (ROOT / "bots-plain/world.json").read_bytes(), "bot diagnostic changes continuation")
        return
    require(ARGS.case in CASES, "choose a case")
    n, variant = ARGS.case.split("-")
    warm = ROOT / ARGS.case
    if ARGS.mode == "warm":
        run([ROOT / "plain/study_scaling", "warm", warm, n, variant], ROOT / f"{ARGS.case}-warm.log")
    elif ARGS.mode == "verify":
        run([ROOT / "plain/study_scaling", "verify", warm, ROOT / f"{ARGS.case}-oracle"], ROOT / f"{ARGS.case}-oracle.log")
    elif ARGS.mode == "count":
        for repeat in range(-1, 5):
            dest = ROOT / f"{ARGS.case}-count-{repeat}"
            run([ROOT / "plain/study_scaling", "count", warm, dest], ROOT / f"{ARGS.case}-count-{repeat}.log", prepared_admission=True)
    elif ARGS.mode == "profile":
        dest = ROOT / f"{ARGS.case}-profile"
        run([ROOT / "profile/study_scaling", "count", warm, dest], ROOT / f"{ARGS.case}-profile.log", prepared_admission=True)
        for name in ["world.json", "driver.json"]:
            require((dest / name).read_bytes() == (ROOT / f"{ARGS.case}-count-0" / name).read_bytes(), "diagnostic changes continuation")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["build", "warm", "verify", "reference", "count", "profile", "bots", "cleanup", "reuse", "import-oracle", "preparation-checks"])
    parser.add_argument("repository")
    parser.add_argument("output", type=Path)
    parser.add_argument("--cpu", type=int, default=6)
    parser.add_argument("--case", choices=CASES)
    parser.add_argument("--prepared-from", help="Retained original output; reuse byte-identical prepared inputs only")
    ARGS = parser.parse_args()
    os.sched_setaffinity(0, {ARGS.cpu})
    ROOT = ARGS.output.resolve()
    main()
