#!/bin/bash
# fps-hud.sh — show/hide the on-screen FPS + frametime overlay (Apple's Metal HUD)
# for cellar games.
#
# Persists the choice to ~/.cellar/fps-hud, which every cellar launcher reads as
# the default. The CELLAR_METAL_HUD env var still overrides per-launch
# (CELLAR_METAL_HUD=0 / =1).
#
# Usage:
#   fps-hud.sh on        # show the overlay for cellar games
#   fps-hud.sh off       # hide it
#   fps-hud.sh toggle    # flip it
#   fps-hud.sh status    # print the current state
#
# NOTE: the overlay is fixed when the game process starts, so a change takes
# effect on the NEXT launch — relaunch the game to apply. It cannot be toggled
# mid-game (Apple's HUD has no runtime hotkey).
set -u
STATE="$HOME/.cellar/fps-hud"
cur() { cat "$STATE" 2>/dev/null || echo 1; }
set_state() { mkdir -p "$(dirname "$STATE")"; echo "$1" > "$STATE"; }

case "${1:-status}" in
  on)     set_state 1; echo "FPS overlay: ON  (relaunch the game to apply)" ;;
  off)    set_state 0; echo "FPS overlay: OFF (relaunch the game to apply)" ;;
  toggle) if [ "$(cur)" = "1" ]; then set_state 0; echo "FPS overlay: OFF (relaunch to apply)";
          else set_state 1; echo "FPS overlay: ON  (relaunch to apply)"; fi ;;
  status) [ "$(cur)" = "1" ] && echo "FPS overlay: ON" || echo "FPS overlay: OFF" ;;
  *)      echo "usage: fps-hud.sh on|off|toggle|status" >&2; exit 1 ;;
esac
