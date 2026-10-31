"""Verify normalized package closure against the registry, without publishing."""
import json
import os
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    sim_version = manifest["workspace"]["package"]["version"]
    protocol = tomllib.loads((ROOT / "crates/slither-protocol/Cargo.toml").read_text())["package"]
    for name, version in (("slither-sim", sim_version), ("slither-protocol", protocol["version"])):
        archive = target / "package" / f"{name}-{version}"
        normalized = tomllib.loads((archive / "Cargo.toml").read_text())
        if normalized["package"]["version"] != version:
            raise ValueError(f"Wrong normalized version: {name}")
        identity = json.loads((archive / ".cargo_vcs_info.json").read_text())["git"]
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        if identity.get("dirty") or identity["sha1"] != head:
            raise ValueError(f"Package must come from the clean selected commit: {name}")
    extracted = target / "package" / f"slither-sim-{sim_version}"
    dependency = tomllib.loads((extracted / "Cargo.toml").read_text())["dependencies"]["slither-protocol"]
    if "path" in dependency or not dependency["optional"] or dependency["version"] != manifest["dependencies"]["slither-protocol"]["version"]:
        raise ValueError("Root archive must retain an optional registry protocol dependency")
    # This is a standalone normalized archive, outside the source workspace.
    # Cargo metadata must identify the resolved package as registry-sourced.
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--format-version", "1",
        "--manifest-path", str(extracted / "Cargo.toml"),
    ], cwd=extracted, text=True))
    resolved = next(p for p in metadata["packages"] if p["name"] == "slither-protocol")
    if not (resolved["source"] or "").startswith("registry+"):
        raise ValueError("Extracted simulator resolved a local protocol package")
    subprocess.run(["cargo", "check", "--locked", "--lib", "--bins",
                    "--manifest-path", str(extracted / "Cargo.toml")], cwd=extracted, check=True)
    print(f"Verified extracted slither-sim {sim_version} library/CLI against registry slither-protocol {resolved['version']}")


if __name__ == "__main__":
    main()
