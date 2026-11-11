# Vision

Doc status: current

Status: **draft**. Open questions are tracked as GitHub issues.

## Audience and IP (decided 2026-10-21, revised 2026-11-11)

A **personal, non-commercial project** with one intended player, the owner. It is never sold.

It is not private, though:

- The repository is public.
- The web replayer is hosted on the public GitHub Pages site (#596; "The hosted replayer" in
  [agent-play.md](agent-play.md)).
- On 2026-11-11 the owner decided that browser play, once it is built, will be hosted on that
  same page.

Canon Avatar names, characters and fan art stay, as a free, non-commercial fan project. The
owner made that call knowing the code, the replayer and later the game itself are public.

(Until 2026-11-11 this section said the game lived in a private repo, was never published or
distributed, and would have no web build or public deploys. That stopped being true when the
repository went public and #596 hosted the replayer.)

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
   early decisions meaningful. Exposure, injury and sickness hurt without enemies, and herbs,
   the Healing Hut and Healing Water answer them ([harm.md](harm.md), proposed).
4. **Readable simulation.** You can hover anything and see why it's happening.

## One nation, no war (decided 2026-10-21)

The player controls **a single nation**. Other civilizations may be modelled later as neighbors
to trade, exchange culture and knowledge with, or help, but **nations never fight each other**.
Avatar is about balance and unity. Pressure comes from survival, the environment and growth,
not conquest. Design features (research #17, bending #6) around peaceful progress.

## Frontend (decided 2026-10-21, revised 2026-11-03)

**The terminal is the only frontend** (#93, #280, #281, owner, 2026-11-03). The game was built
first as a Bevy client with 16px original pixel art (#122; the design docs that recorded that
choice, `art.md` and `pixel-art.md`, have since been removed), then given a terminal UI
(`fn_tui`, docs/design/tui.md) that reached parity with it. The owner then chose to retire the
Bevy client, its art pipeline and its CI cost entirely: the terminal, with Unicode glyphs and an
`--ascii` fallback, is the whole visual language now. `cargo run` opens it on the title screen.

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
