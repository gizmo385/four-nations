#!/usr/bin/env bash
# CI only (.github/workflows/ci.yml): prints `docs` when every file a change touches is a
# document, else `full`. A `docs` change runs `cargo xtask ci --docs-only` (the tests that read
# docs/) instead of the whole gate: nothing that is compiled, linted or run changed.
#
#   scripts/ci-scope.sh < changed-files     # one repo-relative path per line
#
# A document is a file under docs/, a Markdown file at the repository root, or an issue
# template. Nothing else is: Markdown inside crates/, data/ or xtask/ can be compiled into a
# binary or read by a test. An empty list is `full`, so a change this can't see runs everything.
set -euo pipefail
scope=docs
seen=0
while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    seen=1
    case "$f" in
        docs/*) ;;
        .github/ISSUE_TEMPLATE/*) ;;
        */*) scope=full ;;
        *.md) ;;
        *) scope=full ;;
    esac
done
[[ $seen == 1 ]] || scope=full
echo "$scope"
