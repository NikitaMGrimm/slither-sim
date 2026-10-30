"""Close the analysis archive after verifying its separately retained components.

Remote use: python3 export.py ROOT. ROOT has the selected build/row archives,
receipts and an analysis/ directory of reports, scripts, selected data and logs.
Full native/raw components stay separate; do not duplicate them in this archive.
"""
from pathlib import Path
import hashlib
import json
import sys
import tarfile

from assemble import verify_members


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


root = Path(sys.argv[1]).resolve()
analysis = root / "analysis"
components = []
for label, directory, archive_name, ready_name, manifest_name in [
    ("native-build", root, "build-sources.tar.gz", "build-ready.json", "build-manifest.json"),
    *[(label, root / "rows" / label, "row.tar.gz", "row-ready.json", "row-manifest.json")
      for label in ["01-solo", "02-80", "03-160", "04-320"]],
]:
    receipt = json.loads((directory / ready_name).read_text())
    assert receipt["complete"] and receipt["allMembersVerified"] and receipt["requiredSourcesVerified"]
    path = directory / archive_name
    assert path.stat().st_size == receipt["archiveBytes"] and sha(path) == receipt["archiveSha256"]
    verify_members(path, manifest_name)
    components.append(dict(label=label, **receipt))
(analysis / "components.json").write_text(json.dumps(components, indent=2) + "\n")
files = sorted(path for path in analysis.rglob("*") if path.is_file() and "__pycache__" not in path.parts)
manifest = {path.relative_to(analysis).as_posix(): sha(path) for path in files}
manifest_path = root / "analysis-manifest.json"
manifest_path.write_text(json.dumps(manifest, sort_keys=True, indent=2) + "\n")
temporary = root / "analysis.tar.gz.tmp"
with tarfile.open(temporary, "w:gz", compresslevel=1) as archive:
    for path in files:
        archive.add(path, arcname=path.relative_to(analysis).as_posix())
    archive.add(manifest_path, arcname="analysis-manifest.json")
with tarfile.open(temporary) as archive:
    assert len(archive.getmembers()) == len(manifest) + 1
    for name, digest in manifest.items():
        with archive.extractfile(name) as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == digest
output = root / "analysis.tar.gz"
temporary.replace(output)
ready = {"complete": True, "allMembersVerified": True, "requiredSourcesVerified": True,
         "componentArchivesVerified": len(components), "archiveBytes": output.stat().st_size,
         "archiveSha256": sha(output), "manifestSha256": sha(manifest_path)}
path = root / "analysis-ready.tmp"
path.write_text(json.dumps(ready, indent=2) + "\n")
path.replace(root / "analysis-ready.json")
print(json.dumps(ready))
