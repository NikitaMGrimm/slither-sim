# Attempts

- Kept the minimap branch pinned, including its 29 complete byte comparisons,
  mutation controls and full Rust checks. Started this independent spawn branch
  from the merged high-boost study's main revision, preserving the maintaining
  agent's changelog/index wording.
- Implemented only preconstruction rejection outside the fallback window. A
  frozen pre-change search oracle passes bitwise poses/headings, error strings,
  subsequent RNG and checkpoint checks for three seeds, empty/blocked/open
  scenes, floor/no-floor and ages immediately below/at/above two seconds.
- Eight safety tests pass, including invalid/nonfinite ring geometry retaining
  its construction path. These are correctness checks, not timing evidence.
- SSH guest remains unused. No AWS work or added cloud cost.
