# Work tracking and merging

Decided 2026-10-21. Work is tracked in **GitHub Issues** on `gizmo385/four-nations` (a private
repo with one human owner). Agents use the `gh` CLI.

## Labels

| Group | Labels | Rule |
|---|---|---|
| Area | `area:sim` `area:client` `area:data` `area:art` `area:tooling` `area:design` | One or more per issue |
| Feature group | `feat:mapgen` `feat:units` `feat:jobs` `feat:bending` `feat:persistence` `feat:rendering` `feat:dev-tools` | Exactly one. Add a new `feat:*` label when a new feature area starts |
| Priority | `p0` (blocking) `p1` (next) `p2` (soon) `p3` (someday) | Exactly one |
| Workflow | `ready` `in-progress` `large` | See below |

- **`ready`**: only the owner adds this. It means "agents may pick this up". Issues without it
  are ideas or need more detail.
- **`in-progress`**: claimed. The claim comment says by whom and where.
- **`large`**: must use the PR flow (below).

## Dependencies

Use GitHub's **issue relationships** ("Blocked by" / "Blocking" in the issue sidebar), not
labels or body text. An issue counts as blocked while any issue blocking it is still open. When
the blocker closes it's unblocked automatically, with nothing to clean up.

```sh
gh issue create ... --blocked-by 4,5              # new issue that depends on #4 and #5
gh issue edit 7 --add-blocked-by 5                # #7 can't start until #5 is done
gh issue edit 7 --remove-blocked-by 5
gh issue list --search "is:open is:blocked"       # what's waiting on something
gh api repos/gizmo385/four-nations/issues/4/dependencies/blocking --jq 'map(.number)'  # what #4 unblocks
```

## Finding work

```sh
gh issue list --search "is:open label:ready -label:in-progress -is:blocked" --json number,title,labels \
  --jq 'sort_by(.labels | map(.name) | map(select(test("^p[0-3]$")))[0]) | .[] | "#\(.number) \(.title)"'
```

Never pick up a blocked issue, even if it's labelled `ready`.

## Claiming

All agents run as the owner's GitHub account, so assignees can't tell agents apart. Claim with
a label plus a comment instead:

```sh
gh issue view N --json labels   # re-check that nobody claimed it a moment ago
gh issue edit N --add-label in-progress
gh issue comment N --body "Claimed by <agent/session>. Flow: direct-to-main | PR from issue/N-slug"
```

Unclaim if you stop without finishing: remove `in-progress` and leave a handoff comment.

## Two ways to land work

### Direct to `main` (default for small and medium changes)

Use this when **all** of these hold:
- one agent (or the owner) is working on this area right now;
- the change is not labelled `large`;
- it touches roughly < 400 changed lines, adds no new dependency, and does not change a public
  `fn_sim` API used by the client in a breaking way.

Steps:

1. `git pull --rebase origin main`
2. Make focused commits. The subject starts with the issue number, e.g. `#2: Add A* pathfinding`.
   The last commit's body has `Fixes #2`.
3. `cargo xtask ci` must pass.
4. `git pull --rebase origin main && cargo xtask ci && git push origin main`. Never force-push.
5. GitHub closes the issue from `Fixes #N`. Add a closing comment with anything notable.

### Branch + PR (multiple agents in parallel, or large changes)

Use this when **any** of these hold:
- more than one agent is working at the same time (each agent uses its own branch, or worktree);
- the issue is labelled `large`, or turns out to be (> ~400 lines, new dependencies, Bevy
  upgrades, golden or save-format changes, cross-crate API changes);
- you're unsure. When in doubt, use a PR.

Steps:

1. Branch `issue/N-short-slug` from `origin/main`.
2. Commit as above, and push the branch.
3. `gh pr create --fill`, with `Closes #N` in the body. Fill in what changed, how it was
   verified, and anything the reviewer should look at.
4. The owner reviews and merges (squash or rebase). Agents don't merge their own PRs unless
   told to in the session.

If you started direct-to-main and the change grows, move it to a branch and switch to the PR flow.

## Creating issues

Found work outside your current scope? File it instead of expanding scope:

```sh
gh issue create --template work-item.md --title "..." --label area:sim,feat:units,p2 --blocked-by 4
```

Don't add `ready` yourself. Keep issues small: one focused session, reviewable in under
30 minutes. Split bigger work and link the pieces with `--blocked-by`.

## Handoff

Before stopping, whether you finished or not, comment on the issue: what's done, what's next,
and anything surprising. The next agent starts from that comment.
