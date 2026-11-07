# Work tracking and merging

Decided 2026-10-21. Work is tracked in **GitHub Issues** on `gizmo385/four-nations` (a private
repo with one human owner). Agents use the `gh` CLI.

## Labels

| Group | Labels | Rule |
|---|---|---|
| Area | `area:sim` `area:client` `area:data` `area:tooling` `area:design` | One or more per issue |
| Feature group | `feat:mapgen` `feat:units` `feat:jobs` `feat:bending` `feat:persistence` `feat:rendering` `feat:dev-tools` | Exactly one. Add a new `feat:*` label when a new feature area starts |
| Priority | `p0` (blocking) `p1` (next) `p2` (soon) `p3` (someday) | Exactly one |
| Workflow | `ready` `needs-owner` `in-progress` `large` | See below |
| Decision | `decision` | One owner question per issue; see "Decisions" below |

- **`ready`**: the issue is fully specified and has no open questions for the owner, so an
  agent may pick it up **once it is unblocked**. Ready says nothing about ordering;
  dependencies handle that. The owner or the orchestrator's triage pass (below) adds it.
- **`needs-owner`**: the issue contains a decision only the owner can make. It is never marked
  ready until the owner answers and removes the label.
- **`in-progress`**: claimed. The claim comment says by whom and where.
- **`large`**: never direct to `main`; use the batch or PR flow (below).

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

## Decisions

Every question only the owner can answer is **its own issue**, labelled `decision` and titled
`Decision: <the question>`. The body has the options with their trade-offs, the recommended one
marked **Recommended**, a link to the design doc section, and a "Blocks:" list. Each issue the
answer gates is linked as **blocked by** the decision, so triage and dispatch skip it on their
own, and closing the decision unblocks it.

- Designers and implementers file a `decision` issue instead of only writing the question into a
  doc or a comment, and link it as a blocker of the parent and of every follow-up it gates.
- The owner's queue is `gh issue list --label decision --state open`.
- The owner answers with a comment. An agent records the answer in the design doc, adjusts the
  gated issues if needed, and closes the decision.

## Triage and sequencing

The orchestrator (the main Claude session that dispatches agents) runs a **triage pass** when
issues are filed, after each merge, and whenever the owner asks. For every open issue without
`ready`, it marks the issue `ready` only if **all** of these hold:

1. **Concrete acceptance criteria**: observable, testable outcomes, not "implement X".
2. **No owner decisions**: no `needs-owner` label, no open `decision` blocking it, and nothing
   in the body or comments asks the owner to choose (style, tone, scope, balance targets that need taste). Technical choices an
   agent can make and justify in its PR (e.g. which algorithm, which data layout) don't count.
3. **Dependencies are recorded**: anything it obviously needs first is linked as "blocked by".
   Ready issues can still be blocked; they become pickable automatically when their blockers
   close.
4. **Sized for one PR**, or labelled `large`.

If an issue fails 2, the orchestrator files a `decision` issue for the question (or links the
existing one as a blocker) and lists it in its next update.
If it fails 1, 3 or 4, the orchestrator fixes the issue (rewrites criteria, adds links, splits
it) and then re-checks. Every triage pass ends with a short note to the owner listing what was
marked ready and what is waiting on them. The owner can remove `ready` at any time to hold
something back.

**Dispatch.** The orchestrator then starts agents on the top unblocked ready issues, by priority.
It avoids running two issues at once that are likely to edit the same files (same `feat:*` group
or the same module), so parallel branches rarely conflict. As work merges, newly unblocked issues
become pickable and the loop continues.

**Agents.** The orchestrator dispatches project agents from `.claude/agents/`, each with the
repo rules built into its prompt, so dispatch prompts stay one line ("Implement #3; #18 runs in
parallel"):

| Agent | Model | Use for |
|---|---|---|
| `implementer` | Sonnet | `ready` code, data and tooling issues; in batch mode, through a local commit |
| `designer` | Opus | `area:design` issues: design doc plus follow-up issues; owner questions become `decision` issues |
| `integrator` | Sonnet | Batch mode: merges finished branches into local `main`, re-blesses goldens, runs the gate once |
| `pr-shepherd` | Haiku | Getting an existing PR merged: auto-merge, CI watch, trivial rebases |
| `triager` | Sonnet | The triage pass above; returns a dispatch plan |

Use `general-purpose` only for one-off work that fits none of these.

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

## Three ways to land work

`main` is protected: the `ci` GitHub Actions check must pass before a PR can merge. Direct
pushes by the owner's account aren't blocked, which is what makes the direct flow possible. So
running `cargo xtask ci --fast` locally before pushing is mandatory.

### Direct to `main` (default for small and medium changes)

Use this when **all** of these hold:
- one agent (or the owner) is working on this area right now;
- the change is not labelled `large`;
- it touches roughly < 400 changed lines, adds no new dependency, and does not change a public
  `fn_sim` or `fn_ui` API in a breaking way.

Steps:

1. `git pull --rebase origin main`
2. Make focused commits. The subject starts with the issue number, e.g. `#2: Add A* pathfinding`.
   The last commit's body has `Fixes #2`.
3. `cargo xtask ci --fast` must pass.
4. `git pull --rebase origin main && cargo xtask ci --fast && git push origin main`. Never
   force-push.
5. GitHub closes the issue from `Fixes #N`. Add a closing comment with anything notable.

### Batch (default when the orchestrator runs agents in parallel)

Decided 2026-11-06. Parallel agents commit on their own branches and never push; an integrator
merges finished branches into the **local** `main` in batches; the owner pushes `main`. This
replaces a PR, CI run and rebase per issue with one merge, one golden re-bless and one gate per
batch.

**Agents (implementer, designer):**

1. Claim as usual, with `Flow: batch` in the claim comment.
2. Branch from the **local** `main`, never `origin/main`: `git switch -C issue/N-slug main`
   (it can be ahead of `origin`). Then `scripts/warm-target.sh`.
3. Check only what you touched: `cargo xtask check` (fmt, clippy for the workspace, and tests
   for the crates you changed and the crates that depend on them, with golden comparisons
   skipped). Run it once when you're done.
4. **Don't commit golden changes.** Golden frames are generated output in batch mode. If you
   ran `cargo xtask bless` to look at a frame, put the files back before committing
   (`git checkout -- '*tests/golden*'`), and paste the frame into your report instead.
5. Commit with `Fixes #N` in the body. Write the message for `main`'s history: the integrator
   squashes your branch into one commit that reuses it. Don't push and don't open a PR. Leave `in-progress` on;
   the integrator clears it after the merge.
6. Report the branch, the commit, what changed and how you checked it.

**Integrator** (`.claude/agents/integrator.md`), run by the orchestrator when 2–3 branches are
waiting, or at the end of a session:

1. Stop if the main checkout has uncommitted changes. It never stashes or commits the owner's
   work.
2. Squash each branch onto `main` (`git merge --squash`, then one commit that reuses the
   branch's message, `#N: ...` with `Fixes #N`), in the order given. History stays linear, one
   commit per issue, with no merge commits. On a conflict in a golden
   file, take either side. Resolve small conflicts that keep both sides' intent (doc comments,
   test expectations). For anything else, abort that merge and hand the branch back.
3. `cargo xtask bless`, and commit any golden changes as one batch commit.
4. `cargo xtask ci --fast` once. If it fails, find the branch responsible and hand it back,
   or fix a trivial failure (fmt, a stale expectation) itself.
5. Remove the merged worktrees and their branches, clear `in-progress`, and sweep for leftover
   processes.

The owner pushes `main`, and GitHub closes the issues from `Fixes #N`. An issue stays open with
`in-progress` cleared between the local merge and the push.

### Branch + PR (solo `large` work, or when the owner asks)

Use this when **any** of these hold:
- the issue is labelled `large` and no batch is running;
- the owner asks for a PR;
- you're unsure and not in a batch. When in doubt, ask the orchestrator.

The PR flow exists to **prevent conflicts**, not to gate on human review. The agent that
opens a PR **owns it until it merges**:

1. Branch `issue/N-short-slug` from `origin/main`.
2. Commit as above, run `cargo xtask ci --fast`, and push the branch.
3. `gh pr create --fill`, with `Closes #N` in the body. Fill in what changed, how it was
   verified, and anything the owner should look at.
4. Turn on auto-merge right away: `gh pr merge --auto --squash`. GitHub merges once the required
   `ci` check passes and the branch is up to date with `main`. Merging is squash-only and merged
   branches are deleted automatically.
5. Watch it through: `gh pr checks --watch`.
   - **CI failed**: fix it, push, repeat.
   - **Out of date with `main`**: `gh pr update-branch --rebase`, or rebase locally and push
     with `--force-with-lease` (allowed only on your own PR branch).
   - **Conflicts**: rebase onto `origin/main` and resolve them. Keep both sides' intent, rerun
     `cargo xtask ci --fast`, and push. If the resolution needs a design decision, comment on the
     PR, turn off auto-merge (`gh pr merge --disable-auto`) and tell the owner.
6. Once merged, check that the issue closed, and leave a short closing comment if anything is
   notable.

Exceptions, where the agent turns off auto-merge and asks the owner instead: the PR changes
`AGENTS.md`, `docs/process/`, CI/workflow files or repo settings, or it needs a design call the
issue doesn't settle. The owner may also comment "hold" on any PR.

If you started direct-to-main and the change grows, move it to a branch and switch to the PR flow.

## Creating issues

Found work outside your current scope? File it instead of expanding scope:

```sh
gh issue create --template work-item.md --title "..." --label area:sim,feat:units,p2 --blocked-by 4
```

Don't add `ready` to issues you file; the next triage pass decides. Keep issues small: one focused session, reviewable in under
30 minutes. Split bigger work and link the pieces with `--blocked-by`.

## Waiting on background work

Run long commands (the gate, a balance run) in the foreground, or start them in the background
and wait for the completion notice. Never poll with `until`/`while` loops around `pgrep`,
`sleep` or `[ -s file ]`: they match other agents' processes (or their own shell) and can spin
for hours. A hook (`.claude/hooks/no-polling-loops.sh`) rejects such commands.

## Handoff

Before stopping, whether you finished or not, comment on the issue: what's done, what's next,
and anything surprising. The next agent starts from that comment.
