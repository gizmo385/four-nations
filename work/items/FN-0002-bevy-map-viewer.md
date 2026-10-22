+++
id = "FN-0002"
title = "Bevy client that renders the generated map and wandering units"
status = "review"
priority = "p0"
area = "client"
depends_on = ["FN-0001"]
owner = ""
created = 2026-10-21
+++

## Why
Proves Bevy works on our machines and gives a visual harness for sim work.

## Acceptance criteria
- [x] Renders terrain, resources and units from fn_sim state using manifest-listed assets.
- [x] Pan/zoom, regenerate (R), switch nation (1-4), pause, hover tile info, nation HUD.
- [x] Sim ticks at fixed 10 Hz independent of frame rate.
- [ ] A human has run it and approved the feel.

## Out of scope
- Tilemap batching (fine at 96x64; revisit with FN-0012).
