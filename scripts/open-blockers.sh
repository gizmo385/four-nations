#!/usr/bin/env bash
# List the issues still blocking issue N, counting local main as well as GitHub.
#
#   scripts/open-blockers.sh N     # prints one blocker per line; empty output means unblocked
#
# A blocker counts as done when GitHub has closed it, or when the local `main` already has its
# commit: a subject starting `#B: `, or a `Fixes #B` / `Closes #B` line. In batch mode, merged
# work sits on local main until the owner pushes, and GitHub only closes the issue on that push.
set -euo pipefail

n=${1:?usage: $0 ISSUE}
log=$(git log main --format='%s%n%b')

gh api "repos/gizmo385/four-nations/issues/$n/dependencies/blocked_by" \
  --jq '.[] | select(.state == "open") | .number' |
  while read -r b; do
    if grep -qE "^#$b: |^(Fixes|Closes) #$b\b" <<<"$log"; then
      continue
    fi
    echo "$b"
  done
