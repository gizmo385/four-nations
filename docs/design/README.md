# Design docs

Every doc starts with a `Doc status:` line under its title: **current**, **superseded by X** or
**historical**. Each doc's own `Status:` line (proposed, accepted, decided, ...) tracks where its
decisions stand; "current" here means the doc still describes how the game is or is meant to be.
Keep this list in step when you add a doc or change a status.

## Current

- [vision.md](vision.md): what the game is
- [tech-stack.md](tech-stack.md): why the code is shaped this way; the Saves section is the live rule
- [save-compat.md](save-compat.md): saves that survive content changes
- [tui.md](tui.md): the terminal frontend, the only frontend
- [text-voice.md](text-voice.md): how player-facing text talks
- [side-panel-v2.md](side-panel-v2.md): the side panel (glance, feed, here)
- [bending.md](bending.md), [research.md](research.md), [research-horizons.md](research-horizons.md), [horizons-phase-1.md](horizons-phase-1.md), [power.md](power.md): abilities, tech tree, the long-term research spec and its first phase, power
- [buildings.md](buildings.md), [nation-buildings.md](nation-buildings.md), [materials.md](materials.md), [clearing.md](clearing.md): buildings, materials, clearing ground
- [horizons-phase-2.md](horizons-phase-2.md): research horizons phase 2, routes, outposts, beasts and canals
- [pacing.md](pacing.md), [late-tree.md](late-tree.md), [endgame.md](endgame.md), [difficulty.md](difficulty.md), [harm.md](harm.md): pacing, progression and pressure
- [embark.md](embark.md), [cartography.md](cartography.md): starting a settlement, charting scrolls
- [horizons-phase-3.md](horizons-phase-3.md): research horizons phase 3, trade techs and scribing
- [balance-harness.md](balance-harness.md): the balance harness
- [agent-play.md](agent-play.md), [agent-scripting.md](agent-scripting.md), [agent-hillclimb.md](agent-hillclimb.md), [agent-studio.md](agent-studio.md): agents playing, scoring and watched

## Superseded

- [side-panel.md](side-panel.md): superseded by [side-panel-v2.md](side-panel-v2.md)

## Historical

- [ui.md](ui.md): the retired Bevy client's visual direction
