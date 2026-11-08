#!/usr/bin/env bash
# Checks scripts/cleanup-merged.sh against a throwaway repo (never the real one).
#
#   scripts/test-cleanup-merged.sh
#   TEST_BASH=/bin/bash scripts/test-cleanup-merged.sh   # pick the interpreter (macOS: /bin/bash is 3.2)
#
# 1. Lints the script for bash-4-only syntax, since macOS ships bash 3.2.
# 2. Builds a repo with a fresh, an old, a dirty, an unmerged and a process-holding
#    worktree, and checks the dry-run verdicts with both process checks (/proc and lsof).
# 3. Runs --apply in the throwaway repo and checks only the old worktree went.
#
# This does not prove bash 3.2 compatibility on its own (the lint only catches the
# features it knows about); run it with TEST_BASH=/bin/bash on a Mac too.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
script=$here/cleanup-merged.sh
interp=${TEST_BASH:-bash}
fails=0
fail() { echo "FAIL: $*" >&2; fails=$((fails + 1)); }

# --- 1. bash-4 lint -------------------------------------------------------
lint() {
  local pattern=$1 what=$2
  if grep -nE -- "$pattern" "$script" | grep -v '^[0-9]*:[[:space:]]*#' >&2; then
    fail "bash-4 feature in cleanup-merged.sh: $what"
  fi
}
lint 'declare[[:space:]]+-[a-zA-Z]*A|typeset[[:space:]]+-[a-zA-Z]*A|local[[:space:]]+-[a-zA-Z]*A' 'associative array'
lint '(mapfile|readarray)([[:space:]]|$)' 'mapfile/readarray'
lint '\$\{[A-Za-z_0-9]+(,,?|\^\^?)[^}]*\}' 'case-modifying expansion'
lint '\|&|&>>|(^|[[:space:];])coproc[[:space:]]' 'bash-4 redirection or coproc'
lint '\[\[[[:space:]]+-v[[:space:]]' '[[ -v var ]]'
lint 'local[[:space:]]+-n[[:space:]]|declare[[:space:]]+-n[[:space:]]' 'namerefs'
lint ';;&|;&' 'case fall-through'
lint 'globstar|\*\*/' 'globstar'
lint 'read[[:space:]].*-i[[:space:]]|printf[[:space:]]+-v[[:space:]]+[A-Za-z_]+\[' 'read -i / printf -v array'
grep -q 'stat -f' "$script" || fail "no BSD stat fallback"

# --- 2. throwaway repo ----------------------------------------------------
tmp=$(mktemp -d)
pids=""
cleanup() {
  [[ -n $pids ]] && kill $pids 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT

# touch -t STAMP for "3 hours ago", on GNU and BSD date.
three_hours_ago() { date -d '3 hours ago' +%Y%m%d%H%M 2>/dev/null || date -v-3H +%Y%m%d%H%M; }

repo=$tmp/repo
mkdir "$repo"
cd "$repo"
git init -q
git config user.email t@example.com
git config user.name t
git commit -q --allow-empty -m init
git branch -M main
mkdir -p .claude/worktrees
for n in fresh old old2 dirty unmerged busy busy-lsof; do
  git worktree add -q ".claude/worktrees/$n" -b "worktree-agent-$n" main
done
for n in old old2 dirty unmerged busy busy-lsof; do
  touch -t "$(three_hours_ago)" ".claude/worktrees/$n"
done
echo x >.claude/worktrees/dirty/file
git -C .claude/worktrees/unmerged commit -q --allow-empty -m "work that never landed"

# Processes whose cwd is inside busy and busy-lsof. A sibling prefix (old2 vs old) must not
# make "old" look busy, so one sleeper sits in old2/sub.
mkdir .claude/worktrees/old2/sub
for d in busy busy-lsof old2/sub; do
  (cd ".claude/worktrees/$d" && exec sleep 600) &
  pids="$pids $!"
done
cd "$repo"
# Give the subshells a moment to exec; the cwd is already set, so this only avoids a fork race.
sleep 1

check() {
  local method=$1 out
  out=$(CLEANUP_PROCS=$method "$interp" "$script" 2>&1) || { fail "[$method] script exited non-zero: $out"; return; }
  want() { grep -qE -- "$1" <<<"$out" || fail "[$method] expected /$1/ in:
$out"; }
  want "^KEEP +\.claude/worktrees/fresh \[worktree-agent-fresh\]: fresh"
  want "^REMOVE +\.claude/worktrees/old \[worktree-agent-old\]: no commits ahead of main"
  want "^KEEP +\.claude/worktrees/old2 \[worktree-agent-old2\]: processes running inside"
  want "^KEEP +\.claude/worktrees/dirty \[worktree-agent-dirty\]: uncommitted changes"
  want "^KEEP +\.claude/worktrees/unmerged \[worktree-agent-unmerged\]: 1 commits not on main"
  want "^KEEP +\.claude/worktrees/busy \[worktree-agent-busy\]: processes running inside"
  want "^KEEP +\.claude/worktrees/busy-lsof \[worktree-agent-busy-lsof\]: processes running inside"
  want "^would remove 1, keep 6"
}
check proc
check lsof

# --- 3. --apply in the throwaway repo --------------------------------------
"$interp" "$script" --apply >/dev/null 2>&1 || fail "--apply exited non-zero"
[[ ! -e .claude/worktrees/old ]] || fail "--apply left the old worktree"
git rev-parse -q --verify refs/heads/worktree-agent-old >/dev/null && fail "--apply left the old branch"
for n in fresh old2 dirty unmerged busy busy-lsof; do
  [[ -d .claude/worktrees/$n ]] || fail "--apply removed .claude/worktrees/$n"
done

if [[ $fails == 0 ]]; then echo "ok: cleanup-merged.sh ($interp)"; else echo "$fails failure(s)" >&2; exit 1; fi
