+++
id = "FN-0012"
title = "Autotiling: smooth transitions between terrain types"
status = "proposed"
priority = "p2"
area = "client"
depends_on = ["FN-0005"]
owner = ""
created = 2026-10-21
+++

## Acceptance criteria
- [ ] Coast/water and biome edges use transition tiles (bitmask autotiling).
- [ ] Rendering moves to a chunked tilemap if sprite-per-tile becomes a bottleneck at 256x256.
