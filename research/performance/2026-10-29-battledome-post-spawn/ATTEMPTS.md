# Attempts

- The maintaining agent requested a new real-socket comparison of #267's
  candidate-era main against the merged #272/#273 spawn fixes. No production
  changes, local native work or SSH guest use. The limited-profile identity
  check passed before any launch.
- The first generator Spot launch failed with `InsufficientInstanceCapacity`
  before any instance was created. Retained the request/error and reused the
  tagged key/group for a different same-region AZ; no policy denial or compute
  charge occurred on that failed launch.
- The alternate-AZ launch created and recorded both timed instances. Its final
  informational print referenced an unset budget variable and raised after
  launch. Confirmed both role records and continued setup without relaunching.
- The asynchronous setup SSH invocation timed out after dispatch. Inspection
  confirmed `BUILT`, no active setup/build process and successful boost-treatment
  regressions; no build was dispatched again. The complete native build log and
  staged sources were retrieved in a closed, member-verified archive before rows.
- The original Spot target/generator completed all four primary ladders in the
  frozen interleaved order without interruption or generator replacement.
  Every primary and calibration passed generator/queue/boost gates; no row was
  excluded for server overload. Candidate-480 recorded 5,067 deferred ticks,
  so 640 was skipped under the frozen conditional gate. No 160 primary was
  needed. Both 480 rows are overloaded, and the candidate's mixed result is
  retained without a repeatability or capacity claim.
- Final reduction and assembly ran on AWS after all counting stopped. The
  inherited synthetic controls and four assembler provenance/receipt mutations
  passed. Sources remain available until the final analysis archive is closed,
  member-verified and downloaded with matching size/SHA.
- The first final-document command failed to parse because the controller
  expanded a newline inside its embedded Python command. No checks or archive
  export ran in that invocation. Retained the error, corrected the quoting and
  reran the document/export step without repeating any measurement.
- The first cleanup scan still saw the two task Spot requests as active just
  after cancellation. A subsequent read confirmed both cancelled; repeated
  the complete enabled-region scan rather than publishing the stale counts.
- Final cleanup confirmed both instances terminated and both root volumes
  deleted, removed the task key/group and scanned all 17 enabled regions with
  zero nonterminated instances, open/active Spot requests, volumes or Elastic
  IPs. Task resources are gone; protected resources were untouched. The final
  conservative cost bound is recorded in cost.json, including the failed launch.
- The first privacy scan mistook 12-digit fractional parts of metric values
  for account numbers. Inspected the matches and excluded decimal fractions
  from that rule; the identifier/address/private-name scan then passed.
- Review found that selection and assembly could use different master build
  files. Bound each row's archived pins directly to the published master pins.
  Reassembled all four original archives on AWS: selected results are identical
  apart from the assembler digest. Six master-pin mutations (runtime, source
  archive, instrumented manifest, client source, host binary, shared client binary)
  fail at the archived-row binding. The unused base client is not the shared
  candidate client; both rows bind to the actual common client hash.
