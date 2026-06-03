#!/bin/bash
# cellar-logs.sh — quick browser for cellar launcher logs.
#
# All cellar launch scripts write to /tmp/*.log with a predictable prefix
# (carxstreet-*, fifa*, nfsmw*, cellar-<bottle>). This tool lists them
# sorted by recency, and lets you tail or open one without remembering
# the exact name.
#
# Usage:
#   cellar-logs.sh                # list logs
#   cellar-logs.sh <name|index>   # tail -f the matching log
#   cellar-logs.sh open <name>    # `open -e` (TextEdit) the matching log
#   cellar-logs.sh prune          # delete logs older than 7 days
set -u

list_logs() {
  # Match all known cellar log prefixes. Order by mtime, newest first.
  find /tmp -maxdepth 1 -type f \( \
      -name 'cellar-*.log' -o \
      -name 'fifa*.log' -o \
      -name 'carxstreet*.log' -o \
      -name 'nfsmw*.log' -o \
      -name 'rdr2*.log' -o \
      -name 'skyrim*.log' \
    \) -print 2>/dev/null \
    | xargs -I{} stat -f "%m %z %N" {} 2>/dev/null \
    | sort -rn
}

show_list() {
  printf "%-5s %-19s %10s  %s\n" "IDX" "MODIFIED" "SIZE" "PATH"
  printf "%-5s %-19s %10s  %s\n" "---" "---" "---" "---"
  local i=0
  while IFS= read -r line; do
    [ -z "$line" ] && continue
    mtime=$(echo "$line" | awk '{print $1}')
    size=$(echo "$line" | awk '{print $2}')
    path=$(echo "$line" | awk '{$1=""; $2=""; sub(/^  */,""); print}')
    when=$(date -r "$mtime" "+%Y-%m-%d %H:%M" 2>/dev/null || echo "?")
    sz_human=$(echo "$size" | awk '{
      s = $1;
      if (s < 1024) printf "%dB", s;
      else if (s < 1048576) printf "%.1fK", s/1024;
      else printf "%.1fM", s/1048576;
    }')
    printf "%-5d %-19s %10s  %s\n" "$i" "$when" "$sz_human" "$path"
    i=$((i + 1))
  done
}

resolve_one() {
  # Find a log matching the user's argument. Accept either:
  #   - a numeric index into the listing (0 = most recent)
  #   - a substring of the basename
  local arg="$1"
  local logs=()
  while IFS= read -r line; do
    [ -z "$line" ] && continue
    logs+=("$(echo "$line" | awk '{$1=""; $2=""; sub(/^  */,""); print}')")
  done < <(list_logs)

  if [[ "$arg" =~ ^[0-9]+$ ]]; then
    if [ "$arg" -lt "${#logs[@]}" ]; then
      echo "${logs[$arg]}"
      return 0
    fi
    echo "index $arg out of range (have ${#logs[@]} log(s))" >&2
    return 1
  fi
  # Substring match against basename.
  for log in "${logs[@]}"; do
    base=$(basename "$log")
    case "$base" in
      *"$arg"*) echo "$log"; return 0 ;;
    esac
  done
  echo "no log matched '$arg'" >&2
  return 1
}

case "${1:-}" in
  ""|list)
    list_logs | show_list
    ;;
  open)
    if [ -z "${2:-}" ]; then
      echo "usage: $0 open <name|index>" >&2
      exit 1
    fi
    target=$(resolve_one "$2") || exit 1
    echo "opening $target"
    open -e "$target"
    ;;
  prune)
    n=$(find /tmp -maxdepth 1 -type f \( \
        -name 'cellar-*.log' -o -name 'fifa*.log' -o \
        -name 'carxstreet*.log' -o -name 'nfsmw*.log' -o \
        -name 'rdr2*.log' -o -name 'skyrim*.log' \
      \) -mtime +7 -print 2>/dev/null | tee /dev/stderr | wc -l | tr -d ' ')
    if [ "$n" -gt 0 ]; then
      find /tmp -maxdepth 1 -type f \( \
        -name 'cellar-*.log' -o -name 'fifa*.log' -o \
        -name 'carxstreet*.log' -o -name 'nfsmw*.log' -o \
        -name 'rdr2*.log' -o -name 'skyrim*.log' \
      \) -mtime +7 -delete 2>/dev/null
      echo "pruned $n log(s) older than 7 days"
    else
      echo "no logs older than 7 days to prune"
    fi
    ;;
  *)
    target=$(resolve_one "$1") || exit 1
    echo "tailing $target (Ctrl-C to stop)..."
    tail -f "$target"
    ;;
esac
