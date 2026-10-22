+++
id = "FN-0016"
title = "True headless screenshot mode for agents"
status = "proposed"     # proposed | ready | in-progress | review | done | dropped
priority = "p1"         # p0 (blocking) .. p3 (someday)
area = "tooling"            # sim | client | data | art | tooling | design | docs
depends_on = []         # e.g. ["FN-0003"]
owner = ""              # set when claimed: branch name, e.g. "work/FN-0007-pathfinding"
created = 2026-10-21
+++

## Why
`FN_SCREENSHOT` still opens a real window (now unfocused and kept at the bottom), which interrupts
the human. Agents need to check visuals without any window.

## Acceptance criteria
- [ ] Render to an offscreen image target with no window (Bevy `WindowPlugin { primary_window: None }`
      plus a camera targeting an `Image`), write PNG, exit.
- [ ] `cargo xtask screenshot SEED NATION OUT.png` wraps it.

## Notes
- 2026-10-21: Requested after the owner's focus kept getting stolen during the spike.
