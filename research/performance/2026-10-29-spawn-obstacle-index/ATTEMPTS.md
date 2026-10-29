# Attempts

- Started from portable main `9ad002c1`. The limited-profile identity check
  passed; one tagged Spot builder launched with terminate shutdown behavior,
  a 180-minute in-guest timer and delete-on-termination root volume. No local
  native work or SSH guest use. Baseline fixture/profile precedes optimization.
- The first clone was dispatched while the source-bundle transfer was still
  running, so Git reported an early EOF; dependent staging also failed and the
  initial job had not arrived yet. No build ran. Waited for transfer completion,
  verified matching local/remote bundle SHA-256, then repeated clone/staging.
