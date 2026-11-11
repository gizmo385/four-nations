# Side panel v2: a glance, a feed, and what's under your hand

Doc status: current

Status: **accepted** (#414; owner answers 2026-11-06, see "Decisions" below; from the owner's 2026-11-06 midgame/endgame playtest). It revises
docs/design/side-panel.md (#302), which stays the record of why the roster left the panel and why
headings are dividers. Everything that doc settled about the Nation screen (`N`), the tool banner
and divider headings stands.

## The problem

The owner: "As we've increased complexity, it's become tough to fit everything in there."

Since #302 the panel has gained Season, Endeavor, need flags, the Bend ability list and a Tool
section. It now has **nine sections and nine heading rows**, each with its own fixed row cap, and a
fitting loop that takes rows away in a fixed order. The frames below are real: the economy
autoplay (`fn_sim::autoplay::Economy`, seed 7, Normal, Small) run to 100 minutes (Earth, Water) and
220 minutes (Fire), then rendered with `fn_tui::frame`.

**Fire, 220 minutes, 80x24, Select** (today):

```
""¥♣,♣,,,,,,,,▲▲,,,,~,,,,..≈≈≈≈≈≈≈≈≈≈.,,,,,♣,,,.┏────── ◆ FIRE NATION ◆ ───────┓
"♠"♠,,,,,,,,,▲▲▲,,☺☺,~,,..≈≈≈≈≈≈≈≈≈≈≈..,,,,,,...│ ▶ Running x1          220:03 │
""""",,,,,,,,▲▲▲,Θ☺...~..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,...≈≈├─ PEOPLE ──────────────── 20 ─┤
"¥"♠♠♠",,,,,,▲▲,,,☺Θ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,τ,.≈≈≈≈│ Idle 11  Haul 1  Craft 3     │
"""""ƒ,,,,,,,▲,,,,ΘΘ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.ƒ≈≈≈│ +5                           │
♠"""~♠,,¥,,,,,,,,,▦▦ΠΠ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.≈≈.≈│ Hungry 1  Thirsty 3      [N] │
"""~"",,,,,,,,,,,%▦▦ΠΠ%■■≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,...≈≈├─ AUTUMN · Y4 ──── 4:57 left ─┤
""~"",,¥,,,,,,¥,,,%Θ▤·Θ■■■≈≈≈≈≈≈≈≈≈≈≈≈.,,,¥♣,...│ Food in stores 605 · 30/head │
""~",,¥,,,,,,,,,,,☺▤%▤■Θ%≡≈≈≈≈≈≈≈≈≈≈.≈.,,,♣♣♣♣.≈├─ STORES ─────────────────────┤
"~τ",♣,,τ,,,,,,,,,%%%▤%ΘΘ£≡≈≈≈≈≈≈≈≈....,,,♣,♣♣..│ Wood 149  Stone 395          │
~""♠,♣,,,,,¥,,,,,,%☺☺%%▬☺Θ♦≈≈≈≈≈≈≈≈≈..,,,,♣,,,,♣├─ RESEARCH ───────────────────┤
."""♣,,,,,,,,,,,,¥▬τ☺■%%☺≡■≈≈≈≈≈≈≈ƒ.≈.,,,,♣,,,,♣│ No target set  10 pts banked │
..",♣♣,,,,,,,,,,,,¥%■■%%%¥≡≈≈≈≈≈≈≈≈≈..,,,,,,,♣♣,├─ ENDEAVOR ───────────── 8/9 ─┤
≈.",,♣,♣♣,,,,,,,,,ΨΨ■■■ΘΘ≡≡≈≈≈≈≈≈≈≈≈≈..,,,,♣,♣♣♣├─ CURSOR ────────── (92, 64) ─┤
≈."♣,♣,,,♣,,,,,,,,ΨΨΘ■■Θ%%≡≡≈≈≈≈≈≈≈≈≈≈..,,,♣,,♣,│ Ilah                         │
.."♣,,,,,,,,,,,,,,%·Θ··☺%%☺≈≈≈≈≈≈≈≈≈≈≈≈.τ,,,,¥,,│ Fire Nation - Idle:          │
.♠♠♣♣♣,,,,,,,,,,,,ΘΘ·Θ⚒■%■■≈≈≈≈≈≈≈≈≈≈≈≈.,,,♣,♣♣♣│ stockpile full               │
.♠,,,♣♣♣♣♣♣,,¥¥,,,ΦΦ⚒⚒Θ%■☺≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,...│ Carrying: nothing - Bends:   │
.♠,♣♣,♣,♣,,,,,,,,,ΦΦ⚒⚒ΩΩ%%≈≈≈≈≈≈≈≈≈≈≈≈..,,,♣..≈≈│ 26                           │
.♠,,,♣,♣♣,♣,,,,,,,ϞϞ,ΘΩΩ·%≈≈≈≈≈≈≈≈≈≈≈≈.,☺,...≈≈≈│ Press Enter to select        │
♠♠,,,,♣,,,,,,τ...,ϞϞ,...≈≈≈≈≈≈≈≈≈≈≈ƒ%..,,..≈...≈│ Stored: 9 Wood               │
¥♠♠"♠,♣,♣,,,,..≈..,,..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,..ƒ..,..│ Stockpile                    │
.....,,,,,,¥,.≈≈≈..,.≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,..≈≈.,,♣.┗─ seed 7 ─────────────────────┛
 SELECT  arrows/wasd move  Enter select  Esc menu  Tab units  H X P B U tools …
```

**Fire, 220 minutes, 120x40, Bend tool** (today; panel only):

```
┏────── ◆ FIRE NATION ◆ ───────┓
│ ▶ Running x1          220:03 │
├─ PEOPLE ──────────────── 20 ─┤
│ Idle 11  Haul 1  Craft 3     │
│ Eat 4  Sleep 1               │
│ Hungry 1  Thirsty 3      [N] │
├─ AUTUMN · Y4 ──── 4:57 left ─┤
│ Food in stores 605 · 30/head │
│ Winter here: mild            │
├─ STORES ─────────────────────┤
│ Wood 149  Stone 395          │
│ Gold Ore 28  Metal 7         │
├─ RESEARCH ───────────────────┤
│ No target set        3.0/min │
│ 10 pts banked                │
├─ ENDEAVOR ───────────── 8/9 ─┤
│ Next: 3 more scrolls re… [G] │
├─ ALERTS ─────────────── [L] ─┤
│ Ilah used Hearthflame at…    │
│ Ran 2 used Hearthflame at…   │
│ Noriko 2 used Hearthflame a… │
├─ BEND ───────────────── [B] ─┤
│ ▸Hearthflame      20 benders │
│  Inner Fire       20 benders │
│  Weld             20 benders │
├─ CURSOR ────────── (92, 64) ─┤
│ Ilah                         │
│ Fire Nation - Idle:          │
│ stockpile full               │
│ Carrying: nothing - Bends:   │
│ 26                           │
│ Stored: 9 Wood               │
│ Stockpile                    │
│                              │
│                              │
│                              │
│                              │
│                              │
┗─ seed 7 ─────────────────────┛
```

What goes wrong:

1. **At 80x24, things vanish.** The ALERTS section is gone entirely, heading included (fitting
   takes alerts to 0 and an empty section hides; side-panel.md promised "alerts beyond the newest",
   #415). Endeavor is a bare `8/9`. Stores is one row: Wood and Stone, while food, ore, metal and
   tools are off screen. With a tool active, People becomes `Idle 11 Haul 1 Craft 3` / `+5`.
2. **At 120x40, space goes to the wrong place.** Cursor gets 16 rows and uses 7. Alerts, capped at
   three, are cut mid-word (`Noriko 2 used Hearthflame a…`), and Stores stays capped at two rows.
   The caps don't grow with the terminal; only Cursor does.
3. **Alerts are noise.** All three rows are routine "X used Hearthflame at (x, y)" successes.
4. **The one late-game line you need is truncated:** `Next: 3 more scrolls re… [G]`.
5. **Duplication.** Season's "Food in stores" repeats what Stores is for. Cursor repeats the
   selection bar (name, task, carrying, bends) for a selected unit, and prefixes your own
   nation's name on every unit (`Fire Nation - Idle: stockpile full`).
6. **Research with no target is quiet.** Two dim rows (`No target set 3.0/min`, `10 pts banked`)
   for the state that most needs the player's attention.
7. **Headings cost a third of the panel.** Nine heading rows out of 21 at 80x24 carry one word
   each.

## The idea

#302 sorted the panel into what you **watch** and what you **look up**. That still holds. What's
changed is that there is now too much to watch for one heading per thing. So:

- **Things you watch get one labelled row each, under one heading.** "Food 605 · 30/head" needs
  no heading of its own; the label is the heading.
- **Things that happen get a feed** that is always on screen, never spends a row on routine
  successes, and grows when there is room.
- **Things under your hand** (the tool and the tile) stay last, and stop repeating what other
  parts of the screen already show.

Three zones, three or four heading rows, and every row has a fixed place, so nothing jumps.

## The panel

### Zone 1: header and Settlement glance (fixed height)

| Row | Content | Replaces |
|---|---|---|
| Header | Two rows with one shape from tick 0 (#654): `▶ Running x1` (or `‖ Paused x1`), then the clock, `Autumn Y4 · 4:57 left`. With seasons off, `Day 3 · 2:10 left` | Header + the Season heading |
| Heading | `├─ SETTLEMENT ─────── [N] ─┤` | PEOPLE, season, STORES, RESEARCH, ENDEAVOR headings |
| People | `People 20` left, `Idle 11` right (Warn tone when idle is more than half and there is designated work) | The activity-count rows (the breakdown is on `N`) |
| Needs | `Hungry 1  Thirsty 3`, toned as today; hidden when nobody is flagged | Same |
| Food | `Food 605 · 30/head` and the winter outlook word (`❄ mild`, `* mild` in ASCII); Warn tone under `winter_food_per_head` | Season's two rows |
| Materials | Non-food items in a fixed data order, packed into one row, `+N` fold. A second row when there is spare height | Stores |
| Research | Short shape: `Tunneling ████████░░ 77%`. No target: `Research: none set [T]` in Warn tone | Research's two rows |
| Gate | `Gate 8/9 · 3 scrolls [G]`: the count and the first missing sign by a new short label (`short` in data/endeavor.toml) | Endeavor's heading and Next row |

Seven rows plus the header (eight with need flags), at every size. Nothing in this zone is ever
trimmed; the only growth is the second materials row.

Why People loses the activity breakdown: at 20+ settlers the breakdown takes two or three rows and
changes every tick, while the only number that asks for an action is Idle. The full breakdown
stays one key away on the Nation screen, grouped the same way (decision 1).

The elapsed game clock leaves the header while seasons are on (decision 4); it moves to the
Nation screen's header line and the Esc menu.

### Zone 2: Alerts (always visible, grows first)

- The heading always shows, with at least the newest alert under it while any toast is live
  (#415 fixes this in today's panel too).
- **Routine successes don't toast.** An ability cast, a building used: log only (`L`). Warnings,
  failures, deaths, research learned and season changes still toast.
- **Repeats coalesce:** the same kind and subject inside the toast window is one line with a count,
  `Hearthflame failed ×3`.
- **No coordinates in toasts.** They stay in the log entry, which is where you'd go to find the
  place anyway.
- **Spare height comes here first**, up to 6 rows, before Here grows.

### Zone 3: Here (Tool, then Cursor)

*The Here section and the selection bar it mentions were replaced by the inspector (#653, see
"The inspector" below); the Tool section stands.*

- The Tool section keeps its tinted heading, but every tool is **one row**: Build shows the
  chosen building and cost (as today), Bend shows the current ability and its bender count
  (`▸Hearthflame 20 benders`). `B` already cycles, and the banner already says `B: next ability`.
  Then the last result, as today.
- CURSOR becomes **HERE**, keeping the coordinates on the heading.
- A unit under the cursor shows its name, its task and `Enter: select`. The own-nation prefix
  shows only when the settlement has more than one nation (after the transformation).
  `Carrying: nothing` is omitted, and bends live in the selection bar only (it already says
  `Bends 9 of 25`).
- While the selection bar is open for the unit under the cursor, Here shows the tile only.
- `Stored: 9 Wood` plus `Stockpile` become one line, `Stockpile: 9 Wood`.

### Fitting

Zone 1 never gives up rows. Spare rows go, in order: Alerts up to 6, the second materials row,
then Here. When short, Alerts gives up rows down to 1, then the Tool result down to one line, then
Here down to 3 rows. At 80x24 with need flags, a tool and three alerts, that's 1 + 8 + 4 + 2 + 4 =
19 of 21 rows: everything fits, which today's panel can't do.

## Mockups

These are laid out by hand over the real 220-minute Fire map; the panel content is what the rules
above would produce.

**80x24, Select:**

```
""¥♣,♣,,,,,,,,▲▲,,,,~,,,,..≈≈≈≈≈≈≈≈≈≈.,,,,,♣,,,.┏────── ◆ FIRE NATION ◆ ───────┓
"♠"♠,,,,,,,,,▲▲▲,,☺☺,~,,..≈≈≈≈≈≈≈≈≈≈≈..,,,,,,...│ ▶ x1     Autumn Y4 4:57 left │
""""",,,,,,,,▲▲▲,Θ☺...~..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,...≈≈├─ SETTLEMENT ─────────── [N] ─┤
"¥"♠♠♠",,,,,,▲▲,,,☺Θ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,τ,.≈≈≈≈│ People 20            Idle 11 │
"""""ƒ,,,,,,,▲,,,,ΘΘ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.ƒ≈≈≈│ Hungry 1  Thirsty 3          │
♠"""~♠,,¥,,,,,,,,,▦▦ΠΠ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.≈≈.≈│ Food 605  30/head  ❄ mild    │
"""~"",,,,,,,,,,,%▦▦ΠΠ%■■≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,...≈≈│ Wood 149  Stone 395  +5      │
""~"",,¥,,,,,,¥,,,%Θ▤·Θ■■■≈≈≈≈≈≈≈≈≈≈≈≈.,,,¥♣,...│ Research: none set       [T] │
""~",,¥,,,,,,,,,,,☺▤%▤■Θ%≡≈≈≈≈≈≈≈≈≈≈.≈.,,,♣♣♣♣.≈│ Gate 8/9 · 3 scrolls     [G] │
"~τ",♣,,τ,,,,,,,,,%%%▤%ΘΘ£≡≈≈≈≈≈≈≈≈....,,,♣,♣♣..├─ ALERTS ─────────────── [L] ─┤
~""♠,♣,,,,,¥,,,,,,%☺☺%%▬☺Θ♦≈≈≈≈≈≈≈≈≈..,,,,♣,,,,♣│ Hearthflame failed ×3        │
."""♣,,,,,,,,,,,,¥▬τ☺■%%☺≡■≈≈≈≈≈≈≈ƒ.≈.,,,,♣,,,,♣│ Learned Weld                 │
..",♣♣,,,,,,,,,,,,¥%■■%%%¥≡≈≈≈≈≈≈≈≈≈..,,,,,,,♣♣,│ Bao is thirsty               │
≈.",,♣,♣♣,,,,,,,,,ΨΨ■■■ΘΘ≡≡≈≈≈≈≈≈≈≈≈≈..,,,,♣,♣♣♣├─ HERE ──────────── (92, 64) ─┤
≈."♣,♣,,,♣,,,,,,,,ΨΨΘ■■Θ%%≡≡≈≈≈≈≈≈≈≈≈≈..,,,♣,,♣,│ Ilah                         │
.."♣,,,,,,,,,,,,,,%·Θ··☺%%☺≈≈≈≈≈≈≈≈≈≈≈≈.τ,,,,¥,,│ Idle: stockpile full         │
.♠♠♣♣♣,,,,,,,,,,,,ΘΘ·Θ⚒■%■■≈≈≈≈≈≈≈≈≈≈≈≈.,,,♣,♣♣♣│ Enter: select                │
.♠,,,♣♣♣♣♣♣,,¥¥,,,ΦΦ⚒⚒Θ%■☺≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,...│ Stockpile: 9 Wood            │
.♠,♣♣,♣,♣,,,,,,,,,ΦΦ⚒⚒ΩΩ%%≈≈≈≈≈≈≈≈≈≈≈≈..,,,♣..≈≈│                              │
.♠,,,♣,♣♣,♣,,,,,,,ϞϞ,ΘΩΩ·%≈≈≈≈≈≈≈≈≈≈≈≈.,☺,...≈≈≈│                              │
♠♠,,,,♣,,,,,,τ...,ϞϞ,...≈≈≈≈≈≈≈≈≈≈≈ƒ%..,,..≈...≈│                              │
¥♠♠"♠,♣,♣,,,,..≈..,,..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,..ƒ..,..│                              │
.....,,,,,,¥,.≈≈≈..,.≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,..≈≈.,,♣.┗─ seed 7 ─────────────────────┛
 SELECT  arrows/wasd move  Enter select  Esc menu  Tab units  H X P B U tools …
```

**120x40, Bend tool:**

```
━ BEND Hearthflame ━ Enter: set a corner  ·  B: next ability  ·  Esc: leave ━━━━━━━━━━━━┏────── ◆ FIRE NATION ◆ ───────┓
♠♠"♠♠♠♠♠♠""""♣♣♣,,♣,,,,,,,,,··♣♣♣,♣♣,♣♣,,,·,,,,,,...≈...,,...≈≈.≈≈≈≈≈≈≈≈≈≈..."♠♠♠♠♠""♠♠♠│ ▶ x1     Autumn Y4 4:57 left │
♠♠¥♠...♠"♠♠""",,,♣,,,,♣,,,♣····▲♣♣♣,,♣♣♣,,τ♣,,,,..≈≈≈ƒ≈.,,,,...≈≈≈≈≈≈≈≈≈..."""♠""♠♠"♠♠♠♠├─ SETTLEMENT ─────────── [N] ─┤
♠♠♠♠.≈.♠""""""",♣,,τ,,,,,,,,···▲▲•▲▲▲,,♣♣♣♣,¥,,,.≈≈≈≈≈..,♣,,,,..≈≈≈≈≈≈≈..♠♠"""""""""♠"♠♠│ People 20            Idle 11 │
♠♠♠♠...""""♠♠""",,♣,,,,,,,,,♣,,,▲$▲▲▲,,,,,,,%,,..≈≈≈≈..,,τ,,,,,.≈≈≈≈≈...♠♠♠"♠"""""♠"♠"..│ Hungry 1  Thirsty 3          │
"♠♠♠♠♠♠""""♠""♠"",,♣,,,,♣,,,,,♣,▲▲▲▲,,,,,,,,,τ,.≈≈≈≈≈≈..,,,,,,,.≈≈≈≈≈.."""♠"""♠""""""♠.≈│ Food 605  30/head  ❄ mild    │
"♠♠♠♠♠""""♠♠"♠♠""",,,,,,,,,,,,τ,,▲▲▲,,,,,,,,,,..≈≈≈≈≈≈≈%.,,☺,,,.≈..≈≈≈.",""""""♠""♠""√.~│ Wood 149  Stone 395  Ore 28  │
♠♠♠♠♠♠♠♠"♠"♠♠""♠♠"♠,,,,,,,,,,,,,,▲▲▲,☺,~,,,,,,.≈≈≈≈≈≈≈ƒ≈..,,,,,.....≈..",τ""♠""""▲▲$▲♣~√│ Metal 7  Brick 40  Tool 12   │
♠"♠""♠♠"♠♠♠""""¥""♠¥""¥♣,♣,,,,,,,,▲▲,,,,~,,,,..≈≈≈≈≈≈≈≈≈≈.,,,,,♣,,,.≈.,♠,,,"""""●τ•▲♣√~√│ Research: none set       [T] │
♠"♠""♠♠♠"♠♠♠♠♠♠♠""♠""♠"♠,,,,,,,,,▲▲▲,,☺☺,~,,..≈≈≈≈≈≈≈≈≈≈≈..,,,,,,...≈.,,♠,"¥♠"♠♠""▲♠●√~√│ Gate 8/9 · 3 scrolls     [G] │
♠"▲""♠♠"♠"♠♠♠♠¥♠♠"♠"""""",,,,,,,,▲▲▲,Θ☺...~..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,...≈≈≈.¥♠"""♠"♠"♠"""♠√√~√├─ ALERTS ─────────────── [L] ─┤
♠"♠♠♠♠"♠♠♠♠♠""♠♠"""""¥"♠♠♠",,,,,,▲▲,,,☺Θ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,τ,.≈≈≈≈≈.♠♠♠"♠""""""¥""♣√~√│ Winter will be mild          │
♠""""♠♠♠♠♠""♠♠"""¥¥""""""ƒ,,,,,,,▲,,,,ΘΘ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.ƒ≈≈≈≈~♠♠♠""♠"♠♠"""♠♠√♣√~│ Learned Weld                 │
"""♠♠♠♠♠♠♠♠♠♠♠♠"""""♠"""~♠,,¥,,,,,,,,,▦▦ΠΠ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.≈≈.≈≈.~♠♠"♠"♠♠♠♠""♠"√√√√│ Hearthflame failed ×3        │
"""♠"♠♠♠♠♠♠♠♠♠"♠"""""""~"",,,,,,,,,,,%▦▦ΠΠ%■■≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,...≈≈≈.♠~♠""♠♠"♠"""""♠♣√~│ Ilah: stockpile full         │
"""♠♠""♠♠♠...~..♠"♠♠""~"",,¥,,,,,,¥,,,%Θ▤·Θ■■■≈≈≈≈≈≈≈≈≈≈≈≈.,,,¥♣,.....♠♠~♠♠♠♠...♠""♠√√~√│ Bao is thirsty               │
"""♠√♠♠♠♠..≈≈≈≈...""""~",,¥,,,,,,,,,,,☺▤%▤■Θ%≡≈≈≈≈≈≈≈≈≈≈.≈.,,,♣♣♣♣.≈.♣♠♠♠~~"♠.≈."♠♠♠τ~√√├─ BEND ───────────────── [B] ─┤
"""♣♣~"♠♠.≈≈≈≈≈≈≈..""~τ",♣,,τ,,,,,,,,,%%%▤%ΘΘ£≡≈≈≈≈≈≈≈≈....,,,♣,♣♣...♣♠♠♠♠♠ƒ~~≈.♠♠♠♠~♠♠♠│ ▸Hearthflame      20 benders │
""..~."...≈≈≈≈≈≈≈≈..~""♠,♣,,,,,¥,,,,,,%☺☺%%▬☺Θ♦≈≈≈≈≈≈≈≈≈..,,,,♣,,,,♣,♣,♠♠♠♠♠"..~~"~~""""├─ HERE ──────────── (92, 64) ─┤
♠..≈≈...≈≈ƒ≈≈≈≈≈≈≈≈~."""♣,,,,,,,,,,,,¥▬τ☺■%%☺≡■≈≈≈≈≈≈≈ƒ.≈.,,,,♣,,,,♣♣♣♣♣♠♠♠"♠♠"♠"~""♠¥"♠│ Ilah                         │
".≈≈≈ƒ≈≈≈≈≈≈≈≈≈≈≈≈≈≈..",♣♣,,,,,,,,,,,,¥%■■%%%¥≡≈≈≈≈≈≈≈≈≈..,,,,,,,♣♣,,♣,,♠"♠♠♠♠"♠♠"""♠♠""│ Idle: stockpile full         │
..≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.",,♣,♣♣,,,,,,,,,ΨΨ■■■ΘΘ≡≡≈≈≈≈≈≈≈≈≈≈..,,,,♣,♣♣♣,♣♣,,♣,¥"♠♠♠♠""¥"♠"♠│ Enter: select                │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈."♣,♣,,,♣,,,,,,,,ΨΨΘ■■Θ%%≡≡≈≈≈≈≈≈≈≈≈≈..,,,♣,,♣,♣♣♣,♣♣♠♣♣,♠♠♠"""""""│ Stockpile: 9 Wood            │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.."♣,,,,,,,,,,,,,,%·Θ··☺%%☺≈≈≈≈≈≈≈≈≈≈≈≈.τ,,,,¥,,τ♣♠♠♠♠♣♠♠"♠♠"♠""""""│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠♠♣♣♣,,,,,,,,,,,,ΘΘ·Θ⚒■%■■≈≈≈≈≈≈≈≈≈≈≈≈.,,,♣,♣♣♣,♠♠"♠"♠♠♠♠"♠"♠"♠""♠,│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠,,,♣♣♣♣♣♣,,¥¥,,,ΦΦ⚒⚒Θ%■☺≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,....~~~♠"♠"♠♠♠♠♠♠""""♠♠│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠,♣♣,♣,♣,,,,,,,,,ΦΦ⚒⚒ΩΩ%%≈≈≈≈≈≈≈≈≈≈≈≈..,,,♣..≈≈~,""~♠"~~♠♠♠♠τ"♠""""│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈..♠,,,♣,♣♣,♣,,,,,,,ϞϞ,ΘΩΩ·%≈≈≈≈≈≈≈≈≈≈≈≈.,☺,...≈≈≈.♣,""~~♠♠~"♠♠"♠"""♠♠│                              │
≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠♠,,,,♣,,,,,,τ...,ϞϞ,...≈≈≈≈≈≈≈≈≈≈≈ƒ%..,,..≈...≈..,♣τ♠♠♠""ƒ♠♠♠♠♠"♠♠♠│                              │
.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.¥♠♠"♠,♣,♣,,,,..≈..,,..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,..ƒ..,..≈..,,♠♠♠♠♠"~~♠"♠♠♠♠♠│                              │
.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈......,,,,,,¥,.≈≈≈..,.≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,..≈≈.,,♣..≈.","♣♣♠♠♠♠♠ƒ~~♠♠♠~│                              │
.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈....,,,,,.≈≈≈≈.,.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,.≈...,,,,..~"""♠,,♠"♠♠♠♠♠~~~♠│                              │
.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈...,,,.≈≈≈≈....≈≈≈≈≈≈≈≈≈ƒ≈..,,,,...,,♣♣,,,,"~"""♣,♣,♠"♠♠♠"♠""│                              │
.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈..,,.≈≈≈≈≈ƒ≈.≈≈≈≈≈≈≈≈≈%..,,,,,,,,,,,♣♣,,♣""~"♠♣,,,,,,"♠♠""♠│                              │
.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈ƒ≈..,.....≈≈≈≈≈≈≈≈≈≈≈≈..,,,,,,,,,,,♣,,,,,¥"~"""♣,♣♣,,,,,,"""│                              │
≈≈≈≈≈...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈..,.≈....≈≈≈≈≈≈≈....☺♣,,,,,,,,,,,,♣♣,♣"~"""""♣♣♣,,,,♣,♠♠"│                              │
.≈≈...♠.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.....,,....≈≈≈≈.♣,,♣,,,,,,,,♣,,♣,,,♣,""~"""",,♣♣♠♠♠♠♠♠""│                              │
...."♠τ...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈ƒ≈..♣,♣,♣,,......,,,,,♣,,,,,,,,♣,,,,,,,♠¥~""",♣,,♠.......│                              │
¥""""♠♠♠♠..≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,♣♣,,♣♣,,,♣,,τ,,▲▲,♣,♣♣♣,,,♣,♣""~"♠♠♠,♣♣,♠.≈ƒ≈.≈.┗─ seed 7 ─────────────────────┛
 BEND  Enter set corner  Esc leave  |  Kindle: A flame that needs no fuel. Meals eaten near it are cooked, and sleep i…
```

In ASCII the shapes carry the same structure as today (`+- SETTLEMENT --- [N] -+`), and every
Warn state also has a word, not just a colour (`none set`, the need flag words).

## The inspector (#653)

From the owner's 2026-11-10 UI review. At 120x40 the panel used 13 to 19 of its 38 rows and left
the rest blank, while a selected settler opened in a 7-row bar over the bottom of the map. The
details of one thing were split across three places: HERE (the tile), the bar (the settler) and
the Nation screen (the same settler again), each drawn differently, and HERE printed the cursor
position twice. **Owner decision: retire the unit bar; the selected settler is shown in the side
panel.** This supersedes the "Here" zone above and the selection bar it refers to.

- **The last section is the inspector** (`crate::inspector`). Its heading names what it
  describes and is given every row left after the sections above it. Text in it wraps and is
  never cut while a row is free.
- **A selected settler** (`Enter`, `Tab`, `F` follows, `Esc` deselects, all as before): the
  heading is the settler's name with its rank on the right (`BAI ── Student`). Under it: the task
  (the Avatar holding the Harmony Rite in his own tone), what it carries, the five meters with
  percent, the progress to the next rank, and the tile it stands on (`Standing on forest
  (52, 56)`, or `In the Academy (52, 56)` inside a building). The bar's meter rows are reused;
  `crate::bar` is deleted and the map keeps its full height.
- **Otherwise, what is under the cursor.** The heading names the first thing the tile has: a
  settler, a building (`BED`, `BED (UNDER CONSTRUCTION)`), a resource, `STOCKPILE` for a
  stockpile zone (what it holds first, `Wood 20 of 20, full`, then the terrain under it), or the
  terrain itself. The cursor position is on the heading's right, so it appears once. A name too
  long to share the heading with the position (`Plains on the coast`) hands the position to the
  first row instead. The building's anchor line (`Built (x, y)`) is not shown: it is the
  position again. The agent's `inspect` keeps its lines (it is the tool's own text, not the
  panel's).
- **While a tool is active the inspector describes the cursor tile**, even with a settler
  selected: the tool acts on the cursor, and the Build and Gate tools' `Can build here` line has
  to stay where it was. `Esc` leaves the tool and the settler comes back. (A call the owner may
  want to revisit: the alternative is the settler with the placement line above it.)
- **A low meter** is drawn in its warning (or bad) colour with the word `low` after the percent
  (`Thirst ████░░░░░░  40% low`), where the bar had a `!`. Chi never warns.
- **Fitting.** The sections above the inspector shrink as before (alerts to the newest, the tool
  result to two lines) until it keeps its floor: a settler's task, carrying and meters, three
  rows of a tile. Past that the sections above win and the inspector drops rows from the bottom:
  blank rows between its groups first, then the tile line, then the rank progress, and only then
  the meters from the last up. At 80x24 with a settler selected that is the task, carrying and
  the five meters.
- **Tests.** The layout property test (`tests/layout_properties.rs`) checks that the panel ends
  in a heading of the inspector's own and that no ellipsis is drawn in it while the last row
  before the panel's bottom edge is blank, at every size in its matrix. A frame test checks the
  map is untouched by a selection. The golden frames `ref_game_80x24`, `ref_game_wide` and
  `ref_menu` were re-blessed for the new heading (`JUNI (64, 28)` for `HERE (64, 28)`) and the
  rows under it.

Out of scope, as the issue says: grouping and wrapping alerts, what a tool's section shows, and
the SETTLEMENT glance.

## Not chosen

- **Tabs or pages in the panel.** Still no, for #302's reason: Alerts and Here must always be
  visible, and the number keys already switch nation.
- **A top resource bar across the map.** It costs a map row in every mode. The glance block gives
  the same information in the panel's own rows, which the header already pays for.
- **Keeping one heading per section and only tuning caps.** The caps are the problem: every new
  feature adds a heading and another row to argue about. One row per watched thing scales.

## Width

The panel is 36 columns on terminals at least 140 columns wide, 32 below that (decision 3, #423).
Every row packs to the panel's real inner width, so the extra 4 cells go to longer alert and Here
lines rather than padding. At 120x40, the size the mockups use, nothing changes.

## Follow-up issues

- **#415** Bug: the Alerts section vanishes at 80x24 (independent of this doc).
- **#416** Panel v2: one-row header with the season, and the Settlement glance block (blocked
  by #414, #415).
- **#417** Panel v2: alerts coalesce, routine events log-only, spare rows to alerts (blocked by
  #414).
- **#418** Panel v2: the Here section (blocked by #414).
- **#423** Panel v2: 36 columns on terminals 140+ wide (blocked by #414, #416).

## Decisions

The owner answered on 2026-11-06:

1. **People shows `People 20` and `Idle 11` only**; the activity breakdown is the Nation screen's.
2. **Routine successes (ability casts, buildings used) go to the log only**, never to toasts.
3. **The panel widens to 36 columns from 140 columns wide** (#423).
4. **The clock is the season and its time left, everywhere** (#654, owner decision 2026-11-10): the panel, the Nation header, the game menu, the saves list and the message log (a heading row per season, each line stamped with the time left in it). No screen shows minutes since the start; run logs and scorecards keep their own tick-based times. With seasons off the same shape runs on days (`Day 3 · 2:10 left`, a day being five game minutes).
