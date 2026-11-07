#!/usr/bin/env bash
# Remove agent worktrees and branches whose work is already on main.
#
#   scripts/cleanup-merged.sh            # dry run: list what would go and why
#   scripts/cleanup-merged.sh --apply    # remove it
#
# Run from the main checkout. Looks at worktrees under .claude/worktrees/ and at
# `issue/*` and `worktree-agent-*` branches. A worktree goes only when it has no
# uncommitted changes, no process running inside it, and its branch is on main.
# A branch is on main when it has no commits ahead of main, when every commit has a
# patch-equivalent on main, when every commit subject appears on main (the integrator
# squashes with `git commit -C`, which keeps the subject), or, for `issue/N`, when main
# has a `#N:` commit after the branch point and the issue isn't labelled `in-progress`.
# Everything else is kept and reported. Never touches main or the main checkout.
#
# A worktree with no commits ahead of main is usually just-created (an agent hasn't
# committed yet), not actually merged, so it's kept while its directory is younger
# than FRESH_MINUTES. Pass --include-fresh to remove those too.
#
# Manual check (never run --apply against the real repo; use a throwaway one):
#   d=$(mktemp -d); cd "$d" && git init -q && git commit -q --allow-empty -m init \
#     && git branch -M main && mkdir -p .claude/worktrees \
#     && git worktree add -q .claude/worktrees/fresh -b worktree-agent-fresh main \
#     && git worktree add -q .claude/worktrees/old -b worktree-agent-old main \
#     && touch -d "3 hours ago" .claude/worktrees/old \
#     && /path/to/scripts/cleanup-merged.sh
#   Expect: KEEP ... fresh [...]: fresh (created 0m ago) and
#           REMOVE ... old [...]: no commits ahead of main
set -euo pipefail

FRESH_MINUTES=120

apply=0
include_fresh=0
for arg in "$@"; do
  case "$arg" in
    --apply) apply=1 ;;
    --include-fresh) include_fresh=1 ;;
    *) echo "usage: $0 [--apply] [--include-fresh]" >&2; exit 2 ;;
  esac
done

if [[ "$(git rev-parse --git-dir)" != "$(git rev-parse --git-common-dir)" ]]; then
  echo "run this from the main checkout, not a worktree" >&2
  exit 2
fi
root=$(git rev-parse --show-toplevel)
cd "$root"

main_subjects=$(git log --format=%s main)

# Prints why BRANCH counts as on main, or nothing if it doesn't.
on_main() {
  local b=$1 ahead n
  ahead=$(git rev-list --count "main..$b")
  if [[ $ahead == 0 ]]; then echo "no commits ahead of main"; return; fi
  if ! git cherry main "$b" | grep -q '^+'; then echo "all $ahead commits are on main"; return; fi
  local missing=0 s
  while IFS= read -r s; do
    grep -qxF -- "$s" <<<"$main_subjects" || missing=1
  done < <(git log --format=%s "main..$b")
  if [[ $missing == 0 ]]; then echo "squashed onto main (subjects match)"; return; fi
  if [[ $b =~ ^issue/([0-9]+)- ]]; then
    n=${BASH_REMATCH[1]}
    local base
    base=$(git merge-base main "$b")
    if git log --format=%s "$base..main" | grep -q "^#$n: " \
      && ! gh issue view "$n" --json labels --jq '.labels[].name' 2>/dev/null | grep -qx in-progress; then
      echo "main has a #$n: commit and #$n isn't in progress"
      return
    fi
  fi
}

# Prints DIR's age in whole minutes, from its mtime, or nothing if unreadable.
age_minutes() {
  local dir=$1 modified
  modified=$(stat -c %Y "$dir" 2>/dev/null) || return
  echo $(( ($(date +%s) - modified) / 60 ))
}

# Prints the PIDs of processes whose working directory is inside DIR.
procs_in() {
  local dir=$1 p
  for p in $(pgrep -u "$USER" || true); do
    if [[ $(readlink "/proc/$p/cwd" 2>/dev/null) == "$dir"* ]]; then echo "$p"; fi
  done
}

act() { if [[ $apply == 1 ]]; then "$@"; fi; }

removed=0 kept=0
declare -A seen gone

echo "== worktrees"
while read -r wt branch; do
  [[ $wt == "$root/.claude/worktrees/"* ]] || continue
  branch=${branch#refs/heads/}
  seen[$branch]=1
  name=${wt#"$root/"}
  if [[ -n $(git -C "$wt" status --porcelain 2>/dev/null) ]]; then
    echo "KEEP   $name [$branch]: uncommitted changes"; kept=$((kept + 1)); continue
  fi
  pids=$(procs_in "$wt" | tr '\n' ' ')
  if [[ -n $pids ]]; then
    echo "KEEP   $name [$branch]: processes running inside (${pids% })"; kept=$((kept + 1)); continue
  fi
  if [[ $include_fresh == 0 && $(git rev-list --count "main..$branch") == 0 ]]; then
    age=$(age_minutes "$wt")
    if [[ -n $age && $age -lt $FRESH_MINUTES ]]; then
      echo "KEEP   $name [$branch]: fresh (created ${age}m ago)"; kept=$((kept + 1)); continue
    fi
  fi
  why=$(on_main "$branch")
  if [[ -z $why ]]; then
    echo "KEEP   $name [$branch]: $(git rev-list --count "main..$branch") commits not on main"
    kept=$((kept + 1)); continue
  fi
  echo "REMOVE $name [$branch]: $why"
  gone[$wt]=1
  act git worktree unlock "$wt" 2>/dev/null || true
  act git worktree remove --force "$wt"
  act git branch -D "$branch" >/dev/null
  removed=$((removed + 1))
done < <(git worktree list --porcelain | awk '/^worktree /{w=$2} /^branch /{print w, $2}')

echo "== branches without a worktree"
while read -r b; do
  [[ -n ${seen[$b]:-} ]] && continue
  git worktree list --porcelain | grep -qx "branch refs/heads/$b" && continue
  wt="$root/.claude/worktrees/agent-${b#worktree-agent-}"
  if [[ $b == worktree-agent-* && -d $wt && -z ${gone[$wt]:-} ]]; then
    echo "KEEP   $b: its agent's worktree is kept"; kept=$((kept + 1)); continue
  fi
  why=$(on_main "$b")
  if [[ -z $why ]]; then
    echo "KEEP   $b: $(git rev-list --count "main..$b") commits not on main"; kept=$((kept + 1)); continue
  fi
  echo "REMOVE $b: $why"
  act git branch -D "$b" >/dev/null
  removed=$((removed + 1))
done < <(git branch --list 'issue/*' 'worktree-agent-*' --format='%(refname:short)')

act git worktree prune
if [[ $apply == 1 ]]; then
  echo "removed $removed, kept $kept"
else
  echo "would remove $removed, keep $kept (dry run; pass --apply to remove)"
fi
