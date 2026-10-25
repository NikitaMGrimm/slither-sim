"""Read a bounded direct-request ZIP; never extract artifact-controlled paths."""

from pathlib import Path
import stat
import sys
import zipfile

from pr_report_archive import unpack


def request(archive, directory):
    with zipfile.ZipFile(archive) as source:
        entries = source.infolist()
        if len(entries) != 1 or entries[0].filename != "request.json":
            raise ValueError("Expected exactly request.json")
        entry = entries[0]
        if entry.file_size > 20_000 or entry.flag_bits & 1 or stat.S_ISLNK(entry.external_attr >> 16):
            raise ValueError("Invalid request entry")
        with source.open(entry) as stream:
            data = stream.read(20_001)
        if len(data) > 20_000:
            raise ValueError("Oversized request")
    directory.mkdir()
    (directory / "request.json").write_bytes(data)


if __name__ == "__main__":
    (request if sys.argv[1] == "request" else unpack)(Path(sys.argv[2]), Path(sys.argv[3]))
