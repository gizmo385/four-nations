+++
id = "FN-0011"
title = "Save and load games"
status = "proposed"
priority = "p2"
area = "sim"
depends_on = ["FN-0009"]
owner = ""
created = 2026-10-21
+++

## Acceptance criteria
- [ ] World serializes with serde to a versioned format; round-trip test.
- [ ] Decide snapshot vs. seed+command-log (FN-0009 makes the latter cheap).
