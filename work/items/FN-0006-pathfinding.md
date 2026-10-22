+++
id = "FN-0006"
title = "A* pathfinding on the tile grid"
status = "proposed"
priority = "p1"
area = "sim"
depends_on = ["FN-0001"]
owner = ""
created = 2026-10-21
+++

## Acceptance criteria
- [ ] `fn_sim::path::find(map, from, to, &MoveRules) -> Option<Vec<Pos>>`, 8-way with no corner
      cutting, cost per terrain.
- [ ] `MoveRules` lets nations differ (e.g. Water Tribe crossing water, later).
- [ ] Unit tests incl. unreachable targets; a benchmark-style test on a 256x256 map stays < 5 ms.
