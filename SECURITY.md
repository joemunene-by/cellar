# Security policy

## Supported versions

cellar is pre-1.0. The default branch (`main`) is the only supported
version; older tags are archival.

## Reporting a vulnerability

If you've found a security issue in cellar (a script that could be
tricked into running arbitrary code via crafted input, a profile that
opens a privilege-escalation path on the host Mac, etc.), please report
it privately via GitHub's [private security advisory
flow](https://github.com/joemunene-by/cellar/security/advisories/new)
rather than opening a public issue.

Expect a first response within a few days. cellar is a personal project
without a 24/7 on-call, but security reports are taken seriously.

### Scope

In scope:
- Command injection in any `scripts/*.sh` via a crafted profile id,
  game name, bottle name, or environment variable.
- Path traversal in save backup / log viewer / inspector scripts.
- Privilege escalation via wine prefix manipulation.
- Insecure handling of the user's CrossOver runtime or wine binaries.

Out of scope:
- DMCA / IP concerns about the games the user supplies. cellar is a
  launcher; the user supplies the game files. Those concerns go to the
  rights-holder via GitHub's [DMCA process](https://github.com/contact/dmca),
  not via a security advisory.
- Wine, CrossOver, Apple GPTK, D3DMetal, MoltenVK upstream issues.
  Report those to the upstream project.
- Anti-cheat circumvention. cellar does not implement or distribute any
  anti-cheat bypass code; it only documents technical failure modes.

## Hardening notes for users

- cellar runs every game in its own wine prefix under `~/.cellar/bottles/`.
  Prefixes are not sandboxed by macOS in any strong sense; assume a
  malicious game can read `~/Documents/`, `~/Pictures/`, and other dirs
  wine maps into the prefix. Don't run untrusted game binaries.
- The launchers do NOT need `sudo`. If a script asks you to elevate,
  that's a bug — please report it.
