#!/usr/bin/env bash
#
# setup-gptk.sh
#
# One-time install of the cellar runtime stack: Homebrew, Rosetta 2,
# Apple's Game Porting Toolkit, and a GPTK-patched Wine.
#
# Safe to run on a fresh Mac. Re-running this script after a previous
# successful install is a no-op for each step.

set -euo pipefail

log() { printf '\033[36m[cellar]\033[0m %s\n' "$*"; }
err() { printf '\033[31m[cellar]\033[0m %s\n' "$*" >&2; }

# ---------- 1. Apple Silicon check ----------

arch="$(uname -m)"
if [ "$arch" != "arm64" ]; then
  err "cellar targets Apple Silicon (arm64). This machine reports '$arch'."
  err "GPTK works only on M-series Macs."
  exit 1
fi
log "running on Apple Silicon ($arch). good."

# ---------- 2. macOS version check ----------

os_major="$(sw_vers -productVersion | cut -d. -f1)"
if [ "$os_major" -lt 14 ]; then
  err "GPTK requires macOS 14 (Sonoma) or newer. You are on $(sw_vers -productVersion)."
  exit 1
fi
log "macOS $(sw_vers -productVersion) is new enough."

# ---------- 3. Rosetta 2 ----------

if pgrep -q oahd || [ -f /Library/Apple/usr/share/rosetta/rosetta ]; then
  log "Rosetta 2 already installed."
else
  log "installing Rosetta 2 (sudo prompt incoming)."
  softwareupdate --install-rosetta --agree-to-license
fi

# ---------- 4. Xcode CLT ----------

if xcode-select -p >/dev/null 2>&1; then
  log "Xcode Command Line Tools present."
else
  log "installing Xcode Command Line Tools. A dialog will pop; finish it and rerun this script."
  xcode-select --install || true
  exit 1
fi

# ---------- 5. Homebrew ----------

if command -v brew >/dev/null 2>&1; then
  log "Homebrew present at $(command -v brew)."
else
  log "installing Homebrew."
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  # Add to PATH for this session.
  eval "$(/opt/homebrew/bin/brew shellenv)"
fi

# Ensure brew is on PATH for the rest of the script.
if [ -d /opt/homebrew/bin ]; then
  export PATH="/opt/homebrew/bin:$PATH"
fi

# ---------- 6. GPTK-patched wine64 ----------
#
# Two acceptable sources, in preference order:
#   (a) Apple's homebrew formula: apple/apple/game-porting-toolkit
#   (b) Whisky.app, which bundles a GPTK-patched wine64 internally
#
# (a) is the canonical path but its formula depends on openssl@1.1,
# which Homebrew has dropped. On affected Macs the install will fail
# with a "No available formula with the name openssl@1.1" warning. We
# attempt it anyway and fall back to (b) on failure.

whisky_wine="$HOME/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/Wine/bin/wine64"

gptk_ok=0
if brew list game-porting-toolkit >/dev/null 2>&1; then
  log "Game Porting Toolkit already installed via brew."
  gptk_ok=1
else
  log "tapping apple/apple and trying to install game-porting-toolkit."
  brew tap apple/apple http://github.com/apple/homebrew-apple 2>/dev/null || true
  if brew install apple/apple/game-porting-toolkit 2>&1 | tee /tmp/cellar-gptk-install.log; then
    log "game-porting-toolkit formula installed."
    gptk_ok=1
  else
    log "game-porting-toolkit formula install failed (likely openssl@1.1 dep dropped from Homebrew)."
    log "checking for Whisky as a fallback runtime."
  fi
fi

if [ "$gptk_ok" != "1" ] && [ ! -x "$whisky_wine" ]; then
  log "Whisky not installed. Installing it now (provides the same GPTK-patched wine64)."
  brew install --cask whisky
fi

# ---------- 7. Verify wine64 ----------

wine_bin=""
for cand in \
  /opt/homebrew/opt/game-porting-toolkit/bin/wine64 \
  /usr/local/opt/game-porting-toolkit/bin/wine64 \
  "$whisky_wine" \
  /opt/homebrew/bin/wine64; do
  if [ -x "$cand" ]; then
    wine_bin="$cand"
    break
  fi
done

if [ -z "$wine_bin" ]; then
  err "wine64 not found after install. Neither the GPTK formula nor Whisky shipped one."
  err "Manual fix: install Whisky from https://getwhisky.app or pin openssl@1.1 yourself."
  exit 1
fi

log "wine64 ready at $wine_bin"
log "version: $("$wine_bin" --version 2>/dev/null || echo unknown)"

# ---------- 8. Done ----------

cat <<'EOF'

[cellar] setup complete. Next:

  npm install
  cargo tauri dev

If wine64 is not where cellar expects (we probe both
/opt/homebrew/opt/game-porting-toolkit/bin/wine64 and
/usr/local/opt/game-porting-toolkit/bin/wine64), set CELLAR_WINE to its
absolute path before launching:

  export CELLAR_WINE=/path/to/your/wine64

EOF
