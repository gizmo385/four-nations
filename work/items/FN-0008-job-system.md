+++
id = "FN-0008"
title = "Job queue: designations, claiming and work progress"
status = "proposed"
priority = "p1"
area = "sim"
depends_on = ["FN-0006", "FN-0007"]
owner = ""
created = 2026-10-21
+++

## Why
The heart of the colony sim. The Java version had a global Task queue with remaining-work
counters (CollectResource, StoreResource, Build, Plant, Eat, Drink, Sleep).

## Acceptance criteria
- [ ] Player designations become jobs; idle units claim the nearest reachable job.
- [ ] Needs pre-empt jobs above a threshold (go eat/drink/sleep), then resume.
- [ ] Work speed = base × relevant nation modifier (e.g. quarrying uses `earthworks`).
- [ ] Chop tree -> wood item -> haul to stockpile, end to end in a headless test.
