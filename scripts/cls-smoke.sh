#!/usr/bin/env bash
# Quick smoke test of the CLS host + plugin loading.
#
# For each cls-*.dll staged in CELLAR_CLS_DIR, pipes empty input
# through `wine cellar-freearc-cls-host.exe --dll <plugin>` and
# reports the exit code. We are not trying to decode anything; we
# just want to know whether the host process can:
#
#   1. start under wine
#   2. LoadLibrary the plugin DLL
#   3. resolve ClsMain via GetProcAddress
#   4. call ClsMain(CLS_DECOMPRESS, ...) without crashing
#
# Exit codes (from freearc-cls-host/src/main.rs):
#   0        clean run, plugin handled empty input gracefully
#   2        host failed to read stdin
#   3        --params has interior NUL (impossible here)
#   4        LoadLibrary failed (DLL broken or arch mismatch)
#   5        GetProcAddress(ClsMain) failed (not a CLS plugin)
#   6        host failed to write stdout
#   any-other (1, 7..127)
#            plugin ran but returned non-zero (very likely "truncated
#            input" or similar; that is STILL a pass — the plumbing
#            works)
#
# Usage:
#   export CELLAR_CLS_HOST=...
#   export CELLAR_CLS_DIR=...
#   scripts/cls-smoke.sh

set -u

if [ -z "${CELLAR_CLS_HOST:-}" ] || [ -z "${CELLAR_CLS_DIR:-}" ]; then
  echo "set CELLAR_CLS_HOST and CELLAR_CLS_DIR first (see scripts/cls-setup.sh output)"
  exit 1
fi

if [ ! -f "$CELLAR_CLS_HOST" ]; then
  echo "CELLAR_CLS_HOST does not point at a file: $CELLAR_CLS_HOST"
  exit 1
fi

if [ ! -d "$CELLAR_CLS_DIR" ]; then
  echo "CELLAR_CLS_DIR does not point at a dir: $CELLAR_CLS_DIR"
  exit 1
fi

wine_bin="${CELLAR_WINE:-wine}"
if ! command -v "$wine_bin" >/dev/null 2>&1; then
  echo "wine binary not found on PATH (set CELLAR_WINE to override)"
  exit 1
fi

verdict() {
  case "$1" in
    0)   echo "PASS  rc=0  plugin handled empty input cleanly" ;;
    4)   echo "FAIL  rc=4  LoadLibrary failed (DLL broken or bitness mismatch)" ;;
    5)   echo "FAIL  rc=5  GetProcAddress(ClsMain) failed (not a CLS plugin)" ;;
    2|3|6)
         echo "FAIL  rc=$1  host I/O bug, not a plugin problem" ;;
    *)   echo "PASS  rc=$1  plugin returned non-zero (likely 'no input'; plumbing works)" ;;
  esac
}

echo "host:    $CELLAR_CLS_HOST"
echo "cls dir: $CELLAR_CLS_DIR"
echo "wine:    $wine_bin"
echo

found=0
for dll in "$CELLAR_CLS_DIR"/cls-*.dll "$CELLAR_CLS_DIR"/CLS-*.dll; do
  [ -f "$dll" ] || continue
  found=$((found + 1))
  name="$(basename "$dll")"
  printf "%-26s  " "$name"
  # Run with no input, capture rc, swallow stderr (wine is chatty).
  printf '' | "$wine_bin" "$CELLAR_CLS_HOST" --dll "$dll" --params "" \
    >/dev/null 2>/tmp/cls-smoke.stderr
  rc=$?
  verdict "$rc"
done

if [ "$found" -eq 0 ]; then
  echo "no cls-*.dll found in CELLAR_CLS_DIR"
  exit 1
fi

echo
echo "stderr of last run kept at /tmp/cls-smoke.stderr"
