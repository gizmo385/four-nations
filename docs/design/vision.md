# Vision

Status: **draft**. Open questions are tracked as GitHub issues.

## Audience and IP (decided 2026-10-21)

A **personal project**: one player, the owner. It lives in a private repo and is never sold,
published or distributed. So canon Avatar names, characters and fan art are fine. No web build
or public deploys.

## Pitch

A colony/settlement management game set in the world of *Avatar: The Last Airbender*. You lead a
small settlement of one of the four nations. Your people gather, build, eat, sleep and bend.
**Which nation you pick changes how you play**, not just your stats.

## Pillars

1. **Nations play differently.** Every nation has passive modifiers (`data/nations.toml`) and,
   later, signature bending abilities (#6) that open strategies other nations lack.
   - **Air Nomads**: fast movement; wind power (turbines, mills); weak at heavy labor.
   - **Water Tribes**: irrigation, fishing, healing; can work and travel on water.
   - **Earth Kingdom**: earthworks, quarrying and walls are cheap; strong carriers; slow.
   - **Fire Nation**: smelting, forging and warmth without much fuel; poor with water.
2. **Indirect control.** You designate work and the people do it, subject to their needs, as
   in the 2013 version, RimWorld, or Dwarf Fortress.
3. **Survival pressure first, then growth.** Hunger, thirst and energy (and later, chi) keep
   early decisions meaningful.
4. **Readable simulation.** You can hover anything and see why it's happening.

## One nation, no war (decided 2026-10-21)

The player controls **a single nation**. Other civilizations may be modelled later as neighbors
to trade, exchange culture and knowledge with, or help, but **nations never fight each other**.
Avatar is about balance and unity. Pressure comes from survival, the environment and growth,
not conquest. Design features (research #17, bending #6) around peaceful progress.

## Art direction (decided 2026-10-22, revised 2026-10-24)

**16px top-down pixel art, all of it original** (#122, owner, 2026-10-24). Claude writes the art
as palette-indexed text sheets in `art/*.toml` and `cargo xtask art` renders them to
`assets/original/`. The Avatar world's look (outlined sprites, stylised East Asian setting, units
dressed per nation; canon characters may come later as special sprites in the same style) is our
own. Process, palette and scope are in [pixel-art.md](pixel-art.md). The third-party pack we
prototyped with, Ninja Adventure, is fully gone; [art.md](art.md) keeps the record of that choice.

## Core loop (first playable)

Generate world → place settlement → designate (chop, mine, farm, build) → units claim jobs →
resources flow to stockpiles → needs are met or units die → population grows.
Win: reach N population (the 2013 goal was 10). Lose: everyone dies.

## End game (decided 2026-10-25, #224)

A long road of achievements (population, buildings, the nation's research branch, stores,
bending mastery) leads to a great work built **with the Avatar**: the Harmony Gate. Finishing it
opens the settlement to people of all four nations, each bending their own element. Design and
the owner's decisions: [endgame.md](endgame.md).

## Open questions

- **Scale**: one settlement on a 96×64 map, or a larger world map with travel?
