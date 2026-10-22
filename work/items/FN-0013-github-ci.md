+++
id = "FN-0013"
title = "GitHub Actions CI running cargo xtask ci"
status = "proposed"
priority = "p2"
area = "tooling"
depends_on = ["FN-0004"]
owner = ""
created = 2026-10-21
+++

## Acceptance criteria
- [ ] Workflow runs `cargo xtask ci` on PRs; caches target/.
- [ ] Only lands after FN-0004 decides branch layout (current workflow deploys cljs to gh-pages).
