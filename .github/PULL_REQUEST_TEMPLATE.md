<!-- Thanks for the PR! Quick checklist before submit. -->

## What

<!-- one or two sentences -->

## Why

<!-- the user-loop friction this addresses -->

## Checklist

- [ ] `scripts/validate-profiles.sh` passes locally (if profiles.json touched)
- [ ] `bash -n` clean on any shell script touched
- [ ] CHANGELOG entry added under `## Unreleased`
- [ ] README updated if a new script / profile / launcher / env var is user-facing
- [ ] No scene-release-group names introduced as prescriptive sources (see `CONTRIBUTING.md` for the line)
- [ ] No `winemenubuilder.exe=d` or `=disabled` (wine grammar is `n`, `b`, or empty)

## How tested

<!-- doctor / validator / actual launch on what game / etc. -->
