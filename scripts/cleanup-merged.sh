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
set -euo pipefail

apply=0
case "${1:-}" in
  --apply) apply=1 ;;
  "") ;;
  *) echo "usage: $0 [--apply]" >&2; exit 2 ;;
esac

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
