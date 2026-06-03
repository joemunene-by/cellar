#!/bin/bash
# self-update.sh — pull the latest cellar code from origin/main and verify.
#
# Runs git pull, re-runs validate-profiles.sh + cellar-doctor.sh, and
# reports the diff between the previous and new HEAD. Bails on git
# conflicts or validator failures so the user can resolve manually.
#
# Usage:
#   self-update.sh           # pull origin/main, validate, report
#   self-update.sh --check   # just print "you are N commits behind"
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$CELLAR_ROOT"

if ! git rev-parse --git-dir >/dev/null 2>&1; then
  echo "ERROR: $CELLAR_ROOT is not a git repo" >&2
  exit 1
fi

if [ "${1:-}" = "--check" ]; then
  git fetch --quiet origin main 2>/dev/null || true
  behind=$(git rev-list --count HEAD..origin/main 2>/dev/null || echo 0)
  ahead=$(git rev-list --count origin/main..HEAD 2>/dev/null || echo 0)
  if [ "$behind" = "0" ] && [ "$ahead" = "0" ]; then
    echo "up to date with origin/main"
  else
    echo "behind: $behind, ahead: $ahead"
    if [ "$behind" -gt 0 ]; then
      git log --oneline HEAD..origin/main | head -10
    fi
  fi
  exit 0
fi

# Refuse if working tree has uncommitted changes (would be lost on pull
# with merge conflicts). Stash-and-pop is too risky to do silently.
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "ERROR: uncommitted changes in working tree." >&2
  echo "commit, stash, or discard them before self-update:" >&2
  git status --short >&2
  exit 1
fi

prev_head=$(git rev-parse HEAD)
echo "==> fetching origin..."
git fetch --quiet origin main

new_head=$(git rev-parse origin/main)
if [ "$prev_head" = "$new_head" ]; then
  echo "already up to date with origin/main ($(git rev-parse --short HEAD))."
  exit 0
fi

behind=$(git rev-list --count HEAD..origin/main)
echo "==> pulling $behind commit(s)..."
if ! git merge --ff-only origin/main; then
  echo "ERROR: can't fast-forward (local commits ahead of origin)." >&2
  echo "resolve manually with git rebase or git pull --no-ff" >&2
  exit 1
fi

new_head_short=$(git rev-parse --short HEAD)
echo "==> updated to $new_head_short"
echo
echo "==> commits applied:"
git log --oneline "$prev_head".."$new_head" | head -20
echo

# Re-run validators on the new code.
echo "==> validating new state..."
if [ -x "$CELLAR_ROOT/scripts/validate-profiles.sh" ]; then
  if "$CELLAR_ROOT/scripts/validate-profiles.sh" >/dev/null 2>&1; then
    echo "  [OK] validate-profiles.sh"
  else
    echo "  [FAIL] validate-profiles.sh — new state has profile errors" >&2
    "$CELLAR_ROOT/scripts/validate-profiles.sh" >&2 || true
  fi
fi
if [ -x "$CELLAR_ROOT/scripts/cellar-doctor.sh" ]; then
  out=$("$CELLAR_ROOT/scripts/cellar-doctor.sh" 2>&1 | tail -1)
  echo "  doctor: $out"
fi

echo
echo "DONE. See CHANGELOG.md for what's new."
