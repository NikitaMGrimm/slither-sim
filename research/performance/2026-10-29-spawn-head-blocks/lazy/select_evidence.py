"""Reuse original gates, selecting all three arms without duplicate profile pins."""
from pathlib import Path
import json
import hashlib
import sys

prior = Path(__file__).resolve().parents[1] / "select_evidence.py"
text = prior.read_text()
text = text.replace('["base", "candidate", "mutation"]', '["base", "eager", "candidate", "mutation"]')
text = text.replace('for a in ["base", "candidate"]', 'for a in ["base", "eager", "candidate"]')
text = text.replace('assert values["base"]["summary"] == values["candidate"]["summary"]',
                    'assert values["base"]["summary"] == values["eager"]["summary"] == values["candidate"]["summary"]')
needle = '            summaries.append(values["base"]["summary"])'
text = text.replace(needle, '''            assert sha((proof / pair["paths"]["eager"]).with_suffix(".trace")) == sha((proof / pair["paths"]["candidate"]).with_suffix(".trace"))
''' + needle)
start = text.index('profile = read(')
end = text.index('write("verification.json"', start)
text = text[:start] + text[end:]
text = text.replace('"measurementWindows": 60, "discardedWarmupWindows": 12',
                    '"measurementWindows": 90, "discardedWarmupWindows": 18')
exec(compile(text, str(prior), "exec"), {"__file__": str(Path(__file__)), "__name__": "__main__"})
root, destination = map(Path, sys.argv[1:3])
eager = {row: json.loads((root / "proof" / f"selected-eager-fixed-{row}.json").read_text())
         for row in ["spawn", "seats80", "seats160"]}
(destination / "eager-measurements.json").write_text(json.dumps(eager, indent=2) + "\n", newline="\n")
p = destination / "verification.json"
value = json.loads(p.read_text())
value.update(focusedTestsPassed=7, lazyEagerVerdictsAndUnbuiltCacheTest=True,
             initialProfileReference="../profile.json",
             armRoles={"base": "main e860ec23", "eager": "reviewed head 78f7d1ec", "candidate": "lazy revision"},
             additionalEagerProofScope="All measured full-state/byte windows, repeated-fork and direct-parent proofs; catalog/host/queued matrix compares main against lazy.")
value["revisionHelperSha256"] = {name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
                                 for name in ["prepare.py", "run_pairs.py", "reduce.py", "tables.py"]}
for arm in ["base", "eager"]:
    folder = root / "proof" / arm
    pins = json.loads((folder / "pins.json").read_text())
    actual = {p.relative_to(folder / "source").as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in (folder / "source").rglob("*") if p.is_file()}
    assert actual == pins["files"]
value["mainAndEagerStagedSourceIdentityRechecked"] = True
p.write_text(json.dumps(value, indent=2) + "\n", newline="\n")
