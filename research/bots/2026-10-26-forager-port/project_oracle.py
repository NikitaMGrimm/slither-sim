"""Project one raw original-kernel output without rewriting expected-value bytes.

Usage: python project_oracle.py RAW.json PROJECTED.json
Input-frame hashes cover compact UTF-8 JSON with recursively sorted object keys,
no trailing newline and non-ASCII characters preserved. Frames retain f64 bit
strings; this projection never decodes them or runs a policy.
"""

import hashlib
import json
from pathlib import Path
import sys


DECODER = json.JSONDecoder()


def skip_space(text, offset):
    while text[offset].isspace():
        offset += 1
    return offset


def members(text, offset):
    """Locate raw object value spans using the JSON decoder, including strings."""
    assert text[offset] == "{"
    offset = skip_space(text, offset + 1)
    while text[offset] != "}":
        key, offset = DECODER.raw_decode(text, offset)
        offset = skip_space(text, offset)
        assert text[offset] == ":"
        start = skip_space(text, offset + 1)
        _, end = DECODER.raw_decode(text, start)
        yield key, (start, end)
        offset = skip_space(text, end)
        if text[offset] == "}":
            break
        assert text[offset] == ","
        offset = skip_space(text, offset + 1)


def compact(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def project(raw):
    text = raw.decode("utf-8")
    fields = dict(members(text, skip_space(text, 0)))
    metadata = {
        key: json.loads(text[start:end])
        for key, (start, end) in fields.items()
        if key in {"clientContract", "wireProtocol", "kernelCommit", "oraclePlatform", "oracleStatus"}
    }
    assert metadata["oracleStatus"] == "complete"
    metadata["schemaVersion"] = 2
    start, _ = fields["cases"]
    assert text[start] == "["
    offset = skip_space(text, start + 1)
    cases = []
    while text[offset] != "]":
        case, end = DECODER.raw_decode(text, offset)
        spans = dict(members(text, offset))
        expected_start, expected_end = spans["expected"]
        # Copy this complete JSON subtree verbatim, rather than serializing or
        # recomputing any expected Timing, Intent or Action value.
        expected = text[expected_start:expected_end]
        frame_hash = hashlib.sha256(compact(case["frame"]).encode("utf-8")).hexdigest()
        cases.append(
            '{"expected":' + expected + ',"id":' + compact(case["id"])
            + ',"inputSha256":' + compact(frame_hash) + '}'
        )
        offset = skip_space(text, end)
        if text[offset] == "]":
            break
        assert text[offset] == ","
        offset = skip_space(text, offset + 1)
    # Preserve the raw expected subtrees when assembling the enclosing object.
    suffix = compact(metadata)[1:]
    return ('{"cases":[' + ",".join(cases) + '],' + suffix + "\n").encode("utf-8")


if __name__ == "__main__":
    source, destination = map(Path, sys.argv[1:])
    if source.resolve() == destination.resolve():
        raise ValueError("Keep the raw oracle output; choose a different projection path")
    destination.write_bytes(project(source.read_bytes()))
