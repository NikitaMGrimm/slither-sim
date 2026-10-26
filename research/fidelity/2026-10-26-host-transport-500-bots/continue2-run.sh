#!/bin/bash
set -euxo pipefail
export PATH=/home/ubuntu/.cargo/bin:/home/ubuntu/.local/bin:/usr/local/bin:/usr/bin:/bin
export CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/ubuntu/target
cd /home/ubuntu/source
/home/ubuntu/.local/bin/uv run --locked --only-group tools python -c 'import json; from jsonschema import validate; validate(json.load(open("research/fidelity/2026-10-26-host-transport-500-bots/second-continuation-plan.json")),json.load(open("schemas/study-plan-v1.schema.json")))'
python3 research/fidelity/2026-10-26-host-transport-500-bots/continue2.py /home/ubuntu/results >/home/ubuntu/continuation2.log 2>&1
tar -czf /home/ubuntu/results.tar.gz -C /home/ubuntu results
touch /home/ubuntu/complete
sudo /sbin/shutdown -h +20
