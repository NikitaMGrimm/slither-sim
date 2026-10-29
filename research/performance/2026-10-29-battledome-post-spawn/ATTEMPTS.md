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
