# Bash completion for the cellar CLI (bin/cellar).
#
# Install:
#   source /path/to/cellar/completions/cellar.bash
# Or system-wide:
#   ln -s /path/to/cellar/completions/cellar.bash /etc/bash_completion.d/cellar

_cellar_complete() {
  local cur prev cmd
  COMPREPLY=()
  cur="${COMP_WORDS[COMP_CWORD]}"
  prev="${COMP_WORDS[COMP_CWORD-1]}"
  cmd="${COMP_WORDS[1]:-}"

  local commands="doctor find install launch app inspect logs analyze \
                  crash backup watch d3dmetal validate profiles \
                  uninstall setup version help \
                  fifa rdr2 skyrim bethesda re re-engine capcom \
                  ubisoft anvilnext redengine cdpr forza forzatech \
                  pes efootball"

  # First positional arg: command name.
  if [ "$COMP_CWORD" -eq 1 ]; then
    COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
    return 0
  fi

  # Per-command completion for the second arg.
  case "$cmd" in
    install|launch|app)
      # Second arg: profile id. Pull from profiles.json via jq if
      # available, else hardcoded fallback for offline machines.
      local cellar_root="$(dirname "$(dirname "$(command -v cellar)")")"
      local profiles_file="$cellar_root/profiles.json"
      local profiles=""
      if command -v jq >/dev/null 2>&1 && [ -f "$profiles_file" ]; then
        profiles=$(jq -r '.profiles[].id' "$profiles_file" 2>/dev/null | tr '\n' ' ')
      fi
      [ -z "$profiles" ] && profiles="carx-street nfs-most-wanted-2005 fifa-14-23 frostbite-multi rage-rockstar d3d9-classic unreal-engine-4-5 re-engine anvilnext-ubisoft redengine bethesda-creation forzatech pes-foxengine unity-il2cpp-2022"
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "$profiles" -- "$cur") )
        return 0
      fi
      ;;
    inspect|backup|d3dmetal|uninstall)
      # Second arg: bottle name. Enumerate from ~/.cellar/bottles/.
      if [ "$COMP_CWORD" -eq 2 ]; then
        local bottles=""
        if [ -d "$HOME/.cellar/bottles" ]; then
          bottles=$(find "$HOME/.cellar/bottles" -maxdepth 1 -mindepth 1 -type d -exec basename {} \; 2>/dev/null | tr '\n' ' ')
        fi
        COMPREPLY=( $(compgen -W "$bottles" -- "$cur") )
        return 0
      fi
      ;;
    fifa)
      # Versions 14 through 23.
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "14 15 16 17 18 19 20 21 22 23" -- "$cur") )
        return 0
      fi
      ;;
    logs)
      # logs subcommands or log basename.
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "list open prune" -- "$cur") )
      fi
      return 0
      ;;
  esac

  # Default: fall through to filename completion.
  COMPREPLY=( $(compgen -f -- "$cur") )
}

complete -F _cellar_complete cellar
