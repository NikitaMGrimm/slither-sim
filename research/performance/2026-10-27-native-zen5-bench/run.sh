#!/bin/sh
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
if [ "$#" -lt 1 ]; then
    echo "usage: ./run.sh COMMIT [--preset official|battledome] [--count N]" >&2
    exit 2
fi
python3 bench.py trial "$@"
