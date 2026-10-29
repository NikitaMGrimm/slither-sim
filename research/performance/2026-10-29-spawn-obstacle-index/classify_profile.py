"""Classify retained full-DWARF CPU-clock stacks on the remote builder."""
from collections import Counter
from pathlib import Path
import hashlib
import json
import re
import sys

root = Path(sys.argv[1])
selected = json.loads((root / "selected-profile.json").read_text())
for name in ["refused", "successful"]:
    path = root / name / "stacks.txt"
    groups, parts, allocation_callers = Counter(), Counter(), Counter()
    for stack in path.read_text().strip().split("\n\n"):
        if not stack.strip():
            continue
        if "new_excluding" in stack:
            phase = "safetyConstruction"
        elif "head_is_clear" in stack or "blocks_head" in stack:
            phase = "headPredicate"
        elif "clears_departures" in stack:
            phase = "departurePredicates"
        elif "body_margin" in stack:
            phase = "bodyMargin"
        elif re.search(r"\bclears(?:[<(\s+])", stack):
            phase = "bodyPredicate"
        elif any(word in stack for word in ["ring_step", "ring_body", "spawn_body"]):
            phase = "ringGeometry"
        else:
            phase = "otherAdmission"
        # Only the innermost physical/inline frames establish allocator CPU;
        # caller names alone must not reclassify an entire outer phase.
        inner = "\n".join(stack.splitlines()[1:4])
        allocator = bool(re.search(r"\b(?:malloc|cfree|free|realloc)\b|_int_(?:malloc|free)|__(?:rust|rdl)_(?:alloc|dealloc|realloc)", inner))
        groups["allocation" if allocator else phase] += 1
        if allocator:
            allocation_callers[phase] += 1
        kind = ("nearBounds" if re.search(r"\bnear(?:[<(\s+])", stack)
                else "exactDistance" if any(word in stack for word in ["swept_distance", "segment_distance_le", "closest_on_segment", "distance_order", "hypot"])
                else "loopAndOther")
        parts[phase + "/" + kind] += 1
    row = selected["rows"][name]
    row.update(samples=sum(groups.values()), exclusiveGroups=dict(groups),
               phasePartsIncludingNestedAllocation=dict(parts), allocationCallers=dict(allocation_callers),
               exclusivePercent={key: value / sum(groups.values()) * 100 for key, value in groups.most_common()})
selected["build"] = "Portable default release codegen; full DWARF only for sampling, no native flags"
selected["classificationSha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
(root / "classified-profile.json").write_text(json.dumps(selected, indent=2) + "\n")
print(json.dumps({name: row["exclusivePercent"] for name, row in selected["rows"].items()}))
