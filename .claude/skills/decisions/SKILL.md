---
name: decisions
description: Walk the owner through open `decision` issues as quick multiple-choice prompts (works well from mobile), then record each answer on its issue and close it. Use when the owner asks to review, answer or go through pending decisions, or types /decisions. Optional args: issue numbers or a label/area filter (e.g. "power", "411").
---

# Answer open decisions

Owner questions live as GitHub issues labelled `decision` (docs/process/work-tracking.md,
"Decisions"). Each body has `## Question`, `## Options` (bullets, each starting with a bold
option name, the recommended one ending in **Recommended.**), `## Where this comes from` and
`## Blocks`.

## 1. Gather

```sh
gh issue list --label decision --state open --limit 50 --json number,title,labels,body
```

If args were given, keep only those issue numbers, or issues whose title, labels or parent match
the filter. Order by priority label (p0 first), then by design parent, so related questions come
together. If none are open, say so and stop.

## 2. Ask

Use **AskUserQuestion**, up to 4 questions per call, grouped by design parent. Keep asking in
rounds until every selected decision has been asked. For each issue:

- `header`: the issue number plus 1–2 words, at most 12 chars (e.g. `#458 Keys`).
- `question`: the `## Question` text, one sentence, ending in `?`.
- `options`: one per bullet under `## Options`, 2–4 of them, with the recommended option
  **first** and its label ending in ` (Recommended)`. `label` is the bold option name, shortened
  to 1–5 words. `description` is the trade-off sentence, trimmed to fit a phone screen.
- If the issue has only one real option plus "do nothing", keep both. If it has more than 4,
  keep the recommended one and the 3 most distinct others, and say in the description which
  ones were folded.
- Leave `multiSelect` off unless the question explicitly allows several answers.

Don't add your own commentary between rounds. The owner can pick **Other** to type a free
answer or "skip".

## 3. Record

For each answer:

- **An option picked**: comment on the issue:
  `Owner decision (YYYY-MM-DD): <option label>. <any note the owner added>`. Then close it:
  `gh issue close N --reason completed`.
- **Other with a real answer** (not one of the options): comment with the owner's words verbatim
  as the decision, close it, and flag it in the summary. The design doc and the gated issues may
  need more than a one-line update.
- **"skip", "later" or an empty Other**: leave it open and untouched.

Then, for each design parent named in `## Blocks`, check whether any of its `decision` issues
are still open (`gh api repos/gizmo385/four-nations/issues/P/dependencies/blocked_by --jq
'map(select(.state=="open")) | map(.number)'`). If none are, and it still has `needs-owner`,
remove that label and comment `All open decisions answered: #A, #B …`.

## 4. Report

One line per decision: `#N <short question>: <answer>`, marking which were not the
recommendation or were Other. Then list what's now unblocked: the issues from each `## Blocks`
list whose `blocked_by` has no open issues left. End by saying that the design docs still need
the answers written in. The orchestrator hands that to the doc's designer, or does it in the
next triage pass. Don't edit design docs or code from this skill.
