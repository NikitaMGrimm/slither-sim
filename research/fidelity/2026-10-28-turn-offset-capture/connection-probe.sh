#!/bin/sh
# Read-only host census; scratch files belong solely to this attempt.
set -eu
root=$1
name=$2
endpoint=$3
case "$root" in /tmp/turn-capture.*) ;; *) exit 10;; esac
case "$name" in *[!a-zA-Z0-9_.-]*|'') exit 10;; esac
case "$endpoint" in *[!0-9.:]*|'') exit 10;; esac
ss -Htanp dst "$endpoint" > "$root/ss.snapshot"
docker top "$name" -eo pid,args > "$root/owned.snapshot"
: > "$root/commands.snapshot"
for pid in $(grep -o 'pid=[0-9][0-9]*' "$root/ss.snapshot" | cut -d= -f2 | sort -u); do
    ps -ww -p "$pid" -o pid=,args= >> "$root/commands.snapshot" || true
done
base64 -w0 "$root/owned.snapshot"
printf '\n'
base64 -w0 "$root/ss.snapshot"
printf '\n'
base64 -w0 "$root/commands.snapshot"
printf '\n'
