#!/bin/bash
# crash-report.sh — bundle everything an issue triager would ask for.
#
# When a launch fails, attaching just the launcher log usually isn't
# enough; the triager wants the bottle state, the analyzer hits, the
# doctor output, and the launch log. This script collects all of them
# into a single timestamped zip under /tmp/.
#
# Usage:
#   crash-report.sh                       # auto-pick most recent failed bottle
#   crash-report.sh <bottle>              # explicit bottle name
#   crash-report.sh --log /tmp/foo.log    # bundle for a specific log path
#
# The zip lands at /tmp/cellar-crash-<bottle>-<timestamp>.zip and is
# safe to attach to a GitHub issue. The bundle contains nothing
# user-private beyond what's already in the launcher log.
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

LOG=""
BOTTLE=""
case "${1:-}" in
  --log) LOG="${2:?--log needs a path}" ;;
  "") ;;
  *) BOTTLE="$1" ;;
esac

# Auto-pick the most recent cellar log if neither --log nor a bottle name
# was given.
if [ -z "$LOG" ] && [ -z "$BOTTLE" ]; then
  LOG=$(find /tmp -maxdepth 1 -type f \( \
      -name 'cellar-*.log' -o -name 'fifa*.log' -o \
      -name 'carxstreet*.log' -o -name 'nfsmw*.log' -o \
      -name 'rdr2*.log' -o -name 'skyrim*.log' \
    \) -print 2>/dev/null \
    | xargs -I{} stat -f "%m %N" {} 2>/dev/null \
    | sort -rn | head -1 | awk '{$1=""; sub(/^  */,""); print}')
  if [ -z "$LOG" ]; then
    echo "no cellar logs found in /tmp; pass a bottle name or --log path" >&2
    exit 1
  fi
fi

# Infer bottle from log filename if needed.
if [ -z "$BOTTLE" ] && [ -n "$LOG" ]; then
  base=$(basename "$LOG" .log)
  case "$base" in
    cellar-*) BOTTLE="${base#cellar-}" ;;
    fifa*)    BOTTLE="$base" ;;
    *)        BOTTLE="$base" ;;
  esac
fi

# Locate matching log if only bottle name was given.
if [ -z "$LOG" ] && [ -n "$BOTTLE" ]; then
  for cand in "/tmp/cellar-$BOTTLE.log" "/tmp/$BOTTLE.log"; do
    if [ -f "$cand" ]; then LOG="$cand"; break; fi
  done
fi

TS=$(date +%Y-%m-%d-%H%M%S)
WORK="/tmp/cellar-crash-$BOTTLE-$TS"
ZIP="/tmp/cellar-crash-$BOTTLE-$TS.zip"
mkdir -p "$WORK"

echo "==> bundling crash report for bottle: $BOTTLE"

# 1. The launcher log (last 500 lines, full file can be huge).
if [ -n "$LOG" ] && [ -f "$LOG" ]; then
  tail -500 "$LOG" > "$WORK/launcher-log-tail.txt"
  wc -l "$LOG" | awk '{print "(full log is " $1 " lines; included tail of 500)"}' > "$WORK/launcher-log-info.txt"
else
  echo "no launcher log available" > "$WORK/launcher-log-tail.txt"
fi

# 2. analyze-log.sh against the log.
if [ -n "$LOG" ] && [ -x "$CELLAR_ROOT/scripts/analyze-log.sh" ]; then
  "$CELLAR_ROOT/scripts/analyze-log.sh" "$LOG" > "$WORK/analyze-log.txt" 2>&1 || true
fi

# 3. bottle-inspect.sh.
if [ -x "$CELLAR_ROOT/scripts/bottle-inspect.sh" ] && [ -d "$HOME/.cellar/bottles/$BOTTLE" ]; then
  "$CELLAR_ROOT/scripts/bottle-inspect.sh" "$BOTTLE" > "$WORK/bottle-inspect.txt" 2>&1 || true
fi

# 4. cellar-doctor.sh.
if [ -x "$CELLAR_ROOT/scripts/cellar-doctor.sh" ]; then
  "$CELLAR_ROOT/scripts/cellar-doctor.sh" > "$WORK/cellar-doctor.txt" 2>&1 || true
fi

# 5. Host metadata.
{
  echo "macOS: $(sw_vers -productVersion 2>/dev/null || echo unknown)"
  echo "arch:  $(uname -m)"
  echo "model: $(sysctl -n hw.model 2>/dev/null || echo unknown)"
  echo "wine:  $("$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib/wine/x86_64-unix/wine" --version 2>/dev/null || echo not-found)"
  echo "git:   $(cd "$CELLAR_ROOT" && git rev-parse HEAD 2>/dev/null || echo unknown)"
} > "$WORK/host-metadata.txt"

# 6. Profile dump (the matching profile for this bottle).
if [ -f "$CELLAR_ROOT/profiles.json" ] && command -v jq >/dev/null 2>&1; then
  profile_id="${BOTTLE%%-*}"
  jq ".profiles[] | select(.id == \"$profile_id\")" "$CELLAR_ROOT/profiles.json" > "$WORK/profile.json" 2>/dev/null || true
fi

# Zip it up.
(cd /tmp && zip -qr "$ZIP" "$(basename "$WORK")")
rm -rf "$WORK"

size=$(du -h "$ZIP" | cut -f1)
echo
echo "DONE. Crash report: $ZIP ($size)"
echo
echo "Attach the zip to a GitHub issue:"
echo "  https://github.com/joemunene-by/cellar/issues/new?template=bug.yml"
