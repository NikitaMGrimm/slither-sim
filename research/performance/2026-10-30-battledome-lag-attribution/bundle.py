"""Remote archive producer; publish readiness only after closed member verification."""
from pathlib import Path
import hashlib
import json
import tarfile


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream,"sha256").hexdigest()


def package(directory, output):
    directory,output=Path(directory),Path(output)
    files=sorted(p for p in directory.rglob("*") if p.is_file() and "__pycache__" not in p.parts)
    manifest={p.relative_to(directory).as_posix():sha(p) for p in files}
    manifest_path=output.with_suffix(".manifest.json")
    manifest_path.write_text(json.dumps(manifest,sort_keys=True,indent=2)+"\n")
    temporary=output.with_suffix(".tmp")
    with tarfile.open(temporary,"w:gz",compresslevel=1) as archive:
        for path in files:archive.add(path,arcname=path.relative_to(directory).as_posix())
        archive.add(manifest_path,arcname="manifest.json")
    with tarfile.open(temporary,"r|gz") as archive:
        seen=set()
        for member in archive:
            assert member.name not in seen;seen.add(member.name)
            if member.name!="manifest.json":
                assert hashlib.file_digest(archive.extractfile(member),"sha256").hexdigest()==manifest[member.name]
        assert seen==set(manifest)|{"manifest.json"}
    temporary.replace(output)
    ready={"complete":True,"allMembersVerified":True,"requiredSourcesVerified":True,
           "archiveBytes":output.stat().st_size,"archiveSha256":sha(output),
           "manifestSha256":sha(manifest_path)}
    ready_path=output.with_name(output.name.replace(".tar.gz","-ready.json"))
    temporary=ready_path.with_suffix(".tmp")
    temporary.write_text(json.dumps(ready,indent=2)+"\n");temporary.replace(ready_path)
    return ready
