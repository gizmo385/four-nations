# Work tracker

One Markdown file per work item in `items/`, with TOML frontmatter. The tracker lives in the repo
so that a work item's status changes in the *same commit/PR* as the code that satisfies it.

See [docs/process/work-tracking.md](../docs/process/work-tracking.md) for the lifecycle and
claiming rules. Quick reference:

```sh
cargo xtask work next              # what can I pick up?
cargo xtask work list              # everything, by priority
cargo xtask work list in-progress  # who is doing what
cargo xtask work new "Title"       # allocate the next id from TEMPLATE.md
cargo xtask work check             # validate (also run by `cargo xtask ci`)
```
