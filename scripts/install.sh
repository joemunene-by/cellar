#!/bin/bash
# install.sh — one-command setup: GPTK runtime, `cellar` on PATH, completions.
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINDIR="/usr/local/bin"
ZSH_COMP_DIR="/usr/local/share/zsh/site-functions"
ACTION="install"
RUN_RUNTIME=1

while [ $# -gt 0 ]; do
  case "$1" in
    --uninstall)    ACTION="uninstall" ;;
    --skip-runtime) RUN_RUNTIME=0 ;;
    --bindir)       BINDIR="${2:?--bindir needs a path}"; shift ;;
    -h|--help)
      echo "usage: install.sh [--uninstall] [--skip-runtime] [--bindir DIR]"
      exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
  shift
done

log()  { printf '\033[36m[cellar]\033[0m %s\n' "$*"; }
err()  { printf '\033[31m[cellar]\033[0m %s\n' "$*" >&2; }

# sudo only when the target (or its parent, if it doesn't exist) isn't writable
sudo_for() {
  local dir="$1"
  if { [ -d "$dir" ] && [ -w "$dir" ]; } || { [ ! -e "$dir" ] && [ -w "$(dirname "$dir")" ]; }; then
    echo ""
  else
    echo "sudo"
  fi
}

uninstall() {
  local link="$BINDIR/cellar" comp="$ZSH_COMP_DIR/_cellar"
  if [ -L "$link" ]; then $(sudo_for "$BINDIR") rm -f "$link" && log "removed $link"; fi
  if [ -e "$comp" ]; then $(sudo_for "$ZSH_COMP_DIR") rm -f "$comp" && log "removed $comp"; fi
  log "left the runtime, bottles, and saves under ~/.cellar untouched."
  exit 0
}

[ "$ACTION" = "uninstall" ] && uninstall

if [ "$(uname -m)" != "arm64" ]; then
  err "cellar needs Apple Silicon (arm64); this Mac reports $(uname -m)."
  exit 1
fi

if [ "$RUN_RUNTIME" -eq 1 ]; then
  if [ -d "$HOME/.cellar/runtime/CrossOver.app" ]; then
    log "GPTK runtime already present, skipping setup (re-run scripts/setup-gptk.sh to update)."
  else
    log "setting up the GPTK runtime (Homebrew, Rosetta, Game Porting Toolkit, Wine)."
    bash "$CELLAR_ROOT/scripts/setup-gptk.sh" || { err "setup-gptk.sh failed; fix the issue above and re-run."; exit 1; }
  fi
fi

link="$BINDIR/cellar"
s="$(sudo_for "$BINDIR")"
$s mkdir -p "$BINDIR"
$s ln -sfn "$CELLAR_ROOT/bin/cellar" "$link"
log "linked cellar -> $link"

if [ -f "$CELLAR_ROOT/completions/_cellar" ]; then
  s="$(sudo_for "$ZSH_COMP_DIR")"
  $s mkdir -p "$ZSH_COMP_DIR"
  $s cp -f "$CELLAR_ROOT/completions/_cellar" "$ZSH_COMP_DIR/_cellar"
  log "installed zsh completion to $ZSH_COMP_DIR/_cellar"
fi

if ! command -v cellar >/dev/null 2>&1; then
  err "$BINDIR is not on your PATH. Add it, e.g.: echo 'export PATH=\"$BINDIR:\$PATH\"' >> ~/.zshrc"
fi

log "verifying with cellar doctor:"
"$link" doctor || true

cat <<EOF

cellar is installed. Try:
  cellar find "<game name>"          find the matching engine profile
  cellar install <profile> "<game>"  set up the bottle
  cellar app <profile> "<game>"      make a clickable .app
  cellar help                        full command list
EOF
