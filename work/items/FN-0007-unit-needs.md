+++
id = "FN-0007"
title = "Unit needs: hunger, thirst, energy and death"
status = "proposed"
priority = "p1"
area = "sim"
depends_on = []
owner = ""
created = 2026-10-21
+++

## Why
Port the core survival pressure from the 2013 Java game (Unit.update / Eat/Drink/Sleep tasks)
and the cljs unit-attributes.edn (health, chi, energy, hunger, thirst).

## Acceptance criteria
- [ ] Needs defined in data/ (rates, thresholds), not hard-coded.
- [ ] Needs change per tick; at 0 health the unit dies and is removed deterministically.
- [ ] Tests: a unit with no food eventually dies; rates scale with TICKS_PER_SECOND.
