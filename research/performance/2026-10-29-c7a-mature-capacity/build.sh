#!/bin/bash
set -euo pipefail
cd /srv/slither-study
printf 'BUILDING\n' > status
trap 'printf "FAILED build\n" > /srv/slither-study/status' ERR
python3 prepare.py
export CARGO_TARGET_DIR=/srv/slither-study/target
export CARGO_BUILD_JOBS=4
export CARGO_PROFILE_RELEASE_DEBUG=1
export RUSTFLAGS='-C target-cpu=x86-64'
unset CARGO_ENCODED_RUSTFLAGS
export RUSTC="$(rustup which rustc)"
cd server/source
cargo build --release --locked --bin slither-sim --example aws_clients
cd /srv/slither-study
mkdir bin
cp target/release/slither-sim bin/host
cp target/release/examples/aws_clients bin/clients
python3 - <<'PY'
from pathlib import Path
import json,hashlib,subprocess,shutil
r=Path.cwd();p=json.loads((r/'prepared.json').read_text());p['toolchain']=subprocess.check_output(['rustc','--version'],text=True).strip();p['binaries']={f.name:hashlib.file_digest(f.open('rb'),'sha256').hexdigest() for f in (r/'bin').iterdir()};(r/'pins.json').write_text(json.dumps(p,indent=2)+'\n');target=r/'target';assert target.resolve().parent==r and not target.is_symlink();shutil.rmtree(target)
PY
printf 'BUILT\n' > status
