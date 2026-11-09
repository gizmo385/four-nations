#!/usr/bin/env bash
# CI only (.github/workflows/ci.yml): lets Cargo reuse a cached target/ for the workspace crates
# whose sources haven't changed since the cache was built.
#
# Cargo judges a workspace crate fresh by mtime: it rebuilds when a source file is newer than
# the crate's last build. A fresh checkout stamps every file with the checkout time, which is
# newer than any cached artifact, so on its own a cached target/ only saves the dependencies.
# (scripts/warm-target.sh has the opposite problem locally and solves it by touching everything.)
#
# The workflow writes the commit a cache was built from to target/.cache-commit before saving
# it. Here, every tracked file is stamped with an old fixed time, and then the files that differ
# from that commit are stamped with the current time again. Unchanged crates come out older
# than their cached artifacts (fresh, reused); a crate with a changed file, or one that depends
# on it, comes out newer (rebuilt). data/*.toml is compiled into fn_sim with include_str!, so
# Cargo's dep-info tracks it the same way.
#
# With no cache, no recorded commit, or a commit git can't fetch, it changes nothing, and every
# workspace crate rebuilds as before. It never makes a changed file look old, so the worst case
# is a rebuild, never a stale build.
set -euo pipefail
say() { echo "ci-cache-mtimes: $*" >&2; }

stamp=target/.cache-commit
if [[ ! -f "$stamp" ]]; then say "no $stamp (no cache restored); building the workspace from scratch"; exit 0; fi
cached="$(tr -d '[:space:]' < "$stamp")"
if ! [[ "$cached" =~ ^[0-9a-f]{40}$ ]]; then say "$stamp holds no commit id; leaving mtimes"; exit 0; fi

# The checkout is shallow: fetch the cached commit's tree to diff against.
if ! git cat-file -e "$cached^{tree}" 2>/dev/null \
    && ! git fetch --quiet --depth=1 origin "$cached" 2>/dev/null; then
    say "can't fetch $cached; leaving mtimes, so the workspace rebuilds"
    exit 0
fi

changed="$(git diff --name-only --no-renames "$cached" HEAD)"
old=@946684800 # 2000-01-01: older than any artifact a cache can hold.
git ls-files -z | xargs -0 touch -h -d "$old"
# Directories too, and git's HEAD and refs: fn_tui's build script (the build id, #566) reruns
# when any of them looks newer than its last run, and a rerun rebuilds fn_tui and everything
# above it. A directory with a changed file in it still reruns it: cargo scans a directory's
# files, not just its own mtime. When the build id's paths are unchanged, the binary keeps the
# cached build's id: harmless on CI, where nothing built is published and the tests compare
# against the same compiled-in id.
git ls-files | awk -F/ '{ d = "."; print d; for (i = 1; i < NF; i++) { d = d "/" $i; print d } }' \
    | sort -u | xargs -d '\n' touch -h -d "$old"
find "$(git rev-parse --git-dir)" -maxdepth 1 \( -name HEAD -o -name packed-refs \) -exec touch -h -d "$old" {} +
find "$(git rev-parse --git-dir)/refs" -exec touch -h -d "$old" {} +
if [[ -n "$changed" ]]; then
    # Deleted files don't exist to touch; Cargo already treats a missing source as changed.
    while IFS= read -r f; do [[ -e "$f" ]] && touch -h "$f"; done <<< "$changed"
fi
say "cache built from $cached; $(grep -c . <<< "$changed" || true) files changed since"
