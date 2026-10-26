#!/bin/bash
set -euxo pipefail
shutdown -h +120
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq build-essential pkg-config libssl-dev git curl python3 python3-pip xz-utils
curl --fail --location https://nodejs.org/dist/v24.10.0/node-v24.10.0-linux-x64.tar.xz -o /tmp/node.tar.xz
tar -xJf /tmp/node.tar.xz --strip-components=1 -C /usr/local
sudo -u ubuntu bash -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.99.0'
sudo -u ubuntu bash -c 'curl -LsSf https://astral.sh/uv/install.sh | sh'
sudo -u ubuntu git clone https://github.com/NikitaMGrimm/slither-sim.git /home/ubuntu/source
cd /home/ubuntu/source
sudo -u ubuntu git checkout --detach 1ba386f543422e1df9e1a60b0a1ce229d40055a8
while [ ! -f /home/ubuntu/payload-ready ]; do sleep 5; done
tar -xf /home/ubuntu/study.tar -C /home/ubuntu/source
chown -R ubuntu:ubuntu /home/ubuntu/source/research/fidelity/2026-10-26-host-transport-500-bots
cat >/home/ubuntu/execute.sh <<'RUN'
#!/bin/bash
set -euxo pipefail
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:/usr/local/bin:$PATH"
export CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR=/home/ubuntu/target
cd /home/ubuntu/source
cargo build --release --locked -j2
uv run --locked --only-group tools python -B -m unittest discover -s tools/fidelity -p test_transport_parity.py
uv run --locked --only-group tools python -c 'import json; from jsonschema import validate; validate(json.load(open("research/fidelity/2026-10-26-host-transport-500-bots/plan.json")),json.load(open("schemas/study-plan-v1.schema.json")))'
python3 research/fidelity/2026-10-26-host-transport-500-bots/run.py /home/ubuntu/results
tar -czf /home/ubuntu/results.tar.gz -C /home/ubuntu results
touch /home/ubuntu/complete
RUN
chmod +x /home/ubuntu/execute.sh
sudo -u ubuntu /home/ubuntu/execute.sh >/home/ubuntu/runner.log 2>&1
# Allow twenty minutes for robust retrieval; the independent boot timer is 120m.
sleep 1200
shutdown -h now
