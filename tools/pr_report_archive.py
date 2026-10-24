"""Read exactly two bounded report files from an untrusted artifact, never extract paths."""

from pathlib import Path
import stat
import sys
import zipfile


def unpack(archive, directory):
    limits = {"report.json": 1_000_000, "report.md": 50_000}
    with zipfile.ZipFile(archive) as source:
        entries = source.infolist()
        if sorted(item.filename for item in entries) != sorted(limits):
            raise ValueError("Artifact must contain exactly report.json and report.md")
        payloads = {}
        for item in entries:
            if (item.file_size > limits[item.filename] or item.flag_bits & 1
                    or stat.S_ISLNK(item.external_attr >> 16)):
                raise ValueError("Invalid artifact entry")
            with source.open(item) as stream:
                payloads[item.filename] = stream.read(limits[item.filename] + 1)
            if len(payloads[item.filename]) > limits[item.filename]:
                raise ValueError("Oversized report entry")
    directory.mkdir()
    for name, data in payloads.items():
        (directory / name).write_bytes(data)


if __name__ == "__main__":
    unpack(Path(sys.argv[1]), Path(sys.argv[2]))
