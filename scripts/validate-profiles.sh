#!/bin/bash
# validate-profiles.sh — sanity-check profiles.json before shipping.
#
# Catches the class of bug surfaced in the engine-family audit:
#   - WINEDLLOVERRIDES with wine-grammar-invalid tokens (e.g. =disabled, =d)
#   - winetricks_X entries that don't match a known verb
#   - missing required JSON fields per profile
#   - duplicate ids or empty match_name_contains
#   - launch_args containing flags that aren't valid for the engine
#
# Run from the repo root: scripts/validate-profiles.sh
# Exits non-zero on any failure so it can be wired into CI.
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILES="$CELLAR_ROOT/profiles.json"

if ! command -v jq >/dev/null 2>&1; then
  echo "ERROR: jq not found, install with: brew install jq" >&2
  exit 2
fi
if [ ! -f "$PROFILES" ]; then
  echo "ERROR: $PROFILES not found" >&2
  exit 2
fi
if ! jq -e . "$PROFILES" >/dev/null 2>&1; then
  echo "ERROR: $PROFILES is not valid JSON" >&2
  exit 1
fi

# Known winetricks verbs we use across profiles. Update when adding new
# entries to a profile's `requires`. Anything in `requires` starting with
# winetricks_ but not in this set is flagged as a typo or unsupported verb.
known_verbs=(
  vcrun2019 vcrun2022 corefonts d3dcompiler_47 dotnet48 mf
  msxml3 msxml6 wmp10 wmp11 quartz dxvk
)

# Wine WINEDLLOVERRIDES load-order tokens that are valid per the wine
# grammar. Empty string (disabled), n (native), b (builtin), and the
# combinations of n/b separated by commas. The literal token "disabled"
# is NOT in the grammar; the literal "d" is silently ignored (lutris #1183).
valid_overrides_re='^(n|b|nb|bn|n,b|b,n|)$'

fail_count=0
warn_count=0
fail() { echo "FAIL: $*" >&2; fail_count=$((fail_count + 1)); }
warn() { echo "WARN: $*" >&2; warn_count=$((warn_count + 1)); }

# 1. Each profile has required fields. Use `has()` rather than `jq -e` because
#    -e treats boolean false (e.g. dxvk: false) as failure, which would give
#    a false positive for any profile with DXVK off.
while IFS= read -r id; do
  [ -z "$id" ] && continue
  profile=$(jq -c ".profiles[] | select(.id == \"$id\")" "$PROFILES")
  for field in id name match_name_contains description settings requires; do
    has=$(jq -r "has(\"$field\")" <<< "$profile")
    if [ "$has" != "true" ]; then
      fail "$id: missing field '$field'"
    fi
  done
  for field in dxvk esync msync metal_fences metal_hud dll_overrides env launch_args; do
    has=$(jq -r ".settings | has(\"$field\")" <<< "$profile")
    if [ "$has" != "true" ]; then
      fail "$id: missing settings.$field"
    fi
  done
done < <(jq -r '.profiles[].id' "$PROFILES")

# 2. No duplicate ids.
dup=$(jq -r '.profiles[].id' "$PROFILES" | sort | uniq -d)
if [ -n "$dup" ]; then
  fail "duplicate profile id(s): $dup"
fi

# 3. Each non-fallback profile has at least one match_name_contains entry.
#    (The two existing fallbacks `unity-il2cpp-2022` and any future ones can
#    have empty match arrays.)
while IFS= read -r id; do
  [ -z "$id" ] && continue
  count=$(jq ".profiles[] | select(.id == \"$id\") | .match_name_contains | length" "$PROFILES")
  if [ "$count" -eq 0 ]; then
    case "$id" in
      unity-il2cpp-2022) ;; # known fallback
      *) warn "$id: empty match_name_contains, will never auto-apply" ;;
    esac
  fi
done < <(jq -r '.profiles[].id' "$PROFILES")

# 4. dll_overrides syntax: each ; separated entry must have key=value form,
#    with value matching valid_overrides_re. Catches =disabled, =d, =native
#    (long form), and other invalid tokens.
while IFS= read -r id; do
  [ -z "$id" ] && continue
  overrides=$(jq -r ".profiles[] | select(.id == \"$id\") | .settings.dll_overrides // \"\"" "$PROFILES")
  [ -z "$overrides" ] && continue
  IFS=';' read -ra entries <<< "$overrides"
  for entry in "${entries[@]}"; do
    [ -z "$entry" ] && continue
    if [[ "$entry" != *=* ]]; then
      fail "$id: dll_overrides entry has no '=': $entry"
      continue
    fi
    val="${entry#*=}"
    if ! [[ "$val" =~ $valid_overrides_re ]]; then
      fail "$id: dll_overrides value '$val' (in '$entry') is not valid wine grammar"
      fail "  (allowed: n, b, n,b, b,n, or empty for disabled)"
    fi
  done
done < <(jq -r '.profiles[].id' "$PROFILES")

# 5. Every requires entry of form winetricks_X must have X in the known list.
while IFS= read -r id; do
  [ -z "$id" ] && continue
  while IFS= read -r r; do
    [ -z "$r" ] && continue
    case "$r" in
      winetricks_*)
        verb="${r#winetricks_}"
        found=0
        for k in "${known_verbs[@]}"; do
          [ "$verb" = "$k" ] && found=1 && break
        done
        if [ $found -eq 0 ]; then
          warn "$id: unknown winetricks verb '$verb' (in '$r')"
          warn "  (add to known_verbs[] in validate-profiles.sh if intentional)"
        fi
        ;;
      proton_winrt_dlls|homebrew_*) ;;
      *)
        warn "$id: requires entry '$r' has no recognized handler"
        ;;
    esac
  done < <(jq -r ".profiles[] | select(.id == \"$id\") | .requires[]?" "$PROFILES")
done < <(jq -r '.profiles[].id' "$PROFILES")

# 6. launch_args specific lints.
while IFS= read -r id; do
  [ -z "$id" ] && continue
  args=$(jq -c ".profiles[] | select(.id == \"$id\") | .settings.launch_args" "$PROFILES")
  [ "$args" = "[]" ] && continue
  # Elden Ring / Hogwarts specifically can't use -dx11. Flag any profile
  # whose match list intersects those titles that still has -dx11 set.
  if echo "$args" | jq -e '. | index("-dx11")' >/dev/null 2>&1; then
    matches=$(jq -r ".profiles[] | select(.id == \"$id\") | .match_name_contains | join(\",\")" "$PROFILES")
    if [[ "$matches" == *"elden ring"* || "$matches" == *"hogwarts legacy"* ]]; then
      fail "$id: launch_args includes -dx11 but match list contains Elden Ring or Hogwarts Legacy (both are DX12-only at engine build)"
    fi
  fi
done < <(jq -r '.profiles[].id' "$PROFILES")

echo "---"
total=$(jq '.profiles | length' "$PROFILES")
if [ $fail_count -eq 0 ] && [ $warn_count -eq 0 ]; then
  echo "OK: $total profiles, no failures, no warnings."
  exit 0
elif [ $fail_count -eq 0 ]; then
  echo "OK with warnings: $total profiles, 0 failures, $warn_count warning(s)."
  exit 0
else
  echo "FAIL: $total profiles, $fail_count failure(s), $warn_count warning(s)."
  exit 1
fi
