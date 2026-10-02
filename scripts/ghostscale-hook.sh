#!/bin/bash
# Sourced by cellar game app wrappers. When the game is linked to ghostscale (cellar Settings), the launch
# is handed to ghostscale.app, which prepares the game, runs our launcher and puts its upscaler on top.
#   . ghostscale-hook.sh "<game name>" <runtime launcher>
gs_app="$HOME/Applications/ghostscale.app"
gs_profile="$(defaults read com.joemunene.ghostscale "cellar.$1" 2>/dev/null || true)"
if [ -n "$gs_profile" ] && [ -d "$gs_app" ]; then
  # open -n gives ghostscale its own process, so its Screen Recording permission applies, not ours.
  exec /usr/bin/open -n -a "$gs_app" --args play "$gs_profile" --launch "$2" \
    --log "$HOME/Library/Logs/ghostscale.log"
fi
