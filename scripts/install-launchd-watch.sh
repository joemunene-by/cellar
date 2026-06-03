#!/bin/bash
# install-launchd-watch.sh — register watch-games.sh as a macOS launchd agent.
#
# Once installed, launchd starts watch-games.sh at login + restarts it on
# crash, so new game dirs under ~/Games-source/ get notification +
# profile-match suggestion automatically without needing a terminal open.
#
# Usage:
#   install-launchd-watch.sh         # install + start
#   install-launchd-watch.sh --stop  # stop + uninstall
#   install-launchd-watch.sh --status
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WATCH="$CELLAR_ROOT/scripts/watch-games.sh"
PLIST_LABEL="dev.cellar.watch-games"
PLIST_PATH="$HOME/Library/LaunchAgents/$PLIST_LABEL.plist"
LOG_OUT="/tmp/cellar-watch-out.log"
LOG_ERR="/tmp/cellar-watch-err.log"

case "${1:-install}" in
  install|"")
    if [ ! -x "$WATCH" ]; then
      echo "ERROR: $WATCH not found / not executable" >&2
      exit 1
    fi
    if ! command -v fswatch >/dev/null 2>&1; then
      echo "ERROR: fswatch missing, install with: brew install fswatch" >&2
      exit 2
    fi

    mkdir -p "$HOME/Library/LaunchAgents"
    cat > "$PLIST_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$PLIST_LABEL</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/bash</string>
        <string>$WATCH</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOG_OUT</string>
    <key>StandardErrorPath</key>
    <string>$LOG_ERR</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>
    </dict>
</dict>
</plist>
EOF
    # Load it. -w persists across reboots.
    launchctl unload "$PLIST_PATH" 2>/dev/null || true
    launchctl load -w "$PLIST_PATH"
    echo "installed: $PLIST_PATH"
    echo "stdout log: $LOG_OUT"
    echo "stderr log: $LOG_ERR"
    ;;

  uninstall|--stop|stop)
    if [ ! -f "$PLIST_PATH" ]; then
      echo "not installed: $PLIST_PATH" >&2
      exit 1
    fi
    launchctl unload -w "$PLIST_PATH" 2>/dev/null || true
    rm -f "$PLIST_PATH"
    echo "uninstalled: $PLIST_PATH"
    ;;

  --status|status)
    if launchctl list 2>/dev/null | grep -q "$PLIST_LABEL"; then
      echo "running"
      launchctl list "$PLIST_LABEL" 2>/dev/null | head -20
    else
      echo "not running"
    fi
    if [ -f "$PLIST_PATH" ]; then
      echo "plist: $PLIST_PATH"
    fi
    ;;

  *)
    echo "usage: $0 [install|uninstall|status]" >&2
    exit 1
    ;;
esac
