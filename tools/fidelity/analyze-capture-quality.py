import argparse
import json
from pathlib import Path

from capture_quality import audit_captures


def main():
    parser = argparse.ArgumentParser(description="Audit immutable headless archives without deleting or selecting favorable traces.")
    parser.add_argument("captures", nargs="+")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = audit_captures(args.captures)
    text = json.dumps(report, indent=2, allow_nan=False) + "\n"
    if args.output:
        with args.output.open("x", encoding="utf-8") as stream:
            stream.write(text)
    else:
        print(text, end="")
    return 2 if report["exclusions"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
