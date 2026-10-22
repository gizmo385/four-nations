+++
id = "FN-0009"
title = "Player commands: designate chop/mine/build from the client"
status = "proposed"
priority = "p1"
area = "client"
depends_on = ["FN-0008"]
owner = ""
created = 2026-10-21
+++

## Acceptance criteria
- [ ] Sim exposes a `Command` enum; the client only mutates the world via `World::apply`.
- [ ] Click/drag to designate tiles; designated tiles are highlighted.
- [ ] Commands are recorded so a session can be replayed deterministically (test proves it).
