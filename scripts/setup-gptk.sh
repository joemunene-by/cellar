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

# ---------- 6. Apple GPTK formula ----------

if brew list game-porting-toolkit >/dev/null 2>&1; then
  log "Game Porting Toolkit already installed via brew."
else
  log "tapping apple/apple and installing game-porting-toolkit. This pulls a sizeable Wine build."
  brew tap apple/apple http://github.com/apple/homebrew-apple
  brew install apple/apple/game-porting-toolkit
fi

# ---------- 7. Verify wine64 ----------

wine_bin=""
for cand in \
  /opt/homebrew/opt/game-porting-toolkit/bin/wine64 \
  /usr/local/opt/game-porting-toolkit/bin/wine64 \
  /opt/homebrew/bin/wine64; do
  if [ -x "$cand" ]; then
    wine_bin="$cand"
    break
  fi
done

if [ -z "$wine_bin" ]; then
  err "wine64 not found after install. Check 'brew --prefix game-porting-toolkit'."
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
