#!/usr/bin/env bash
# Seeds a fresh git worktree's target/ with a copy of the main checkout's ci-profile build, so the first build in
# the worktree reuses already-compiled dependencies (Bevy above all) instead of rebuilding them.
# Cargo stores fingerprint paths relative to the package or target root, so a moved target/ stays
# valid for registry crates.
#
# Workspace crates are the trap: Cargo judges them fresh by mtime, and the copied artifacts (built
# from the main checkout's sources) are usually newer than this worktree's edits. Left alone, Cargo
# would silently test main's code. So after copying we touch every workspace file, which forces
# the workspace crates to rebuild while registry dependencies stay cached.
#
#   scripts/warm-target.sh    # run from inside the worktree; does nothing if target/ exists
#
# The copy is a copy-on-write clone where the filesystem supports it (APFS via `cp -c`, btrfs/XFS
# via `--reflink`), so it is near-instant and costs no disk until files diverge. Elsewhere it falls
# back to a plain copy, which is still much faster than compiling Bevy.
set -euo pipefail
here="$(git rev-parse --show-toplevel)"
main="$(git worktree list --porcelain | awk 'NR == 1 { print $2 }')"
say() { echo "warm-target: $*" >&2; }

if [[ "$here" == "$main" ]]; then say "in the main checkout; nothing to do"; exit 0; fi
if [[ -e "$here/target" ]]; then say "$here/target already exists; leaving it"; exit 0; fi
# Only the `ci` profile: agents build with `cargo xtask ci ... --fast`, and the main checkout's
# dev profile (target/debug) is several times bigger and would be copied for nothing.
if [[ ! -d "$main/target/ci" ]]; then say "no $main/target/ci to copy; building cold"; exit 0; fi

mkdir -p "$here/target"
# -p keeps mtimes, so Cargo's mtime checks see the same ordering as in the original.
if ! cp -cRp "$main/target/ci" "$here/target/ci" 2>/dev/null; then
  rm -rf "$here/target/ci" # a partial clone; we checked above that target/ didn't exist before
  cp -a --reflink=auto "$main/target/ci" "$here/target/ci"
fi
git -C "$here" ls-files -z --cached --others --exclude-standard \
  | (cd "$here" && xargs -0 touch)
say "seeded target/ci from $main/target/ci; workspace crates will rebuild"
