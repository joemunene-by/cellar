#!/bin/bash
# find-profile.sh — given a game name, suggest which cellar profile would match.
#
# Walks profiles.json, finds the first profile whose match_name_contains
# array has a case-insensitive substring match against the input. Prints
# the profile id + name + the suggested cellar-install / launch commands.
# Falls through to a "no exact match" message if nothing matches, with the
# list of profile ids to choose from manually.
#
# Usage:
#   find-profile.sh "<game name>"
#
# Examples:
#   find-profile.sh "FIFA 19"
#   find-profile.sh "Need for Speed Heat"
#   find-profile.sh "Grand Theft Auto V"
#   find-profile.sh "Some Obscure Indie"
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILES="$CELLAR_ROOT/profiles.json"
GAME="${1:?game name required (use quotes for multi-word names)}"

if ! command -v jq >/dev/null 2>&1; then
  echo "jq missing, install with: brew install jq" >&2
  exit 2
fi

needle=$(echo "$GAME" | tr 'A-Z' 'a-z')
matches=()
while IFS= read -r line; do
  [ -z "$line" ] && continue
  pid=$(echo "$line" | jq -r '.id')
  found=0
  while IFS= read -r alias; do
    [ -z "$alias" ] && continue
    case "$needle" in
      *"$alias"*) found=1; break ;;
    esac
  done < <(echo "$line" | jq -r '.match_name_contains[]?')
  [ $found -eq 1 ] && matches+=("$pid")
done < <(jq -c '.profiles[]' "$PROFILES")

if [ ${#matches[@]} -eq 0 ]; then
  echo "No profile matched '$GAME'."
  echo
  echo "Available profile ids (pick one manually):"
  jq -r '.profiles[] | "  " + .id + " — " + .name' "$PROFILES"
  echo
  echo "Then either:"
  echo "  scripts/cellar-install.sh <profile-id> \"$GAME\""
  echo "  scripts/launch-engine.sh <profile-id> \"$GAME\""
  exit 1
fi

# Print primary match plus any others (e.g. a game that overlaps two
# profile match lists). First one is the canonical pick.
primary="${matches[0]}"
primary_name=$(jq -r ".profiles[] | select(.id == \"$primary\") | .name" "$PROFILES")
primary_desc=$(jq -r ".profiles[] | select(.id == \"$primary\") | .description" "$PROFILES")
echo "Best match: $primary"
echo "  name: $primary_name"
echo "  description (first sentence):"
# Split on literal ". " using sed; if no period, fall through to the whole
# description. awk's FS='. ' treats . as regex any-char and breaks early.
first_sentence=$(echo "$primary_desc" | sed 's/\. .*$/./' | head -c 240)
echo "    $first_sentence"
echo
if [ ${#matches[@]} -gt 1 ]; then
  echo "Other matches:"
  for i in $(seq 1 $((${#matches[@]} - 1))); do
    echo "  ${matches[$i]}"
  done
  echo
fi
echo "Suggested commands:"
echo "  scripts/cellar-install.sh $primary \"$GAME\""
echo "  scripts/launch-engine.sh $primary \"$GAME\""
echo "  scripts/make-cellar-app.sh $primary \"$GAME\""
