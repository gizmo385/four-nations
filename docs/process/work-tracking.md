# Work tracking

Status: **proposal** (FN-0004).

## Recommendation: in-repo Markdown work items

Each work item is a file at `work/items/FN-0007-short-slug.md` with TOML frontmatter (see
`work/TEMPLATE.md`). The `cargo xtask work` command lists, filters, validates and allocates them.

Why in-repo rather than GitHub Issues, Linear or similar:

| | In-repo files (proposed) | GitHub Issues/Projects | `beads`-style agent tracker |
|---|---|---|---|
| Agent access | Plain file reads/edits; works offline and in sandboxes | Needs `gh`, network and auth | Needs an extra CLI |
| Status changes reviewed with the code | **Yes**: same PR | No | Partly |
| Works before we push anything | **Yes** | No | Yes |
| Human UX (boards, notifications) | Weak (CLI + editor) | **Strong** | Weak |
| Parallel-agent conflicts | Possible id collisions (detected by `work check`) | None | Handled |

If we later want boards and notifications, we can add a one-way sync from `work/items` to GitHub
Issues. The files stay the source of truth.

## Lifecycle

```
proposed ──► ready ──► in-progress ──► review ──► done
    │          │            │
    └──────────┴────────────┴──► dropped (record why in Notes)
```

- **proposed**: an idea. It may lack acceptance criteria. Agents don't pick these up.
- **ready**: has concrete acceptance criteria and its dependencies are known. `work next`
  shows ready items whose dependencies are all `done`.
- **in-progress**: claimed. `owner` must be set (the branch name).
- **review**: code is done and a PR or branch is waiting for a human.
- **done**: merged. Only a human, or an agent the human explicitly told to merge, sets this.

Only humans move items from `proposed` to `ready`. That's how we steer what agents work on.

## Claiming protocol (for parallel agents)

1. Run `cargo xtask work next` and pick the top item you can do.
2. Create the branch `work/FN-0007-short-slug` from the current mainline.
3. **First commit on the branch**: set `status = "in-progress"` and `owner = "work/FN-0007-..."`.
   Nothing else goes in that commit.
4. Work. Append dated entries to the item's **Notes**: decisions, dead ends, anything the next
   agent needs.
5. Tick the acceptance criteria as they become true, set `status = "review"`, and make sure
   `cargo xtask ci` passes.
6. If you discover new work, create a new item with `cargo xtask work new "..."` in `proposed`.
   Don't expand the current item's scope.

New ids are allocated as max+1, so two branches can collide. `work check` catches this at
merge time, and the later branch renumbers.

## Granularity

An item should fit in one focused agent session or one evening of human work: roughly 1 PR,
reviewable in under 30 minutes. If it doesn't, split it and use `depends_on`.
