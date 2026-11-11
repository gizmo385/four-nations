# Side panel: summaries in the panel, detail on demand

Doc status: superseded by [side-panel-v2.md](side-panel-v2.md)

> Revised by docs/design/side-panel-v2.md (#414), which regroups the sections below into a
> Settlement glance, an Alerts feed and a Here section.

Status: **proposed** (#302, from the owner's 2026-11-04 playtest). It revises the side panel in
docs/design/tui.md ("Screen layout", "What #99 built"), gives #307 (active tool) its direction,
and leaves room for #299 (Season section) and #306 (Esc game menu).

## The problem

The owner played Earth with 20 settlers. The panel's 32 columns were crowded and hard to read:

- **People** took half the panel, one line per settler, most of them reading "Idle". It grows
  with the population, so it gets worse as the game goes well.
- **Activity lines truncate** ("Delivering 4 Wood to …"), because a name plus a task doesn't fit
  in 28 cells.
- **Tool** wraps its hint over three or four lines, and the status line already says it.
- **Headings look like body text.** `◇ PEOPLE` is bold accent text, the same tone as the
  selected unit and "Enter: set a corner".
- **The fitting loop starves the rest.** Stores, Research, Alerts and Cursor compete for what's
  left. Endeavor and Modifiers give way first, and at 80x24 with a tool active the roster
  disappears with no "+N more" line.

Today at 80x24 with the Harvest tool active (real `--frame`, seed 7, six settlers):

```
""~♠,,¥,,,,,,,♣,♣...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♣,,.≈≈.≈≈.┏───── ◆ EARTH KINGDOM ◆ ──────┓
"~"",,,,,,,,,,,,,,,....≈≈≈≈≈≈≈≈≈≈≈≈≈..♣,,...≈≈≈.│ Seed 7                 02:30 │
~"",,¥,,,,,♣¥,,,♣,,♣,≡..≈≈≈≈≈≈≈≈≈≈≈≈.,,,¥♣,.....│ ▶ Running x1      Space  + - │
~",,¥,,,,,,,,,,,,,,,,≡,.≈≈≈≈≈≈≈≈≈≈.≈.,,♣♣♣♣♣.≈.♣│                              │
τ",♣,,τ,,,,,,,,,,,♣,≡,,..≈≈≈≈≈≈≈≈....,,,♣,♣♣...♣│ ◇ PEOPLE                   6 │
"♠,♣,,,,,¥,,,,,,,♣,♣,≡,,.≈≈≈≈≈≈≈≈≈..♣,♣♣♣,,,,♣,♣│                              │
""♣,,,,,,,,,,,,¥,τ,♣≡,,,.≈≈≈≈≈≈≈ƒ.≈.,,,,♣,,,,♣♣♣│ ◇ STORES                     │
",♣♣,,,,,♣,♣,♣,,¥,,,,≡,¥.≈≈≈≈≈≈≈≈≈..,,,,,,,♣♣,,♣│ Wood 36  Fruit 2  Cabbage 1  │
",,♣,♣♣,,,,,♣,,♣,,,,,,,,.≈≈≈≈≈≈≈≈≈≈..,♣♣,♣,♣♣♣,♣│                              │
"♣,♣,,,♣,,♣,,,♣,♣♣♣,≡,,,..≈≈≈≈≈≈≈≈≈≈..,,,♣,,♣,♣♣│ ◇ RESEARCH                   │
"♣,,,,,,,,,,,♣,,,,,♣,≡,,.≈≈≈≈≈≈≈≈≈≈≈≈.τ,,,,¥,,τ♣│ No target set        3.0/min │
♠♣♣♣,,,,,,%,,,,,♣,♣♣,♣,..≈≈≈≈≈≈≈≈≈≈≈≈.,♣,♣,♣♣♣,♠│ 8 pts banked                 │
,,,♣♣≡≡≡≡≡,¥¥,,♣,,,,,,♣.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,....~│                              │
,♣♣,♣,,,,,,,,,,,,,♣♣♣,♣.≈≈≈≈≈≈≈≈≈≈≈≈..,,,♣..≈≈~,│ ◇ TOOL           Harvest [H] │
,,,♣,≡,,≡,,,,,,♣♣♣,♣♣...≈≈≈≈≈≈≈≈≈≈≈≈.,,,...≈≈≈.♣│ designate harvest (chop,     │
,,,,♣,,,,,,τ...♣,,,...≈≈≈≈≈≈≈≈≈≈≈ƒ...,,..≈...≈..│ mine, gather); hold Shift to │
♠"♠,,☺,,,,,..≈..♣,..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,≡..ƒ..,..≈.│ repeat: 2 done, 3 skipped    │
...,,,,,,¥,.≈≈≈..,.≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,%.≈≈.,,♣..≈│ (nothing to harvest)         │
≈≈....,,,,,.≈≈≈≈.,.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,%≈...,,,,..│                              │
≈≈≈≈≈...,,,.≈≈≈≈....≈≈≈≈≈≈≈≈≈ƒ≈..,,,,≡≡.,,♣♣,,,,│ ◇ CURSOR           (114, 77) │
≈≈≈≈≈≈≈..,☺.≈≈≈≈≈ƒ≈.≈≈≈≈≈≈≈≈≈...,,,,,,,,,,,♣♣,,♣│ Enter: set a corner          │
≈≈≈≈≈≈ƒ≈..,.....≈≈≈≈≈≈≈≈≈≈≈≈..♣,,,,,,,,♣,♣,,,,,¥│ Land                         │
≈≈≈≈≈≈≈≈≈..,.≈.☺..≈≈≈≈≈≈≈....,♣,,,,,,,,,,,,♣♣,♣"┗──────────────────────────────┛
 HARVEST  Enter set corner  Esc leave  |  designate harvest (chop, mine, gathe…
```

The panel holds two kinds of information. One kind you **watch** (time, how many people are idle
or hungry, stores, research, alerts, the tile under the cursor). The other you **look up**
(each settler's exact task, the nation's modifiers, the seed). The panel should hold only what
you watch, as short summaries that stay the same height. What you look up goes on a full screen,
the way Research (`T`) and Endeavor (`G`) already work.

## Decisions in brief

1. **People becomes counts.** Settlers are grouped by activity ("Idle 9  Haul 4  Harvest 3"),
   with a line of need flags ("Hungry 2  Exhausted 1") that shows only when someone is in
   trouble. The panel stays the same height whether you have 6 settlers or 60.
2. **A Nation screen (`N`) holds the detail:** every settler with the full task text and need
   meters, grouped the same way, plus all stores, the nation's traits (today's Modifiers) and the
   seed. `Enter` on a settler selects them and closes the screen.
3. **Headings become dividers joined to the frame:** `├─ PEOPLE ──────── 20 ─┤`. The rule makes
   each section start unmistakable, in colour, in mono and in ASCII. The blank row between
   sections goes, which saves 7 rows.
4. **Every section gets a fixed, short shape.** The header is one row, Research two, Endeavor
   one, each alert one line, and Tool only its extras. The seed moves to the panel's bottom
   edge. Modifiers move to the Nation screen.
5. **The active tool gets a banner across the map's top row**, in the tool's own colour (for
   #307). The panel's Tool section moves down next to Cursor and loses its hint.

## The panel

### Sections, top to bottom

| Section | Rows (full / short) | Content | Change |
|---|---|---|---|
| Header | 1 | `▶ Running x1` (or `‖ Paused`) on the left, the clock on the right | Seed moves to the bottom edge; `Space + -` is in the status line and key help |
| People | 1 heading, 1-3 | Activity counts, then the need flags | Replaces the roster |
| Season (#299) | 1 heading, 2 / 1 | The heading is the season itself: `├─ AUTUMN · Y1 ── 4:10 left ─┤`; then food per head, then "Winter here" | #299's content, in the divider format |
| Stores | 1 heading, 2 / 1 | As today, at most two rows; past that `+N [N]` | Capped |
| Research | 1 heading, 2 / 1 | Target and rate; bar, percent and points. Short: `Masonry ██████░░░░ 68%` | Same content |
| Endeavor | 1 heading, 1 / 0 | Count on the heading (`2/9`), then only the first missing sign, `Next: 14 more people [G]` | The other missing signs are on `G` |
| Alerts | 1 heading, 3 / 1 | Newest last, **one line each** with an ellipsis; `[L]` on the heading | The full text is in the log |
| Tool | 1 heading, 0-4 | Only in a tool. The heading always shows (a second signal beside the banner); under it, the Build choice or the Bend abilities, then the last result | Hint lines removed; moved from above Alerts to just above Cursor |
| Cursor | 1 heading, the rest (at least 3) | As today, without the "Enter: set a corner" line, which moves to the banner | Always last; it gets the spare rows |

Order: Header, People, Season, Stores, Research, Endeavor, Alerts, Tool, Cursor. **Tool sits
next to Cursor** because both are about what's under your hand. Putting it last also means
picking or leaving a tool never moves the sections above it.

**Fitting.** When the panel is too short, it gives up rows in this order: Endeavor's "Next" line,
alerts beyond the newest, Research's second row, Stores' second row, People's extra group rows
(folded into `+N`), Season's "Winter here" line, the Tool notice beyond two lines, then the Bend
ability list down to the current ability. The header, every heading and three Cursor rows always
stay. With these shapes, everything fits at 80x24 with Season and a tool active, which today's
layout can't do.

### People: counts, not names

`fn_ui::view::activity_counts(session) -> Vec<(Activity, usize)>` groups units by task, in a
**fixed order** with zero counts hidden. A fixed order means a group stays in the same place from
one tick to the next. Sorting by count would make the line jitter.

| `Activity` | Label | From `Task` |
|---|---|---|
| Idle | Idle | no task |
| Haul | Haul | `Haul`, `Deliver` |
| Harvest | Harvest | `Harvest` |
| Farm | Farm | `Sow`, `Reap` |
| Build | Build | `Build` |
| Bend | Bend | `Bend` (channelling included) |
| Craft | Craft | `Craft` |
| Study | Study | `Study` |
| Gate | Gate | `Endeavor`, `Attend`, `Rite` |
| Eat | Eat | `Eat`, `Drink` |
| Sleep | Sleep | `Sleep` |
| Heal | Heal | `Healing` |

The labels are short verbs because `Harvesting 3` doesn't fit beside two other groups in 28
cells. The groups pack into at most three rows.

**Need flags** come from `fn_ui::view::need_flags(session)`. They count units by
`need_status` (the thresholds in data/needs.toml, as the selection bar uses), in `warn` and
`bad` words: Hungry/Starving, Thirsty/Parched, Tired/Exhausted, Hurt/Dying, and Cold when #299
lands. The row hides when everyone is fine, so it shows up only when it matters. `[N]` sits at
its right end, or at the end of the last group row when there are no flags.

Both functions are presentation over sim state. They read no balance numbers of their own, so
nothing new goes into `data/`.

### Headings

```
├─ PEOPLE ──────────────── 20 ─┤      +- PEOPLE ---------------- 20 -+   (--ascii)
```

- The rule replaces the frame's `│` on both sides, with `├` `┤` in `accent_soft` like the frame
  (`+` and `-` in ASCII).
- The title is `accent_text`, bold, upper case. The right-hand value (count, coordinates, key)
  is `text`.
- The `◇` bullet goes: the rule already does its job.
- In the Tool section, the title is the tool's name in the tool's tint (`├─ HARVEST ── [H] ─┤`),
  which ties it to the banner.
- `Tone::Title` stays for selected things. Headings get their own `Tone::Heading`, so the two
  can't be confused again.

### 120x40, 20 settlers, autumn, Harvest tool

A mockup: the map is a real `--frame` (seed 7, Earth). The panel is laid out by hand, since the
People summary and Season don't exist yet. The top row is the tool banner.

```
━ HARVEST ━ chop, mine, gather  ·  Enter: set a corner  ·  Esc: leave ━━━━━━━━━━━━━━━━━━┏───── ◆ EARTH KINGDOM ◆ ──────┓
"♠♠♠♠♠♠""""♣♣♣,,♣,,,,,,,,,··♣♣♣,♣♣,♣♣,,,·,,,,,,...≈...,,...≈≈.≈≈≈≈≈≈≈≈≈≈..."♠♠♠♠♠""♠♠♠♠"│ ▶ Running x1           05:00 │
¥♠...♠"♠♠""",,,♣,,,,♣,,,♣····▲♣♣♣,,♣♣♣,,τ♣,,,,..≈≈≈ƒ≈.,,,,...≈≈≈≈≈≈≈≈≈..."""♠""♠♠"♠♠♠♠♠"├─ PEOPLE ──────────────── 20 ─┤
♠♠.≈.♠""""""",♣,,τ,,,,,,,,···▲▲•▲▲▲,,♣♣♣♣,¥,,,.≈≈≈≈≈..,♣,,,,..≈≈≈≈≈≈≈..♠♠"""""""""♠"♠♠♠♣│ Idle 9  Haul 4  Harvest 3    │
♠♠...""""♠♠""",,♣,,,,,,,,,♣,,♣▲$▲▲▲♣♣,,,,,,,,..≈≈≈≈..,♣τ,♣♣,,.≈≈≈≈≈...♠♠♠"♠"""""♠"♠"...♣│ Build 2  Sleep 2             │
♠♠♠♠♠""""♠""♠"",,♣,,,,♣,,,,,♣♣▲▲▲•♣,,♣,♣,♣,τ,.≈≈≈≈≈≈..,,♣♣,,,.≈≈≈≈≈.."""♠"""♠""""""♠.≈~~│ Hungry 2  Exhausted 1    [N] │
♠♠♠♠""""♠♠"♠♠""",,,,,,,,,,,,τ,,▲▲▲,♣,,♣,,,,,..≈≈≈≈≈≈≈..,,,♣,,.≈..≈≈≈.",""""""♠""♠""√.~.≈├─ AUTUMN · Y1 ──── 4:10 left ─┤
♠♠♠♠♠♠"♠"♠♠""♠♠"♠,,,,,,,,,,,,,,▲▲▲,♣,~♣,,,,,.≈≈≈≈≈≈≈ƒ≈..,,,♣,.....≈..",τ""♠""""▲▲$▲♣~√..│ Food in stores   38 · 2/head │
♠""♠♠"♠♠♠""""¥""♠¥""¥♣,♣,,,,,,,,•▲,,,,~,,,,..≈≈≈≈≈≈≈≈≈≈.,♣♣,,♣,,,.≈.,♠,,,"""""ττ•▲♣√~√√√│ Winter here: cold, frost     │
♠""♠♠♠"♠♠♠♠♠♠♠""♠""♠"♠,,,,,,,,,•▲▲,♣♣,,~,,..≈≈≈≈≈≈≈≈≈≈≈..♣,,,,,...≈.,,♠,"¥♠"♠♠""▲♠√√~√√"├─ STORES ─────────────────────┤
▲""♠♠"♠"♠♠♠♠¥♠♠"♠"""""",,,,,,,,▲▲▲,,,...~..≈≈≈≈≈≈≈≈≈≈≈≈≈..♣♣,...≈≈≈.¥♠"""♠"♠"♠"""♠√√~√√♣│ Wood 36  Stone 12  Fruit 21  │
♠♠♠♠"♠♠♠♠♠""♠♠"""""¥"♠♠♠",,,,,,▲▲,,,,.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,τ,.≈≈≈≈≈.♠♠♠"♠""""""¥""♣√~√√√│ Cabbage 17  Brick 4          │
"""♠♠♠♠♠""♠♠"""¥¥""""""ƒ,,,,,,,•,,♣♣♣.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.ƒ≈≈≈≈~♠♠♠""♠"♠♠"""♠♠√♣√~√√├─ RESEARCH ───────────────────┤
"♠♠♠♠♠♠♠♠♠♠♠♠"""""♠"""~♠,,¥,,,,,,,♣,♣...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♣,,.≈≈.≈≈.~♠♠"♠"♠♠♠♠""♠"√√√√~√│ Masonry              7.2/min │
"♠"♠♠♠♠♠♠♠♠♠"♠"""""""~"",,,,,,,,,,,,,,,....≈≈≈≈≈≈≈≈≈≈≈≈≈..♣,,...≈≈≈.♠~♠""♠♠"♠"""""♠♣√~√√│ ██████░░░░  68%  41/60 pts   │
"♠♠""♠♠♠...~..♠"♠♠""~"",,¥,,,,,♣¥,,,♣,,♣,≡..≈≈≈≈≈≈≈≈≈≈≈≈.,,,¥♣,.....♠♠~♠♠♠♠...♠""♠√√~√√♠├─ ENDEAVOR ───────────── 2/9 ─┤
"♠√♠♠♠♠..≈≈≈≈...""""~",,¥,,,,,,,,,,,,,,,,≡,.≈≈≈≈≈≈≈≈≈≈.≈.,,♣♣♣♣♣.≈.♣♠♠♠~~"♠.≈."♠♠♠τ~√√♠♠│ Next: 14 more people     [G] │
"♣♣~"♠♠.≈≈≈≈≈≈≈..""~τ",♣,,τ,,,,,,,,,,,♣,≡,,..≈≈≈≈≈≈≈≈....,,,♣,♣♣...♣♠♠♠♠♠ƒ~~≈.♠♠♠♠~♠♠♠"♠├─ ALERTS ─────────────── [L] ─┤
..~."...≈≈≈≈≈≈≈≈..~""♠,♣,,,,,¥,,,,,,,♣,♣,≡,,.≈≈≈≈≈≈≈≈≈..♣,♣♣♣,,,,♣,♣,♠♠♠♠♠"..~~"~~""""♠♠│ Learned Stoneworking         │
.≈≈...≈≈ƒ≈≈≈≈≈≈≈≈~."""♣,,,,,,,,,,,,¥,τ,♣≡,,,.≈≈≈≈≈≈≈ƒ.≈.,,,,♣,,,,♣♣♣♣♣♠♠♠"♠♠"♠"~""♠¥"♠♠♠├─ HARVEST ────────────── [H] ─┤
≈≈≈ƒ≈≈≈≈≈≈≈≈≈≈≈≈≈≈..",♣♣,,,,,♣,♣,♣,,¥,,,,≡,¥.≈≈≈≈≈≈≈≈≈..,,,,,,,♣♣,,♣,,♠"♠♠♠♠"♠♠"""♠♠"""♠│ 2 done, 3 skipped (nothing   │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.",,♣,♣♣,,,,,♣,,♣,,,,,,,,.≈≈≈≈≈≈≈≈≈≈..,♣♣,♣,♣♣♣,♣♣,,♣,¥"♠♠♠♠""¥"♠"♠"♠│ to harvest)                  │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈."♣,♣,,,♣,,♣,,,♣,♣♣♣,≡,,,..≈≈≈≈≈≈≈≈≈≈..,,,♣,,♣,♣♣♣,♣♣♠♣♣,♠♠♠"""""""♠♠├─ CURSOR ───────── (114, 77) ─┤
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.."♣,,,,,,,,,,,♣,,,,,♣,≡,,.≈≈≈≈≈≈≈≈≈≈≈≈.τ,,,,¥,,τ♣♠♠♠♠♣♠♠"♠♠"♠"""""""♠│ Land                         │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠♠♣♣♣,,,,,,%,,,,,♣,♣♣,♣,..≈≈≈≈≈≈≈≈≈≈≈≈.,♣,♣,♣♣♣,♠♠"♠"♠♠♠♠"♠"♠"♠""♠,""│ Plains (114, 77)             │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠,,,♣♣≡≡≡≡≡,¥¥,,♣,,,,,,♣.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,....~~~♠"♠"♠♠♠♠♠♠""""♠♠♠♠│ Elevation 157                │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠,♣♣,♣,,,,,,,,,,,,,♣♣♣,♣.≈≈≈≈≈≈≈≈≈≈≈≈..,,,♣..≈≈~,""~♠"~~♠♠♠♠τ"♠""""♠♠│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈..♠,,,♣,≡,,≡,,,,,,♣♣♣,♣♣...≈≈≈≈≈≈≈≈≈≈≈≈.,,,...≈≈≈.♣,""~~♠♠~"♠♠"♠"""♠♠♠♠│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠♠,,,,♣,,,,,,τ...♣,,,...≈≈≈≈≈≈≈≈≈≈≈ƒ...,,..≈...≈..,♣τ♠♠♠""ƒ♠♠♠♠♠"♠♠♠♠"│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.¥♠♠"♠,,☺,,,,,..≈..♣,..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,≡..ƒ..,..≈..,,♠♠♠♠♠"~~♠"♠♠♠♠♠♠♠│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈......,,,,,,¥,.≈≈≈..,.≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,%.≈≈.,,♣..≈.","♣♣♠♠♠♠♠ƒ~~♠♠♠~~~│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈....,,,,,.≈≈≈≈.,.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,%≈...,,,,..~"""♠,,♠"♠♠♠♠♠~~~♠".│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈...,,,.≈≈≈≈....≈≈≈≈≈≈≈≈≈ƒ≈..,,,,≡≡.,,♣♣,,,,"~"""♣,♣,♠"♠♠♠"♠""♠.│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈..,☺.≈≈≈≈≈ƒ≈.≈≈≈≈≈≈≈≈≈...,,,,,,,,,,,♣♣,,♣""~"♠♣,,,,,,"♠♠""♠""│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈ƒ≈..,.....≈≈≈≈≈≈≈≈≈≈≈≈..♣,,,,,,,,♣,♣,,,,,¥"~"""♣,♣♣,,,,,,"""♠"│                              │
≈≈≈...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈..,.≈.☺..≈≈≈≈≈≈≈....,♣,,,,,,,,,,,,♣♣,♣"~"""""♣♣♣,,,,♣,♠♠"♠"│                              │
≈...♠.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.....,☺....≈≈≈≈.♣,,♣,,,,,,,,♣,,♣,,,♣,""~"""",,♣♣♠♠♠♠♠♠""""│                              │
.."♠τ...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈ƒ≈..♣,♣,♣,,..☺...,,☺,,♣,,,,,,,,♣,,,,,,,♠¥~""",♣,,♠.......""│                              │
"""♠♠♠♠..≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,♣♣,,♣♣,,,♣,,τ,,▲▲,♣,♣♣♣,,,♣,♣""~"♠♠♠,♣♣,♠.≈ƒ≈.≈.""┗─ seed 7 ─────────────────────┛
 HARVEST  Enter set corner  Esc leave  O standing order  ? keys
```

### 80x24, Select mode

```
""~♠,,¥,,,,,,,♣,♣...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♣,,.≈≈.≈≈.┏───── ◆ EARTH KINGDOM ◆ ──────┓
"~"",,,,,,,,,,,,,,,....≈≈≈≈≈≈≈≈≈≈≈≈≈..♣,,...≈≈≈.│ ▶ Running x1           02:30 │
~"",,¥,,,,,♣¥,,,♣,,♣,≡..≈≈≈≈≈≈≈≈≈≈≈≈.,,,¥♣,.....├─ PEOPLE ───────────────── 6 ─┤
~",,¥,,,,,,,,,,,,,,,,≡,.≈≈≈≈≈≈≈≈≈≈.≈.,,♣♣♣♣♣.≈.♣│ Idle 2  Haul 3  Harvest 1    │
τ",♣,,τ,,,,,,,,,,,♣,≡,,..≈≈≈≈≈≈≈≈....,,,♣,♣♣...♣├─ STORES ─────────────────────┤
"♠,♣,,,,,¥,,,,,,,♣,♣,≡,,.≈≈≈≈≈≈≈≈≈..♣,♣♣♣,,,,♣,♣│ Wood 36  Fruit 2  Cabbage 1  │
""♣,,,,,,,,,,,,¥,τ,♣≡,,,.≈≈≈≈≈≈≈ƒ.≈.,,,,♣,,,,♣♣♣├─ RESEARCH ───────────────────┤
",♣♣,,,,,♣,♣,♣,,¥,,,,≡,¥.≈≈≈≈≈≈≈≈≈..,,,,,,,♣♣,,♣│ No target set      8 pts [T] │
",,♣,♣♣,,,,,♣,,♣,,,,,,,,.≈≈≈≈≈≈≈≈≈≈..,♣♣,♣,♣♣♣,♣├─ ENDEAVOR ───────────── 0/9 ─┤
"♣,♣,,,♣,,♣,,,♣,♣♣♣,≡,,,..≈≈≈≈≈≈≈≈≈≈..,,,♣,,♣,♣♣├─ ALERTS ─────────────── [L] ─┤
"♣,,,,,,,,,,,♣,,,,,♣,≡,,.≈≈≈≈≈≈≈≈≈≈≈≈.τ,,,,¥,,τ♣│ Learned Stoneworking         │
♠♣♣♣,,,,,,%,,,,,♣,♣♣,♣,..≈≈≈≈≈≈≈≈≈≈≈≈.,♣,♣,♣♣♣,♠├─ CURSOR ───────── (114, 77) ─┤
,,,♣♣≡≡≡≡≡,¥¥,,♣,,,,,,♣.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,....~│ Land                         │
,♣♣,♣,,,,,,,,,,,,,♣♣♣,♣.≈≈≈≈≈≈≈≈≈≈≈≈..,,,♣..≈≈~,│ Plains (114, 77)             │
,,,♣,≡,,≡,,,,,,♣♣♣,♣♣...≈≈≈≈≈≈≈≈≈≈≈≈.,,,...≈≈≈.♣│ Elevation 157                │
,,,,♣,,,,,,τ...♣,,,...≈≈≈≈≈≈≈≈≈≈≈ƒ...,,..≈...≈..│ Stockpile                    │
♠"♠,,☺,,,,,..≈..♣,..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,≡..ƒ..,..≈.│                              │
...,,,,,,¥,.≈≈≈..,.≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,%.≈≈.,,♣..≈│                              │
≈≈....,,,,,.≈≈≈≈.,.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,%≈...,,,,..│                              │
≈≈≈≈≈...,,,.≈≈≈≈....≈≈≈≈≈≈≈≈≈ƒ≈..,,,,≡≡.,,♣♣,,,,│                              │
≈≈≈≈≈≈≈..,☺.≈≈≈≈≈ƒ≈.≈≈≈≈≈≈≈≈≈...,,,,,,,,,,,♣♣,,♣│                              │
≈≈≈≈≈≈ƒ≈..,.....≈≈≈≈≈≈≈≈≈≈≈≈..♣,,,,,,,,♣,♣,,,,,¥│                              │
≈≈≈≈≈≈≈≈≈..,.≈.☺..≈≈≈≈≈≈≈....,♣,,,,,,,,,,,,♣♣,♣"┗─ seed 7 ─────────────────────┛
 SELECT  arrows move  Enter select  H X P B U tools  N nation  Esc menu  ? keys
```

### 80x24, Harvest tool, `--ascii --color never`

The banner and the headings work through shape alone:

```
= HARVEST = Enter: set a corner - Esc: leave ===+----- * EARTH KINGDOM * ------+
"="",,,,,,,,,,,,,,,....~~~~~~~~~~~~~..T,,...~~~.| > Running x1           02:30 |
="",,Y,,,,,TY,,,T,,T,/..~~~~~~~~~~~~.,,,YT,.....+- PEOPLE ----------------- 6 -+
=",,Y,,,,,,,,,,,,,,,,/,.~~~~~~~~~~.~.,,TTTTT.~.T| Idle 2  Haul 3  Harvest 1    |
c",T,,c,,,,,,,,,,,T,/,,..~~~~~~~~....,,,T,TT...T+- STORES ---------------------+
"F,T,,,,,Y,,,,,,,T,T,/,,.~~~~~~~~~..T,TTT,,,,T,T| Wood 36  Fruit 2  Cabbage 1  |
""T,,,,,,,,,,,,Y,c,T/,,,.~~~~~~~f.~.,,,,T,,,,TTT+- RESEARCH -------------------+
",TT,,,,,T,T,T,,Y,,,,/,Y.~~~~~~~~~..,,,,,,,TT,,T| No target set      8 pts [T] |
",,T,TT,,,,,T,,T,,,,,,,,.~~~~~~~~~~..,TT,T,TTT,T+- ENDEAVOR ------------- 0/9 -+
"T,T,,,T,,T,,,T,TTT,/,,,..~~~~~~~~~~..,,,T,,T,TT+- ALERTS --------------- [L] -+
"T,,,,,,,,,,,T,,,,,T,/,,.~~~~~~~~~~~~.c,,,,Y,,cT| Learned Stoneworking         |
FTTT,,,,,,%,,,,,T,TT,T,..~~~~~~~~~~~~.,T,T,TTT,F+- HARVEST -------------- [H] -+
,,,TT/////,YY,,T,,,,,,T.~~~~~~~~~~~~~.,,,,,....=| 2 done, 3 skipped (nothing   |
,TT,T,,,,,,,,,,,,,TTT,T.~~~~~~~~~~~~..,,,T..~~=,| to harvest)                  |
,,,T,/,,/,,,,,,TTT,TT...~~~~~~~~~~~~.,,,...~~~.T+- CURSOR --------- (114, 77) -+
,,,,T,,,,,,c...T,,,...~~~~~~~~~~~f...,,..~...~..| Land                         |
F"F,,@,,,,,..~..T,..~~~~~~~~~~~~~..,,/..f..,..~.| Plains (114, 77)             |
...,,,,,,Y,.~~~..,.~~~~~~~~~~~~~..,,,%.~~.,,T..~| Elevation 157                |
~~....,,,,,.~~~~.,.~~~~~~~~~~~~~.,,,,%~...,,,,..| Stockpile                    |
~~~~~...,,,.~~~~....~~~~~~~~~f~..,,,,//.,,TT,,,,|                              |
~~~~~~~..,@.~~~~~f~.~~~~~~~~~...,,,,,,,,,,,TT,,T|                              |
~~~~~~f~..,.....~~~~~~~~~~~~..T,,,,,,,,T,T,,,,,Y|                              |
~~~~~~~~~..,.~.@..~~~~~~~....,T,,,,,,,,,,,,TT,T"+- seed 7 ---------------------+
 HARVEST  Enter set corner  Esc leave  O standing order  ? keys
```

## The Nation screen (`N`)

This is a full screen (docs/design/tui.md "Full screens", #652), like `T`, `L` and `G`: it covers
the side panel too, and the status line reads `NATION`.

- **People**, grouped under the same activity headings as the panel and in the same order, so
  the panel line "Idle 9" is the screen's "IDLE 9" block. Each row has a pointer, the name, the
  full `describe_task` text (place included) and, when the screen is at least 80 columns wide
  (every size the game draws at, since the screen took the side panel's columns in #652),
  five-cell meters for hunger, thirst, energy and health with a `!` on any that isn't ok (chi is
  added for benders when there's room). Narrower, the meters would give way to a flag after the
  task (`Idle  Hungry!`).
- **Stores**: every item, not just two rows' worth.
- **Traits**: today's Modifiers section, moved here. The values only change when research
  changes them, so they don't need watching.
- The header line: nation, seed and population.
- **Keys:** arrows/`W` `S` move the pointer and `PgUp` `PgDn` page. `Enter` selects that settler,
  puts the cursor on them and closes the screen (the selection bar opens). `N` or `Esc` closes.
  `Tab` on the map still cycles settlers as today.

```
┏───────────────────────────────────── ◆ NATION ◆ ─────────────────────────────────────┓┏───── ◆ EARTH KINGDOM ◆ ──────┓
│ Earth Kingdom · seed 7 · 20 people                                             05:00 ││ ▶ Running x1           05:00 │
│                                                                                      │├─ PEOPLE ──────────────── 20 ─┤
│ PEOPLE          Doing                                 Hunger Thirst Energy Health    ││ Idle 9  Haul 4  Harvest 3    │
│ IDLE 9                                                                               ││ Build 2  Sleep 2             │
│  ▸Oma   Idle                                  █░░░░! ███░░  ███░░  █████             ││ Hungry 2  Exhausted 1    [N] │
│   Lin   Idle                                  █░░░░! █████  ███░░  █████             │├─ STORES ─────────────────────┤
│   Ping  Idle                                  ███░░  ███░░  █░░░░! █████             ││ Wood 36  Stone 12  Fruit 21  │
│   Tyro  Idle                                  ████░  ████░  ███░░  █████             ││ Cabbage 17  Brick 4          │
│   Ming  Idle                                  ███░░  ████░  █████  █████             │├─ RESEARCH ───────────────────┤
│   Song  Idle                                  ███░░  ████░  █████  █████             ││ Masonry              7.2/min │
│   Haru  Idle                                  ███░░  ███░░  ███░░  █████             ││ ██████░░░░  68%  41/60 pts   │
│   Kyo   Idle                                  ███░░  █████  █████  █████             │├─ ENDEAVOR ───────────── 2/9 ─┤
│   Bo    Idle                                  ████░  ████░  ████░  █████             ││ Next: 14 more people     [G] │
│ HAUL 4                                                                               │├─ CURSOR ───────── (110, 77) ─┤
│   Shu   Delivering 4 Wood to a stockpile      ████░  ███░░  ███░░  █████             ││ Land                         │
│   Jin   Fetching 2 Fish from (96, 70)         ████░  █████  █████  █████             ││                              │
│   Lee   Delivering 3 Stone to a stockpile     █████  ████░  ███░░  █████             ││                              │
│   Mai   Fetching 4 Wood from (88, 61)         █████  ████░  ███░░  █████             ││                              │
│ HARVEST 3                                                                            ││                              │
│   Toph  Chopping Oak Tree at (76, 64), 40% d… █████  ███░░  ███░░  █████             ││                              │
│   Yuan  Mining Stone at (81, 58), 10% done    ████░  ███░░  ████░  █████             ││                              │
│   Gan   Picking Fruit Tree at (90, 41), 75% … ███░░  █████  █████  █████             ││                              │
│ BUILD 2                                                                              ││                              │
│   Dai   Building at (95, 66)                  ███░░  █████  ████░  █████             ││                              │
│   Hide  Building at (95, 66)                  █████  ████░  █████  █████             ││                              │
│ SLEEP 2                                                                              ││                              │
│   Mu    Sleeping                              ███░░  ████░  ███░░  █████             ││                              │
│   Pei   Sleeping                              █████  █████  █████  █████             ││                              │
│                                                                                      ││                              │
│ STORES                                                                               ││                              │
│   Wood 36  Stone 12  Fruit 21  Cabbage 17  Brick 4  Fish 0                           ││                              │
│                                                                                      ││                              │
│ TRAITS                                                                               ││                              │
│   Move speed x0.9  Carry capacity x1.3  Earthworks x2.5  Water work x0.9             ││                              │
│   Power generation x0.8  Construction x1.5                                           ││                              │
│                                                                                      ││                              │
│                                                                                      ││                              │
┗─ Arrows W S choose  Enter select  PgUp PgDn page  Esc/N close ───────────────────────┛┗─ seed 7 ─────────────────────┛
 NATION  arrows/W S choose  Enter select and close  Esc/N close  ? keys  Space pause
```

At 80x24 the same screen drops the meters:

```
┏───────────────── ◆ NATION ◆ ─────────────────┓┏───── ◆ EARTH KINGDOM ◆ ──────┓
│ Earth Kingdom · seed 7 · 20 people           ││ ▶ Running x1           05:00 │
│                                              │├─ PEOPLE ──────────────── 20 ─┤
│ IDLE 9                                       ││ Idle 9  Haul 4  Harvest 3    │
│  ▸Oma   Idle  Hungry!                        ││ Build 2  Sleep 2             │
│   Lin   Idle  Hungry!                        ││ Hungry 2  Exhausted 1    [N] │
│   Ping  Idle  Exhausted!                     │├─ STORES ─────────────────────┤
│   Tyro  Idle                                 ││ Wood 36  Stone 12  Fruit 21  │
│   Ming  Idle                                 │├─ RESEARCH ───────────────────┤
│   Song  Idle                                 ││ Masonry ██████░░░░ 68%       │
│   Haru  Idle                                 │├─ ENDEAVOR ───────────── 2/9 ─┤
│   Kyo   Idle                                 │├─ ALERTS ─────────────── [L] ─┤
│   Bo    Idle                                 ││ Learned Stoneworking         │
│ HAUL 4                                       │├─ CURSOR ───────── (110, 77) ─┤
│   Shu   Delivering 4 Wood to a stockpile     ││ Land                         │
│   Jin   Fetching 2 Fish from (96, 70)        ││ Plains (110, 77)             │
│   Lee   Delivering 3 Stone to a stockpile    ││                              │
│   Mai   Fetching 4 Wood from (88, 61)        ││                              │
│ HARVEST 3                                    ││                              │
│   Toph  Chopping Oak Tree at (76, 64), 40% … ││                              │
│   Yuan  Mining Stone at (81, 58), 10% done   ││                              │
│   Gan   Picking Fruit Tree at (90, 41), 75%… ││                              │
┗─ Enter select  Esc close  ▼ ─────────────────┛┗─ seed 7 ─────────────────────┛
 NATION  arrows choose  Enter select  Esc/N close  ? keys
```

**Why `N`.** The free letters are `E I J K M N V Y Z` (`P` is Stockpile and `C` is Sow). `N` for
"your nation" matches the screen's title and the panel's own title. The owner may prefer another
letter (question 1).

## The active tool (direction for #307)

**A banner across the top row of the map region while any tool other than Select is active.**

```
━ HARVEST ━ chop, mine, gather  ·  Enter: set a corner  ·  Esc: leave ━━━━━━━━━━━━━━━━━━
━ HARVEST ━ Enter: designate 5x3  ·  Esc: drop the corner ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
━ HARVEST · REPEATING ━ Enter: set a corner  ·  O: one-shot  ·  Esc: leave ━━━━━━━━━━━━━
━ BUILD Bed ━ Enter: build 1x1  ·  U: pick another  ·  Esc: leave ━━━━━━━━━━━━━━━━━━━━━━
━ BEND Raise Earth ━ Enter: set a corner  ·  B: next ability  ·  Esc: leave ━━━━━━━━━━━━
= HARVEST = Enter: set a corner - Esc: leave =========   (--ascii)
```

- **Look.** The whole row is a solid band in the tool's tint (`Tint::for_tool`: harvest amber,
  stockpile blue, build violet, bend the nation's accent, the repeating tint for a standing
  order), with bold `on_accent` text and a `━` fill. On 16 colours it uses the tint's solid
  colour. With `--color never` it is reverse video and bold. In `--ascii` the fill is `=`. Because
  it carries both text and shape, it never relies on colour alone.
- **Content.** The tool's name (plus the Build choice or Bend ability), then, as room allows: a
  short brief (a new `fn_ui::Tool::brief`, without the stale Shift text from #305), what `Enter`
  does now (today's `rect_hint`), and what `Esc` does now. The Cursor section stops repeating the
  `Enter` line.
- **Placement.** It covers the map's top row and moves nothing. Entering or leaving a tool shifts
  no tile on screen. The view treats that row as hidden, so the cursor never sits under it, the
  same way the selection bar hides the bottom 7 rows. A screen or modal hides the banner along
  with the map.
- **The status-line badge** takes the same tint, so the bottom-left badge and the banner match.
  Select keeps the accent badge.
- **With no tool there is no banner.** The difference is the signal. It also tells #306's player
  what `Esc` will do: in a tool the banner says "Esc: leave", and in Select the status line says
  "Esc menu".

**Why this over the alternatives.** Colouring the cursor in the tool's tint fails because the
cursor sits inside the dragged rectangle, which is drawn in the same hue, and because it does
nothing without colour. A border round the map costs two columns and a row in every mode, or
shifts the whole map when it appears. A bigger status badge stays at the bottom-left, where the
eye isn't. The banner is text on the map itself, a row the eye crosses all the time, and it costs
one row only while a tool is active.

**Contrast test:** `on_accent` must reach 4.5:1 on every tool tint for every nation, after
quantising to 256 and 16 colours.

## How it fits #299 and #306

- **#299 Season.** The section sits under People as difficulty.md says, with its heading in the
  divider format (`AUTUMN · Y1`, because `YEAR 1` plus `4:10 left` overflows 28 cells). Its alerts
  are one line each like every alert, with the full text in the log. Whichever of #299 and the
  panel issue lands second adapts the other: a re-bless, not a redesign.
- **#306 Esc menu.** It's a modal, so the panel doesn't change. The status line's Select keys gain
  `Esc menu`. The menu's title can carry the seed, a third place to find it.

## Not chosen

- **A top status bar** (time, population, food across the top row). It costs a map row in every
  mode (23 to 22 at 80x24) and duplicates the panel header, which this layout already shrinks to
  one row.
- **Tabs or pages in the panel** (1 Overview, 2 People, ...). They hide Alerts and Cursor, which
  must always be visible (pillar 4). They add another mode to a modal UI, and the number keys
  already switch nation.
- **Keeping a short roster when there's room** (say 6 lines at 120x40). It brings the
  half-the-panel problem back at the size the owner plays at, and the names are one key away.
- **A wider panel at 120+ columns.** Moving the roster out removes most of the truncation, so
  this isn't needed now. A `PANEL_WIDTH` that scales with the terminal stays possible later.

## Follow-up issues

- **#309** Panel: divider headings, a one-row header, compact sections, Tool next to Cursor,
  the new fitting order.
- **#308** Nation screen (`N`): grouped roster, stores, traits, seed; `fn_ui` activity counts
  and need flags; Modifiers leave the panel.
- **#310** Panel: People shows activity counts and need flags (blocked by #309 and
  #308).
- **#307** (existing): the tool banner as specified above.
- **#299**, **#306** (existing): notes added on those issues.

## Open questions for the owner

1. **Should the roster leave the panel for a Nation screen on `N`?** *Recommended: yes.* The
   panel shows counts and need flags, and the screen lists everyone with their full task. If you'd
   rather a different letter, `V` or `I` are free.
2. **Should the active tool get a full-width banner on the map's top row?** *Recommended: yes*,
   and #307 can build it now. The alternative is only a tinted status badge, which is cheaper but
   easy to miss.
3. **Should the seed, Modifiers and the `Space + -` hint leave the panel?** *Recommended: yes.*
   The seed moves to the panel's bottom edge and the Nation screen, Modifiers become the Nation
   screen's Traits, and the speed keys are already in the status line and key help.
4. **Should headings become dividers joined to the frame, with no blank rows and no `◇`
   bullets?** *Recommended: yes.* It's the biggest readability gain for the least code. The
   alternative is reverse-video heading bands, which are louder and fight the map for attention.
