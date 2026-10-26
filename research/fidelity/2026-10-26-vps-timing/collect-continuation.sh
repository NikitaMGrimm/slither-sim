#!/bin/sh
set -eu
cd /study
for round in round-1 round-2 round-3; do
  printf '%s starting %s\n' "$(date -u +%FT%TZ)" "$round" >> /data/attempts.log
  set +e
  timeout --signal=TERM --kill-after=10 640 node research/fidelity/2026-10-26-vps-timing/capture-continuation.mjs research/fidelity/2026-10-26-vps-timing/continuation-plan.json "/data/$round" "$round" /data/endpoint-stops.json > "/data/$round.log" 2>&1
  result=$?
  set -e
  printf '%s ended %s exit=%s\n' "$(date -u +%FT%TZ)" "$round" "$result" >> /data/attempts.log
  # Endpoint failures are outcomes, not a fatal coordinator error. Watchdog/driver failures stop here.
  if [ "$result" -ne 0 ]; then exit "$result"; fi
  if [ "$round" != round-3 ]; then sleep 30; fi
done
