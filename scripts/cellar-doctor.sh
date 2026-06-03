#!/bin/bash
# cellar-doctor.sh — one-shot health check for the cellar runtime.
#
# Exits 0 if everything cellar needs to launch games is in place,
# non-zero with a summary of what's missing otherwise. No fixes; this
# is the read-only diagnostic. Anything that fails here should be
# triaged by the user, not auto-installed.
#
# Run: scripts/cellar-doctor.sh
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
fail_count=0
warn_count=0
ok_count=0

ok()   { printf "  [OK]   %s\n" "$*"; ok_count=$((ok_count + 1)); }
warn() { printf "  [WARN] %s\n" "$*"; warn_count=$((warn_count + 1)); }
fail() { printf "  [FAIL] %s\n" "$*"; fail_count=$((fail_count + 1)); }

section() { printf "\n%s\n" "$*"; }

section "host"
if [ "$(uname)" = "Darwin" ]; then
  ok "macOS ($(sw_vers -productVersion 2>/dev/null || echo unknown))"
  arch=$(uname -m)
  if [ "$arch" = "arm64" ]; then
    ok "Apple Silicon ($arch)"
  else
    warn "non-Apple-Silicon CPU detected ($arch); cellar is M-series tuned"
  fi
else
  warn "not running on macOS ($(uname)); this doctor is macOS-specific"
fi

section "rosetta 2"
if /usr/bin/pgrep -q oahd 2>/dev/null; then
  ok "Rosetta 2 daemon running"
elif [ -d /Library/Apple/usr/share/rosetta ]; then
  ok "Rosetta 2 installed (daemon not active right now)"
else
  fail "Rosetta 2 not installed; install with: softwareupdate --install-rosetta --agree-to-license"
fi

section "CrossOver runtime"
CROSSOVER="$HOME/.cellar/runtime/CrossOver.app"
if [ -d "$CROSSOVER" ]; then
  ok "CrossOver.app present at $CROSSOVER"
  wine="$CROSSOVER/Contents/SharedSupport/CrossOver/lib/wine/x86_64-unix/wine"
  wineserver="$CROSSOVER/Contents/SharedSupport/CrossOver/CrossOver-Hosted Application/wineserver"
  if [ -x "$wine" ]; then
    wver=$("$wine" --version 2>/dev/null || echo "unknown")
    ok "wine binary: $wine ($wver)"
  else
    fail "wine binary not executable at $wine"
  fi
  if [ -x "$wineserver" ]; then
    ok "wineserver binary present"
  else
    fail "wineserver binary not executable at $wineserver"
  fi
  d3dmetal="$CROSSOVER/Contents/SharedSupport/CrossOver/lib64/apple_gptk/external/D3DMetal.framework"
  if [ -d "$d3dmetal" ]; then
    info_plist="$d3dmetal/Resources/Info.plist"
    if [ -f "$info_plist" ]; then
      ver=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$info_plist" 2>/dev/null || echo "?")
      ok "D3DMetal.framework version $ver"
    else
      warn "D3DMetal.framework present but Info.plist not readable"
    fi
  else
    fail "D3DMetal.framework not found under apple_gptk/external"
  fi
else
  fail "$CROSSOVER not found; cellar runtime missing"
fi

section "command-line tools"
for cmd in jq winetricks cabextract; do
  if command -v "$cmd" >/dev/null 2>&1; then
    ok "$cmd ($(command -v "$cmd"))"
  else
    fail "$cmd not on PATH; install with: brew install $cmd"
  fi
done

section "homebrew packages (optional, profile-dependent)"
for pkg in gstreamer gst-libav; do
  if /opt/homebrew/bin/brew list "$pkg" >/dev/null 2>&1; then
    ok "brew $pkg installed"
  else
    warn "brew $pkg missing; CarX (and other Media Foundation users) need it"
  fi
done

section "cellar profile set"
if [ -f "$CELLAR_ROOT/profiles.json" ]; then
  if command -v jq >/dev/null 2>&1 && jq -e . "$CELLAR_ROOT/profiles.json" >/dev/null 2>&1; then
    n=$(jq '.profiles | length' "$CELLAR_ROOT/profiles.json")
    ok "$n profiles in profiles.json"
    if [ -x "$CELLAR_ROOT/scripts/validate-profiles.sh" ]; then
      out=$("$CELLAR_ROOT/scripts/validate-profiles.sh" 2>&1 | tail -1)
      case "$out" in
        OK*) ok "validate-profiles.sh: $out" ;;
        WARN*) warn "validate-profiles.sh: $out" ;;
        *) fail "validate-profiles.sh: $out" ;;
      esac
    fi
  else
    fail "profiles.json present but invalid"
  fi
else
  fail "profiles.json missing under $CELLAR_ROOT"
fi

section "user state"
if [ -d "$HOME/.cellar/bottles" ]; then
  bottles=$(find "$HOME/.cellar/bottles" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | wc -l | tr -d ' ')
  ok "~/.cellar/bottles exists ($bottles bottle(s))"
else
  warn "~/.cellar/bottles missing; will be created on first launch"
fi
if [ -d "$HOME/Games-source" ]; then
  games=$(find "$HOME/Games-source" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | wc -l | tr -d ' ')
  ok "~/Games-source exists ($games game dir(s))"
else
  warn "~/Games-source missing; cellar expects games dropped under there"
fi

printf "\n---\n"
printf "%d ok, %d warn, %d fail\n" "$ok_count" "$warn_count" "$fail_count"
[ $fail_count -eq 0 ]
