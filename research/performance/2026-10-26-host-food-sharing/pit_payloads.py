"""Normalize a successful single pit test's complete payload vectors."""
import json
import sys

raw = sys.stdin.read()
assert "test result: ok. 1 passed" in raw
frames = [json.loads(line.split("PIT_TRACE ", 1)[1])
          for line in raw.splitlines() if "PIT_TRACE " in line]
assert frames
for frame in frames:
    assert all(type(value) is int and 0 <= value <= 255 for value in frame)
    print(json.dumps(frame, separators=(",", ":")))
print(json.dumps({"kind": "end", "case": sys.argv[1], "frames": len(frames)},
                 separators=(",", ":")))
