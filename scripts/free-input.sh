#!/bin/bash
# free-input.sh — release input devices that other apps seize, so cellar games
# get clean keyboard/controller input.
#
# Steam Input seizes USB game controllers *exclusively* at the macOS level, which
# hides them from Wine entirely (Wine's Game Controllers panel shows nothing while
# Steam is running). Quitting Steam frees the device. Safe to source into a
# launcher (defines cellar_free_input) or run standalone; no-op if Steam is closed.
#
# NOTE (macOS 15.4+): Apple's GameController framework *separately* grabs generic
# USB controllers and maps buttons to system UI (Select -> Launchpad), starving
# the raw HID that Wine games need. That's an Apple-side regression this can't
# fix — keyboard + mouse is the reliable input path for cellar games for now.

cellar_free_input() {
  local log="${1:-/dev/null}"
  # CELLAR_KEEP_STEAM=1 keeps Steam running on purpose (Steam Input method:
  # Steam seizes the pad and maps it to keyboard/mouse for the Wine game).
  if [ "${CELLAR_KEEP_STEAM:-0}" = "1" ]; then
    echo "free-input: CELLAR_KEEP_STEAM=1 - leaving Steam running for Steam Input" >> "$log"
    return 0
  fi
  if pgrep -x Steam >/dev/null 2>&1 || pgrep -x steam_osx >/dev/null 2>&1; then
    echo "free-input: quitting Steam so it releases seized controllers..." >> "$log"
    osascript -e 'tell application "Steam" to quit' >/dev/null 2>&1
  fi
}

# Run it if executed directly rather than sourced.
if [ "${BASH_SOURCE[0]:-$0}" = "$0" ]; then
  cellar_free_input "/dev/stdout"
fi
