#!/usr/bin/env bash
# File: scripts/test/winsync.check.sh
# Purpose: self-check of `scripts/wsl/winsync clean`: its destination guard refuses a drive, a folder one level below a drive, a path outside the Windows drives and a path that climbs back to a drive, and lets a copy folder through. The real winsync runs from a throwaway repo (so no winsync.env applies) with an `rm` on PATH that only logs, so nothing is ever deleted.
# Contents: run_clean, expect, the cases, the verdict.
# Usage: bash scripts/test/winsync.check.sh   (run in WSL after changing winsync; prints PASS or FAIL and exits 1 on any failure)
set -uo pipefail
SANDBOX=$(mktemp -d)
trap 'rm -rf "$SANDBOX"' EXIT
mkdir -p "$SANDBOX/repo/scripts/wsl" "$SANDBOX/bin"
cp "$(dirname "$0")/../wsl/winsync" "$SANDBOX/repo/scripts/wsl/winsync"
printf '#!/bin/sh\necho "$*" >> "%s/rm.log"\n' "$SANDBOX" > "$SANDBOX/bin/rm"
chmod +x "$SANDBOX/bin/rm"
FAILED=0

# Runs winsync clean with the given DEST_WSL and the logging rm. Input: the destination. Prints the exit code and what rm was asked to delete.
run_clean() {
  : > "$SANDBOX/rm.log"
  DEST_WSL=$1 PATH="$SANDBOX/bin:$PATH" bash "$SANDBOX/repo/scripts/wsl/winsync" clean >/dev/null 2>&1
  echo "rc=$? rm=[$(cat "$SANDBOX/rm.log")]"
}

# Records and prints one case. Inputs: name, expected run_clean output, destination.
expect() {
  local got; got=$(run_clean "$3")
  if [ "$got" = "$2" ]; then printf '%-58s ok\n' "$1"; else printf '%-58s FAILED (%s)\n' "$1" "$got"; FAILED=1; fi
}

expect "refuses a drive" "rc=2 rm=[]" /mnt/c
expect "refuses a folder one level below a drive" "rc=2 rm=[]" /mnt/c/dev
expect "refuses a path outside the Windows drives" "rc=2 rm=[]" /tmp/cue-copy
expect "refuses a path that climbs back to a drive" "rc=2 rm=[]" /mnt/c/dev/cue-copy/../..
expect "removes a copy folder" "rc=0 rm=[-rf /mnt/c/dev/cue-check-copy]" /mnt/c/dev/cue-check-copy
if [ "$FAILED" -eq 0 ]; then echo PASS; else echo FAIL; exit 1; fi
