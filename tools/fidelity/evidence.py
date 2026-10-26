"""Freeze study plans, audit attempt accounting and package reproducible evidence."""

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import tarfile
import tempfile
import zlib

from jsonschema import Draft202012Validator

from capture_quality import audit_captures, sha256_string

ROOT = Path(__file__).resolve().parents[2]
MAX_METADATA = 4 * 1024 * 1024
MAX_FILE = 512 * 1024 * 1024
MAX_TOTAL = 2 * 1024 * 1024 * 1024
MAX_CAPTURE_TEXT = 256 * 1024 * 1024
MAX_FILES = 10000
# The original v1 packager's minimum, independent of the verifier's checkout.
V1_SOURCE_FLOOR = frozenset("""
Cargo.lock Cargo.toml LICENSE pyproject.toml uv.lock
examples/basic.rs examples/fidelity_replay.rs
schemas/attempt-event-v1.schema.json schemas/evidence-bundle-v1.schema.json
schemas/study-plan-v1.schema.json
src/boundary.rs src/kiosk.rs src/lib.rs src/main.rs src/model.rs
src/observation/mod.rs src/physics.rs src/prey.rs src/profiling.rs
src/simulator.rs src/spawn.rs src/transport.rs
tools/fidelity/analyze-capture-quality.py tools/fidelity/capture_quality.py
tools/fidelity/collect.py tools/fidelity/evidence.py
tools/fidelity/prepare-fidelity-replay.py tools/fidelity/slither-feeding-shape.mjs
tools/fidelity/slither-headless.mjs tools/fidelity/slither-paired-death.mjs
tools/fidelity/slither-paired-prey.mjs tools/fidelity/slither_protocol.py
tools/fidelity/worked_study.py
""".split())
RESERVED = {"con", "prn", "aux", "nul"} | {f"{name}{n}" for name in ("com", "lpt") for n in range(1, 10)}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def source_map_hash(files):
    return sha256(json.dumps(files, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8"))


def utc_now():
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _constant(value):
    raise ValueError(f"non-finite JSON number: {value}")


def read_json(path):
    if Path(path).stat().st_size > MAX_METADATA:
        raise ValueError(f"metadata exceeds {MAX_METADATA} bytes: {path}")
    data = Path(path).read_bytes()
    if len(data) > MAX_METADATA:
        raise ValueError(f"metadata exceeds {MAX_METADATA} bytes: {path}")
    return json.loads(data, object_pairs_hook=_pairs, parse_constant=_constant)


def write_json(path, value):
    with Path(path).open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())


def validate(value, schema_name):
    schema = read_json(ROOT / "schemas" / f"{schema_name}-v1.schema.json")
    Draft202012Validator.check_schema(schema)
    errors = sorted(Draft202012Validator(schema).iter_errors(value), key=lambda error: str(error.path))
    if errors:
        raise ValueError(f"{schema_name}: {errors[0].message}")
    for key in ("time", "createdAt"):
        if key in value:
            parsed = datetime.fromisoformat(value[key].replace("Z", "+00:00"))
            if parsed.tzinfo is None:
                raise ValueError(f"{key} must include a timezone")


def portable_path(name):
    if not isinstance(name, str) or not name or "\\" in name:
        raise ValueError(f"unsafe artifact path: {name!r}")
    path = PurePosixPath(name)
    if path.is_absolute() or path.as_posix() != name:
        raise ValueError(f"unsafe artifact path: {name!r}")
    for part in path.parts:
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", part) or part.endswith(".") or part.split(".")[0].lower() in RESERVED:
            raise ValueError(f"nonportable artifact path: {name!r}")
    return name


def _file_set(root):
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("study must be an existing regular directory")
    root = root.resolve()
    files, spellings = [], {}
    total = 0
    for directory, dirs, names in os.walk(root, followlinks=False):
        for name in dirs + names:
            path = Path(directory) / name
            relative = path.relative_to(root).as_posix()
            if relative == ".study.lock":
                continue
            portable_path(relative)
            if path.is_symlink() or path.resolve() != path.absolute():
                raise ValueError(f"links are not allowed: {relative}")
            key = relative.casefold()
            if key in spellings and spellings[key] != relative:
                raise ValueError(f"case-colliding artifact paths: {relative}")
            spellings[key] = relative
            if path.is_dir():
                continue
            if not path.is_file():
                raise ValueError(f"not a regular file: {relative}")
            size = path.stat().st_size
            total += size
            if size > MAX_FILE or total > MAX_TOTAL:
                raise ValueError("study exceeds bundle size limits")
            files.append(relative)
            if len(files) > MAX_FILES:
                raise ValueError("too many artifacts")
    return sorted(files)


@contextmanager
def study_lock(study):
    path = Path(study) / ".study.lock"
    with path.open("x", encoding="utf-8") as stream:
        stream.write(f"pid={os.getpid()}\n")
        stream.flush()
        os.fsync(stream.fileno())
    try:
        yield
    finally:
        path.unlink()


def create_study(plan_source, output):
    plan_source, output = Path(plan_source), Path(output)
    plan = read_json(plan_source)
    validate(plan, "study-plan")
    if plan["kind"] == "pipeline-test" and plan["dataOrigin"] != "local-simulator":
        raise ValueError("pipeline tests must be labelled local-simulator")
    data = plan_source.read_bytes()
    output.mkdir(parents=True, exist_ok=False)
    (output / "plan.json").write_bytes(data)
    (output / "plan.sha256").write_text(sha256(data) + "\n", encoding="ascii", newline="\n")
    (output / "attempts.jsonl").touch(exist_ok=False)
    (output / "raw").mkdir()
    (output / "analysis").mkdir()
    (output / "fixtures").mkdir()
    (output / "REPORT.md").write_text(
        f"# {plan['title']}\n\nData origin: {plan['dataOrigin']}. Scientific status: unreviewed.\n\n"
        "## Observations\n\nRecord measurements, exposure and failed/inconclusive attempts.\n\n"
        "## Interpretation\n\nSeparate direct observations, inferences and provisional rules.\n\n"
        "## Reproduce\n\nRecord exact commands and analysis parameters.\n\n"
        "## Data sources\n\nFor external data, note its source and any sharing restrictions.\n\n"
        "## Limitations\n\nState missing coverage, timing uncertainty and independent validation.\n",
        encoding="utf-8", newline="\n",
    )
    return output


def read_plan(study):
    study = Path(study)
    if (study / "plan.json").stat().st_size > MAX_METADATA or (study / "plan.sha256").stat().st_size != 65:
        raise ValueError("oversized plan or invalid frozen digest")
    data = (study / "plan.json").read_bytes()
    if (study / "plan.sha256").read_text(encoding="ascii") != sha256(data) + "\n":
        raise ValueError("frozen plan bytes changed")
    plan = read_json(study / "plan.json")
    validate(plan, "study-plan")
    if plan["kind"] == "pipeline-test" and plan["dataOrigin"] != "local-simulator":
        raise ValueError("pipeline tests must be labelled local-simulator")
    return plan


def read_attempts(study):
    study = Path(study)
    plan = read_plan(study)
    plan_hash = sha256((study / "plan.json").read_bytes())
    if (study / "attempts.jsonl").stat().st_size > MAX_METADATA:
        raise ValueError("oversized attempt ledger")
    data = (study / "attempts.jsonl").read_bytes()
    if len(data) > MAX_METADATA or (data and not data.endswith(b"\n")):
        raise ValueError("truncated or oversized attempt ledger")
    starts, ends = {}, {}
    for line in data.splitlines():
        event = json.loads(line, object_pairs_hook=_pairs, parse_constant=_constant)
        validate(event, "attempt-event")
        if event["studyId"] != plan["studyId"] or event["planSha256"] != plan_hash:
            raise ValueError("attempt does not belong to the frozen study")
        key = event["attemptId"]
        if event["event"] == "started":
            if key in starts:
                raise ValueError("duplicate attempt start")
            starts[key] = event
        else:
            if key not in starts or key in ends:
                raise ValueError("finish requires exactly one preceding start")
            before = datetime.fromisoformat(starts[key]["time"].replace("Z", "+00:00"))
            after = datetime.fromisoformat(event["time"].replace("Z", "+00:00"))
            if after < before:
                raise ValueError("attempt ends before it starts")
            if event["status"] == "completed" and event["exitCode"] != 0:
                raise ValueError("completed attempt must have exitCode 0")
            for capture in event["captures"]:
                if not portable_path(capture).startswith("raw/"):
                    raise ValueError("capture must be under raw/")
            ends[key] = event
    if len(starts) > plan["sampling"]["attempts"]:
        raise ValueError("attempt count exceeds the frozen plan")
    return starts, ends


def append_event(study, event):
    """Append durably while holding study_lock; an unfinished start remains visible."""
    study = Path(study)
    validate(event, "attempt-event")
    plan = read_plan(study)
    if event["studyId"] != plan["studyId"] or event["planSha256"] != sha256((study / "plan.json").read_bytes()):
        raise ValueError("attempt event belongs to another plan")
    starts, ends = read_attempts(study)
    key = event["attemptId"]
    if event["event"] == "started":
        if key in starts or len(starts) >= plan["sampling"]["attempts"]:
            raise ValueError("duplicate or unplanned attempt")
    elif key not in starts or key in ends:
        raise ValueError("finish requires exactly one preceding start")
    elif event["status"] == "completed" and event["exitCode"] != 0:
        raise ValueError("completed attempt must have exitCode 0")
    if event["event"] == "finished":
        before = datetime.fromisoformat(starts[key]["time"].replace("Z", "+00:00"))
        after = datetime.fromisoformat(event["time"].replace("Z", "+00:00"))
        if after < before:
            raise ValueError("attempt ends before it starts")
        for capture in event["captures"]:
            if not portable_path(capture).startswith("raw/"):
                raise ValueError("capture must be under raw/")
    with (study / "attempts.jsonl").open("a", encoding="utf-8", newline="\n") as stream:
        stream.write(json.dumps(event, allow_nan=False, separators=(",", ":")) + "\n")
        stream.flush()
        os.fsync(stream.fileno())


def _source_files():
    paths = [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "build.rs", "pyproject.toml", "uv.lock", "LICENSE")]
    for folder, suffixes in (("src", {".rs"}), ("schemas", {".json"}), ("tools/fidelity", {".py", ".mjs"}), ("examples", {".rs"})):
        paths.extend(path for path in (ROOT / folder).rglob("*") if path.suffix in suffixes and not path.name.startswith("test_") and ".test." not in path.name)
    return sorted(path for path in paths if path.is_file())


def _digest_file(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def audit_study(study):
    study = Path(study)
    files = _file_set(study)
    plan = read_plan(study)
    starts, ends = read_attempts(study)
    issues = []
    if len(starts) != plan["sampling"]["attempts"]:
        issues.append("planned_attempts_missing")
    if any(key not in ends or ends[key]["status"] != "completed" for key in starts):
        issues.append("failed_or_unfinished_attempts")
    declared = [capture for event in ends.values() for capture in event["captures"]]
    captures = [name for name in files if name.startswith("raw/") and name.endswith(".jsonl.gz")]
    if len(set(declared)) != len(declared) or set(declared) != set(captures):
        issues.append("raw_capture_membership_mismatch")
    if any(name.startswith("raw/") and name not in captures for name in files):
        issues.append("unexpected_raw_artifact")
    if any(event["status"] == "completed" and len(event["captures"]) != plan.get("capture", {}).get("bots", 1) for event in ends.values()):
        issues.append("completed_attempt_missing_members")
    if not captures:
        issues.append("no_captures")
    safe = []
    for name in captures:
        try:
            expanded = 0
            with gzip.open(study / name, "rb") as stream:
                for block in iter(lambda: stream.read(1024 * 1024), b""):
                    expanded += len(block)
                    if expanded > MAX_CAPTURE_TEXT:
                        raise ValueError("expanded capture exceeds limit")
            safe.append(study / name)
        except (OSError, EOFError, ValueError, zlib.error) as error:
            issues.append(f"invalid_capture:{name}:{type(error).__name__}")
    report = audit_captures(safe)
    if any(not all(batch[key] for key in ("completeMembership", "matchedProvenance", "sharedAcquisitionInterval")) for batch in report["batches"]):
        issues.append("incomplete_or_mismatched_capture_batch")
    software = study / "software"
    source_paths = [software / name[len("software/"):] for name in files if name.startswith("software/")] if software.is_dir() else _source_files()
    source_hashes = {_digest_file(path) for path in source_paths}
    source_root = software if software.is_dir() else ROOT
    source_names = {path.relative_to(source_root).as_posix() for path in source_paths}
    for capture in report["captures"]:
        name = Path(capture["path"]).relative_to(study).as_posix()
        capture["path"] = name
        provenance = capture.get("provenance")
        provenance = provenance if isinstance(provenance, dict) else {}
        header = capture.get("header")
        header = header if isinstance(header, dict) else {}
        if capture["status"] != "verified":
            issues.append(f"capture_not_verified:{name}")
        if provenance.get("planSha256") != sha256((study / "plan.json").read_bytes()):
            issues.append(f"capture_plan_mismatch:{name}")
        for key in ("collectorSha256", "headlessSha256"):
            if key in provenance and (not sha256_string(provenance[key]) or provenance[key] not in source_hashes):
                issues.append(f"unavailable_source:{name}:{key}")
        if "sources" in provenance:
            declared = provenance["sources"]
            if not isinstance(declared, dict) or not isinstance(declared.get("files"), dict):
                issues.append(f"invalid_source_manifest:{name}")
            else:
                pins = declared["files"]
                if not pins or not all(sha256_string(value) for value in pins.values()) or declared.get("combinedSha256") != source_map_hash(pins):
                    issues.append(f"invalid_source_manifest:{name}")
                for source_name, digest in pins.items():
                    try:
                        portable_path(source_name)
                    except ValueError:
                        issues.append(f"invalid_source_path:{name}")
                        continue
                    source = source_root / source_name
                    if source_name not in source_names or not source.is_file() or _digest_file(source) != digest:
                        issues.append(f"source_pin_mismatch:{name}:{source_name}")
        terminal = {}
        if capture["completeGzip"]:
            with gzip.open(study / name, "rb") as stream:
                last = b""
                for line in stream:
                    last = line
            if last:
                value = json.loads(last, object_pairs_hook=_pairs, parse_constant=_constant)
                terminal = value if isinstance(value, dict) else {}
        if "collector_source_changed_on_disk" in capture["warnings"] or terminal.get("collectorSourcesUnchanged") is False:
            issues.append(f"collector_source_changed:{name}")
        for start_key, end_key in (("collectorSha256", "collectorEndSha256"), ("headlessSha256", "headlessEndSha256")):
            if start_key in provenance and provenance[start_key] != terminal.get(end_key):
                issues.append(f"collector_source_changed:{name}:{start_key}")
        origin = header.get("dataOrigin")
        if (plan["dataOrigin"] == "local-simulator" and origin != "local-simulator") or (origin is not None and origin != plan["dataOrigin"]):
            issues.append(f"capture_origin_mismatch:{name}")
        if "capture" in plan:
            expected = plan["capture"]
            server = header.get("server")
            server = server if isinstance(server, dict) else {}
            options = provenance.get("options")
            options = options if isinstance(options, dict) else {}
            if any(server.get(key) != expected["server"][key] for key in ("ip", "port")) or any(options.get(key) != expected[key] for key in ("scenario", "seconds", "bots")):
                issues.append(f"capture_config_mismatch:{name}")
            if options.get("origin") != expected.get("origin"):
                issues.append(f"capture_config_mismatch:{name}")
            if expected["scenario"] in ("paired-prey", "paired-snake") and any(
                    options.get(key) != expected.get(key) or header.get(key) != expected.get(key)
                    for key in ("mode", "direction")):
                issues.append(f"capture_config_mismatch:{name}")
    by_name = {capture["path"]: capture for capture in report["captures"]}
    for event in ends.values():
        members = [by_name[name].get("provenance") for name in event["captures"] if name in by_name]
        batch_ids = {member["captureBatchId"] for member in members if isinstance(member, dict) and isinstance(member.get("captureBatchId"), str)}
        if len(batch_ids) > 1:
            issues.append(f"attempt_mixes_capture_batches:{event['attemptId']}")
    incomplete = {"planned_attempts_missing", "failed_or_unfinished_attempts", "no_captures"}
    status = "quarantined" if any(issue not in incomplete for issue in issues) else "incomplete" if issues else "verified"
    return {"status": status, "issues": sorted(set(issues)), "attempts": len(starts), "captures": len(captures)}


def package_study(study, output):
    study, output = Path(study).resolve(), Path(output).resolve()
    if output.is_relative_to(study):
        raise ValueError("bundle output must be outside the study directory")
    if output.exists():
        raise FileExistsError(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with study_lock(study), tempfile.TemporaryDirectory(prefix="slither-evidence-") as temporary:
        snapshot = Path(temporary) / "study"
        snapshot.mkdir()
        for name in _file_set(study):
            if name == "manifest.json" or name.startswith("software/"):
                raise ValueError("manifest.json and software/ are reserved for packaging")
            target = snapshot / name
            target.parent.mkdir(parents=True, exist_ok=True)
            with (study / name).open("rb") as source, target.open("xb") as destination:
                shutil.copyfileobj(source, destination)
        for source in _source_files():
            target = snapshot / "software" / source.relative_to(ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        plan = read_plan(snapshot)
        manifest = {
            "schemaVersion": 1, "studyId": plan["studyId"],
            "planSha256": sha256((snapshot / "plan.json").read_bytes()),
            "dataOrigin": plan["dataOrigin"], "scientificStatus": "unreviewed", "createdAt": utc_now(),
            "audit": audit_study(snapshot),
            "files": [{"path": name, "sha256": _digest_file(snapshot / name), "bytes": (snapshot / name).stat().st_size} for name in _file_set(snapshot)],
        }
        validate(manifest, "evidence-bundle")
        write_json(snapshot / "manifest.json", manifest)
        with tempfile.NamedTemporaryFile(prefix=".evidence-", suffix=".tar.gz", dir=output.parent, delete=False) as temporary_bundle:
            bundle = Path(temporary_bundle.name)
        try:
            with tarfile.open(bundle, "w:gz", format=tarfile.USTAR_FORMAT) as archive:
                for name in _file_set(snapshot):
                    archive.add(snapshot / name, arcname=name, recursive=False)
            verify_bundle(bundle)
            # Exclusive publication: a concurrent destination is preserved.
            os.link(bundle, output)
        finally:
            bundle.unlink()
    return manifest


@contextmanager
def verified_bundle(path):
    """Yield a checked snapshot; consumers never execute its bundled sources."""
    with tempfile.TemporaryDirectory(prefix="slither-evidence-verify-") as temporary:
        root = Path(temporary)
        names, directories, spellings = set(), set(), {}
        total = 0
        # Scan past padding so a concatenated second tar cannot hide extra members.
        with tarfile.open(path, "r:gz", ignore_zeros=True) as archive:
            for member in archive:
                name = portable_path(member.name)
                key = name.casefold()
                if not member.isfile() or member.issparse() or key in names or key in directories:
                    raise ValueError(f"duplicate or non-regular archive member: {name}")
                parents = [parent.as_posix().casefold() for parent in PurePosixPath(name).parents if parent.as_posix() != "."]
                for part in [PurePosixPath(name), *PurePosixPath(name).parents]:
                    spelling = part.as_posix()
                    if spelling == ".":
                        continue
                    folded = spelling.casefold()
                    if folded in spellings and spellings[folded] != spelling:
                        raise ValueError(f"case-colliding archive paths: {name}")
                    spellings[folded] = spelling
                if any(parent in names for parent in parents):
                    raise ValueError(f"archive file/directory collision: {name}")
                names.add(key)
                directories.update(parents)
                total += member.size
                if len(names) > MAX_FILES or member.size > MAX_FILE or total > MAX_TOTAL:
                    raise ValueError("archive exceeds size limits")
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                with archive.extractfile(member) as source, target.open("xb") as destination:
                    shutil.copyfileobj(source, destination)
        manifest = read_json(root / "manifest.json")
        validate(manifest, "evidence-bundle")
        expected, seen = {}, set()
        for entry in manifest["files"]:
            name = portable_path(entry["path"])
            if name.casefold() in seen or name == "manifest.json":
                raise ValueError("duplicate or self-referencing manifest entry")
            seen.add(name.casefold())
            expected[name] = entry
        actual = set(_file_set(root)) - {"manifest.json"}
        if actual != set(expected):
            raise ValueError("manifest does not describe exactly the archive members")
        missing_sources = {f"software/{name}" for name in V1_SOURCE_FLOOR} - actual
        if missing_sources:
            raise ValueError(f"bundle is missing its source snapshot files: {', '.join(sorted(missing_sources))}")
        for name, entry in expected.items():
            if (root / name).stat().st_size != entry["bytes"] or _digest_file(root / name) != entry["sha256"]:
                raise ValueError(f"artifact hash/size mismatch: {name}")
        plan = read_plan(root)
        if any(manifest[key] != plan[key] for key in ("studyId", "dataOrigin")) or manifest["planSha256"] != sha256((root / "plan.json").read_bytes()):
            raise ValueError("manifest identity differs from the frozen plan")
        if manifest["audit"] != audit_study(root):
            raise ValueError("manifest audit differs from recomputed capture checks")
        yield root, manifest


def verify_bundle(path):
    with verified_bundle(path) as (_, manifest):
        return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    init = commands.add_parser("init", help="freeze exact plan bytes into a fresh study")
    init.add_argument("--plan", required=True, type=Path)
    init.add_argument("--output", required=True, type=Path)
    audit = commands.add_parser("audit", help="audit a study without modifying it")
    audit.add_argument("study", type=Path)
    pack = commands.add_parser("pack", help="package a complete or failed study with source snapshots")
    pack.add_argument("study", type=Path)
    pack.add_argument("--output", required=True, type=Path)
    verify = commands.add_parser("verify", help="verify a bundle without executing its contents")
    verify.add_argument("bundle", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "init":
            result = {"study": str(create_study(args.plan, args.output))}
        elif args.command == "audit":
            with study_lock(args.study):
                result = audit_study(args.study)
        elif args.command == "pack":
            result = package_study(args.study, args.output)
        else:
            result = verify_bundle(args.bundle)
        print(json.dumps(result, indent=2, allow_nan=False))
        status = result.get("audit", result).get("status")
        return 2 if status in ("incomplete", "quarantined") else 0
    except (OSError, ValueError, KeyError, tarfile.TarError, zlib.error) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
