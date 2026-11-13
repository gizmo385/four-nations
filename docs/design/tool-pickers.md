# Tool pickers: Bend opens its list, one Remove tool

Doc status: current

Status: **accepted** (#946, from the owner's 2026-11-12 web playtest). The owner chose the
direction in the playtest thread; see "Owner decisions" at the end. Mock-ups:
[Tool pickers mock-ups](https://claude.ai/artifact/RLbsLyF97nyYChStmdRr6w) (a Claude artifact,
private to the owner).

## The problem

The owner, after a web playtest: "Not being able to clear out existing buildings is frustrating.
The game seems to start with a few existing stockpiles that I can never clear out and am forced
to build around. Similarly, there are times when I build a building or bed and want to move it
later or redesign things. I'd expect cancel or clear to do that but it doesn't seem to work."

All of it already works. It is hidden behind "press the key again" and a held toggle:

| Today | What the player has to know |
|---|---|
| `X` Cancel | withdraws marks only; sites and stockpiles need the `O` toggle on (#335) |
| `E` Clear | clears trees, rock and plants; `E` **again** flips it to taking down buildings (#342), shown only by a `CLEAR buildings` badge |
| `B` Bend | arms the signature ability; `B` again steps to the next usable one (#123) |
| `U` Build | opens the Build screen (#285, #881): a list, details, locked rows say why, `Enter` arms the tool |

The owner, on the pattern: "I wonder if this actually represents the limits of the 'hit button
multiple times to switch' behavior. It's also pretty confusing for bending." And on the fix:
"I wonder if we could move towards the model that buildings use?"

## The rule

**A tool's key never changes what the tool does by being pressed again.** A tool that needs a
choice opens a list in the Build screen's style and `Enter` arms it; a tool that needs no choice
just arms. Pressing a tool's key while its list is open closes the list (as `U` does today).

## Bend: `B` opens the abilities list

Mock-up board 1. `B` behaves as `U` does: it opens a full-screen list in the shared page frame,
with the pointer on the ability the Bend tool last channelled, so `B` `Enter` re-arms the last
one. `Enter` arms the Bend tool with the highlighted ability and closes the list; on a locked row
it does nothing and the detail strip says what it still needs. `Esc` or `B` closes it and leaves
the tool as it was.

The Abilities screen (`I`, #719) already is this list: every ability, usable ones first, locked
ones dim naming what they need, `Enter` arming the Bend tool. So `B` opens that screen (as
`App::open_build_modal` does for `U`), and the press-again cycle (`tools::cycle_ability`) goes.
`I` keeps opening the same screen from Select mode. Number keys `1`-`9` on the list pick a row
directly (also worth adding to the Build list).

Columns stay as the Abilities screen has them; the mock-up's chi and work columns are optional.

## One Remove tool on `X`

Mock-up boards 2 and 3. Cancel (`X`) and Clear's buildings mode (`E` twice) merge into one
**Remove** tool on `X`. It has no list and no toggles: a rectangle takes away everything the
player made inside it:

- marks: harvest (including standing orders), bend, sow and clear designations;
- build sites not yet finished (what was delivered drops, as cancelling a site does today);
- stockpile tiles (their goods drop on the tile and are hauled to another zone, as
  `World::cancel_stockpile` does today);
- finished buildings, beds included, marked to be taken down (`World::designate_deconstruct`,
  the refund and work shares of `[deconstruct]`, or the packed ones with Pack Up).

Landmarks (the Harmony Gate) are skipped, as deconstruct refuses them today. Units, ground items
and terrain are never touched.

**The safety is the preview, not a mode.** #335 added the `O` toggle because a rectangle cancel
swept up stockpiles nobody meant to lose. With one tool, the preview has to make that impossible
to miss. While a corner is set, the tool section lists, one line a kind, what releasing would do,
with names where they fit and the stockpile line in the warning colour:

```
◇ REMOVE ──────────────── [X]
Takes away anything you made: marks,
build sites, stockpiles and buildings.

8 x 6 tiles
Takes down 3: Bed, Bed, Workshop
Cancels 2 marks: chop, sow
Unzones 4 stockpile tiles: 18 goods drop to be hauled
Back: 9 Wood, 2 Stone

Trees and rock stay: that's Clear (E).
```

The map overlay marks each tile that would change, and the box carries a one-line count. A
rectangle with nothing to remove says so and issues nothing. The banner, hint bar and tool tint
use the warning colour so the tool never looks like Select.

## Clear (`E`) keeps the ground only

Clear goes back to one meaning: uproot plants and work resources off the ground for good
(docs/design/clearing.md). Its buildings mode moves into Remove, so `E` pressed again does
nothing new. Taking away what the player built and clearing away nature are different intents; a
Remove drag over the settlement must not fell every tree in it.

## Frontends and agents

- **fn_ui** holds the behaviour: `Tool::CancelHarvest` and `Tool::Deconstruct` become
  `Tool::Remove`; `Pick::teardown`, `Here` and the `O` toggle go; the preview report gains the
  per-kind lines; `Tool::Clear` loses its toggle. `B` opens the abilities screen.
- **Terminal and egui** draw the same rows, as they do for every tool section today. No new
  widgets.
- **Agents and the harness**: the agent's designate tool takes `cancel` with `teardown?`
  (`crates/agent/src/tools.rs`). It becomes `remove` with no flag; scripts and scorecards that
  send `x`/`o` key sequences need updating; key help and `--help` text follow `keys.rs`.
- **Saves**: nothing new is saved (tools are view state).

## Out of scope

- A "move building" tool. Moving stays "take it down, build it again"; with Pack Up that loses
  nothing. File it separately if wanted.
- Changing what taking down refunds or costs.

## Follow-up issues

- `B` opens the abilities list; drop the press-again cycle; number keys in the Build and
  Abilities lists.
- One Remove tool on `X` (merging Cancel, Cancel+`O` and Clear's buildings mode) with the
  per-kind preview; Clear keeps the ground only; agent tool and key help updated.

## Owner decisions

Answered in the playtest thread, 2026-11-12 and 13:

1. **Bend:** a full-screen list in the Build screen's style (board 1), not a list in the side
   panel.
2. **Removal:** one combined Remove tool. "The number of different clear tools feels like
   overkill. Can we combine them? Should we just have a single remove tool?"
3. **No per-kind toggles on Remove:** "I think we can leave out the selection on the side for the
   remove tool - feels like overkill." The preview carries the safety instead.
4. **Clear stays on `E`, ground only.** Recommended and not objected to; reopen if wanted.
5. **Move building:** out of scope for now.
