# End game: the Harmony Gate, built with the Avatar

Status: **decided** (#224, owner, 2026-10-25). The owner's answers are under "Owner decisions",
and follow-up work is listed in "Follow-up issues".

The game has no long-term goal yet. A settlement researches its tree in about two hours, builds,
and then just keeps going. The owner's idea (2026-10-25) is an end-game **great work that needs
the Avatar**: after a long list of achievements, the player builds one big thing *with* the
Avatar, and finishing it changes what late-game play looks like.

## Decisions in brief

1. **One shared endeavor, the Harmony Gate**, dressed in each nation's materials. A 4x4 plaza
   with four element pillars and a gate arch, built at the heart of the settlement. Finishing it
   opens the settlement to people of all four nations. This follows Aang's own post-war story,
   when he founded a city where the nations live together, and it fits vision.md's "Avatar is
   about balance and unity".
2. **The road is ten "signs"** in data: population, beds, building variety, the nation's whole
   research branch, stores, farm and mine output, bending masters and scrolls. A screen (`G`)
   and a side-panel section show progress from minute one. Once every sign is met at the same
   moment, the endeavor **latches** as ready, and later dips don't take it away.
3. **Aang arrives on Appa as a guest** once the foundation is laid. He has his own AI and is
   never drafted. He claims Avatar tasks and any bend job, and can cast **all four signature
   abilities**. He eats, sleeps and leaves if neglected, but never dies.
4. **Four stages:** foundation (settlers), the Four Pillars (one bend per element; your own
   benders can raise your element's pillar), the Arch (settlers, plus the Avatar lifting the
   beam), and the **Harmony Rite**: the Avatar in the Avatar State while settlers gather on the
   plaza. A death breaks the rite, and too few people on the plaza pauses it. The whole project
   takes about an hour.
5. **The transformation is the United City.** Newcomers of all four nations start to arrive,
   each with their own nation's modifiers and signature ability. A Water settlement gains Earth
   benders who raise fords, the bend picker shows all four abilities, and the player plans
   around who can do what. That changes moment-to-moment play, not a stat.
6. **Everything that varies is data** (`data/endeavor.toml`), and the sim stays deterministic. The
   only randomness is the newcomers' nation and name, from `rng::stream(seed,
   "endeavor.newcomers")`. New state is `serde(default)`, so there's no `SAVE_VERSION` bump.

## Concepts considered

| Concept | Avatar | What changes after | Cost | Verdict |
|---|---|---|---|---|
| **A. The Harmony Gate (United City)** | Aang | Settlers of all four nations arrive with their own bending. Mixed abilities change daily play | Medium. Per-unit nation and population growth, both of which the game wants anyway | **Chosen** (owner, 2026-10-25) |
| B. A Spirit Portal | Korra | Spirit vines (a new resource) spread from the portal, and spirits wander in. They bless a settlement in balance and blight one that isn't | High. Spirit AI, a balance or mood system, and possibly a second map for the spirit world | A strong sequel: a second endeavor later |
| C. A per-nation wonder (restore an Air Temple, a Moon and Ocean shrine, an Earth great bridge, a Sun Warrior eternal flame) | Each nation's own Avatar | A different transformation per nation | 4x design, 4x art and 4x balance, and the four transformations are hard to keep equal | No. It is better used as flavour inside A (each nation builds "its" pillar) |
| D. Calm a spirit storm | Any | Mostly removes a threat | Low, but a storm is a disaster system, and the payoff is a stat | No. It could become a trial during a later endeavor |

**Why A.** It is the only concept whose payoff uses systems the game already has, the four
signature abilities, in a new combination, so the late game gets new decisions without a new
rulebook. It also delivers the "neighbors and cultural exchange" direction research.md sketched,
without trade or diplomacy screens, and with no war.

## The road: ten signs

The signs are a list in `data/endeavor.toml`. Every sign counts things the settlement does
anyway, so the road rewards playing well rather than a separate grind.

| Group | Sign (default target) | Counts | Depends on |
|---|---|---|---|
| People | Population **16** | Living units (the Avatar excluded) | Population growth (#232) |
| People | Everyone has a bed | Beds ≥ population | #58 |
| Buildings | **8** kinds of building raised | Distinct finished building ids | #64, #196 |
| Knowledge | The nation's branch complete | Every tech with `nation = <own>` known | #19, #25–#28 |
| Stores | **150** food in stores | Food items stored, summed | #3 |
| Stores | **200** stone in stores | Stored | |
| Harvests | **100** crops harvested from farm plots | Lifetime tally | #198 |
| Harvests | **40** ore mined | Lifetime tally of `gold_ore` (later metal ore) | #132, #134 |
| Bending | **3** bending masters | Units with at least 25 finished bends | #67 |
| Bending | **3** ancient scrolls read | Lifetime tally | #24 |

- **Pacing target:** about 3.5 to 4 hours for a steady player. That's the research tree (~2 h)
  plus building out and growing to 16. The project itself adds about an hour. Tune with the
  targets first.
- **Latching.** The endeavor becomes **Ready** the first tick every sign is met at once. Stores
  that are eaten afterwards don't undo it. The toast reads "Word of your settlement has reached
  the Avatar. Choose a site for the Harmony Gate (G)."
- **Per nation.** A sign can name per-nation values (`target = { air = 14, ... }`), and the
  branch sign is per nation by construction. Air's lighter labour can get a lower stone target,
  for example. Defaults are shared.
- **A sign whose system doesn't exist yet** (farm plots before #198, ore before #132) is left out
  of the data until its system lands. Validation rejects a sign kind the sim can't count, so the
  list stays honest, as `research_earth.rs` does for unlocks.

**Showing progress.**

- **The Endeavor screen (`G`):** a centred Seal screen like Research (`T`): the gate in grey,
  coloured in up to the share of signs met, then the signs grouped with 10-cell text bars like
  the selection bar's meters, a check for each sign met, and a warn-coloured hint on any sign
  that isn't met ("Spare beds and food draw newcomers"). After Ready it shows the stages
  instead. The side panel gets an **Endeavor** section under Research: name, `7/10`, a bar, and
  the next three missing things. A toast fires when a sign is first met. The texts come from one
  shared function in `fn_ui`, as research does (shipped first for the Bevy client and the TUI
  together, #235; TUI-only since the client was removed, #281).
- **Readable simulation:** hovering a sign says what feeds it ("Ore mined: 34 of 40. Mine gold
  veins; tunnels yield more").

## The Avatar

**Which Avatar: Aang, with Appa.** The game is *The Last Airbender*, the Harmony Gate is Aang's
story (unity after the war), and an airbender's speed suits a visitor who works all over the
site. Appa rests beside the site while Aang stays, as a 3x2 prop. Appa is decoration in v1 and
does no hauling, so he doesn't pre-empt the sky bison work in #205. Companions (Katara, Sokka,
Toph, Zuko) aren't in v1 (owner decision 1).

**Arrival.** Two in-game minutes after the foundation stage completes, Appa lands on the
nearest free tiles west of the site (searched in row-major order, so it is deterministic), and
Aang steps off. A toast and a camera ping announce it.

**Control: a guest with his own AI** (owner decision 3). The Avatar is a unit in `World::units`,
marked `guest = Some(Guest::Avatar)`. He uses the normal job loop with a different priority
list:

1. his needs, like anyone;
2. **Avatar tasks**: endeavor jobs that only he can claim (the beam lift, the rite);
3. **pillar tasks** of any element (your own benders compete with him only for your nation's
   pillar);
4. **bend jobs** for any of the four signature abilities, which the player designates as usual
   with `B`. While he is here, the bend picker shows all four, marked "Avatar";
5. otherwise he idles on the plaza and meditates (a cosmetic pose).

He never hauls, builds, studies or harvests, so he can't be used as a super-worker, and the
player never drags him around. That keeps pillar 2 (indirect control). Being able to designate
"his" bends is the fun part: an Earth settlement can finally Wind Carry a far stack, or a Fire
settlement can Water the Land.

**Numbers** (all in `[avatar]`): chi max 200 and chi regen ×2. Work modifiers are the best of the
four nations (earthworks 2.5, water_work 2.0, heating 2.5, power_generation 2.0, construction
1.5), and move_speed is 1.6.

**Needs and leaving.** He has the normal needs. He is vegetarian (`refuses = ["fish"]`, a canon
touch and a real constraint for a fish-fed Water settlement), and he claims any free bed. If his
hunger or energy stays below `low` for **20 minutes** (no food he eats, no bed), he leaves on
Appa: "Aang has gone to find food. He'll be back." He returns **30 minutes** later, and stage
progress is kept. He never dies, and his needs never pre-empt the rite once it has started.

**How long he stays:** from the foundation to the end of the rite, about 45 to 60 minutes of
play. Then he gives a farewell, stays one in-game day for the celebration, and flies off. He
doesn't come back in v1. Later visits with small requests are an extension, not part of this
design.

**#78 is reshaped** into this. The first canon character is the Avatar, defined in data
(`[avatar]`), not special-cased in code, and drawn with our own 64x96 sheet and 32x32 portrait.
Companions or other canon characters can follow the same pattern later.

## The project: four stages

The Harmony Gate is a building def with `footprint = [4, 4]` (the largest #196 allows),
**walkable** (it is a plaza), and `endeavor = true`, which keeps it out of the build picker.
After Ready, the Endeavor screen's "Choose a site" button opens the Build tool with the gate's
ghost. Placement uses #207's rules, sent as `Command::BeginEndeavor { anchor }`. The site can
be moved (cancelled and re-placed) until the foundation work starts.

| # | Stage | Who | Needs (default) | Time (about) | Sprite |
|---|---|---|---|---|---|
| 1 | **Foundation** | Settlers: the usual construct job | 80 stone, 40 wood; 600 s of work | 10–15 min | `foundation` |
| 2 | **The Four Pillars** | Four pillar tasks. Each needs its materials delivered, then one bend of its element (150 chi, 60 s channel). The Avatar can do any of them. **Your own benders can raise your nation's pillar** | Air: 20 stone, 10 wood. Water: 20 stone, plus fresh water within 6 tiles of the plaza *or* a stocked well (later). Earth: 40 stone. Fire: 20 stone, 12 gold ore | 15–20 min | `pillars` (per-pillar caps) |
| 3 | **The Arch** | Settlers build (construct job), then an Avatar task: lift the beam with airbending (90 s) | 60 wood, 20 gold ore; 600 s | 10–15 min | `complete` without the emblem lit |
| 4 | **The Harmony Rite** | The Avatar meditates in the Avatar State on the plaza. Settlers with nothing urgent take a low-priority **Attend** job on plaza tiles | 10 minutes of rite with attendance ≥ half the settlers | 10–20 min | `complete`, with a glow |

**Per-nation flavour, in data:** each nation's pillar costs less and can be raised by its own
benders. The roof tiles are the nation's material (green, red, blue or gold tiles: one grid with
four legends). The stage names are shared.

**What can go wrong** (survival pressure, not combat):

- **Shortages stall a stage.** Gold ore for the Fire pillar and the arch is the classic one, and
  it is why "ore mined" is a sign.
- **The Avatar leaves** if he is unfed or has no bed (above). Every pillar task waits for him
  except your own element's.
- **The rite needs a full larder.** Attendance only counts settlers standing on the plaza, so
  the player has to stock food and water first and keep the urgent work small. If attendance
  drops below half, the rite **pauses**. If a settler **dies**, it **resets** ("The rite is
  broken by grief"). That makes the last ten minutes a test of everything the player built.
- **No failure state.** Nothing is lost for good. A stalled project waits.

## The transformation: the United City

When the rite completes, the gate's emblem lights up and the toast reads "The Harmony Gate is
complete. People of all four nations will come to settle here."

1. **Newcomers of all four nations (moment to moment).** Population growth (see below) keeps
   working, but each newcomer's nation is drawn from data weights (default: your own 40%,
   each other 20%). A newcomer is drawn with their own nation's characters and names. **They
   work with their own nation's modifiers**: an Earth newcomer quarries at earthworks 2.5 in a
   Water settlement, and an Air newcomer walks at 1.6. **They bend their own nation's signature
   ability.** The bend picker (`B`) lists every ability someone in the settlement can cast, with
   a count ("Raise Earth: 14 benders"). Daily play changes: a Fire settlement can now ford
   rivers, Air hauls get Wind Carry *and* strong Earth backs, and who you have decides which
   plans work.
2. **The research tree stays your own.** Newcomers don't bring their nation's techs: the owner
   deferred cross-nation learning (research.md, owner decision 2). Only signature abilities come
   with them, and branch abilities stay with the home nation. Teaching techs can be a later
   decision built on this.
3. **A permanent landmark.** The plaza is walkable, a natural gathering point. It also counts as
   a heat source of radius 3 (#58), so meals there are cooked and people gather. This is small
   and data-only.
4. **A new goal, light:** once the gate is complete, the Endeavor screen shows "The United City":
   the population by nation and the next population milestone. A second endeavor (the Spirit
   Portal, concept B) is the natural sequel, and its own design issue if the owner likes this one.

## Population growth (a prerequisite, #232)

There is no population growth in the sim today, so the population sign can't be met. A minimal
rule, also used by the United City: every `arrival_minutes` (default 15), if there is a **free
bed** and at least `food_per_newcomer` (default 30) food in stores, one newcomer walks in from
the nearest map edge, named from the nation's pool. That's deterministic, with names from
`rng::stream(seed, "population.names")`. Its numbers go in `data/needs.toml` `[growth]`.
Births, families and a migration screen are later, if ever.

## Fit with existing systems

- **Data:** a new `data/endeavor.toml` with `[endeavor]`, `[[sign]]`, `[[stage]]`, `[[task]]` and
  `[avatar]`, loaded strictly like `research.rs`. Unknown keys are rejected. Sign kinds are a
  closed enum: `population`, `beds_for_all`, `building_kinds`, `branch_complete`, `stored`,
  `tally`, `masters`. Ids resolve at load, and every number has a range check. The gate is an
  ordinary `[[building]]` with `footprint`, `passable`, `endeavor = true` and `art` stages.
  Abilities, items and techs are referenced by id.

  ```toml
  [[sign]]
  id = "ore"
  group = "harvests"
  name = "Ore mined"
  kind = "tally"
  tally = "mined"
  items = ["gold_ore"]
  target = 40
  hint = "Mine gold veins; tunnels yield more"

  [[stage]]
  id = "pillars"
  name = "The Four Pillars"
  [[stage.task]]
  id = "earth_pillar"
  element = "earth"                    # who may do it: the Avatar, or benders of this nation
  cost = [{ item = "stone", count = 40 }]
  chi = 150.0
  work = 60.0                          # channel seconds
  ```

- **Sim state** (all `#[serde(default)]`):
  - `World::endeavor: EndeavorState`, one of `Dormant`, `Ready { tick }`,
    `Building { anchor, stage, tasks, delivered }` or `Complete { tick }`;
  - `World::tallies` (harvested and mined by item, scrolls read, crops harvested);
  - `Unit::bends: u32` (mastery), `Unit::guest: Option<Guest>` and `Unit::nation: Option<Nation>`
    (`None` means the world's nation).

  Stage and task are stored by **string id**, so reordering the data doesn't misread a save.
  The gate joins the buildings table, which changes `Defs::id_hash` as any new building does.
- **Commands:** `BeginEndeavor { anchor }` and `CancelEndeavor` (only before the foundation
  work starts). Pillar tasks are jobs (`JobKind::EndeavorTask { task }`), queued automatically
  when their materials are in, so there's nothing for the player to designate. Bend jobs work
  as today, with one change: `BendError::WrongNation` becomes "nobody here can cast this", and
  a unit claims a bend if its own nation (or the Avatar's list) includes the ability.
- **Events:** `EndeavorEvent { tick, kind }` covers sign met, ready, stage done, Avatar arrived,
  left or returned, rite paused or broken, complete, and newcomer. They feed toasts and the log,
  like `BendEvent`.
- **Determinism:** no RNG except newcomers. The arrival tile, task claims (nearest by path,
  ties by `Pos`) and attendance are all ordered. A headless test plays a scripted settlement
  from Ready to Complete and compares the event log across two runs. Another test breaks the
  rite with a death and checks the reset.
- **Bending:** pillar tasks reuse the bend channel, chi spend, pose rows and cast effects
  (#166). The Avatar's bends are ordinary `Bend` jobs.
- **Buildings and #196:** the gate needs #207 (footprints and placement) and #208 (ring stand,
  walkable footprints). Staged sprites are a small extension of #210's drawing: the site sprite
  is picked by stage instead of faded by progress.
- **Research:** unchanged. Signs read known techs. No tech unlocks the endeavor, because the
  branch sign already needs the whole branch.
- **Frontend (TUI; the Bevy client had its own version of this until #281 removed it):** the
  `G` screen and side-panel section, and a bend picker with several nations' abilities. The TUI
  draws the Avatar as `@` in white on the accent (an Avatar State variant marks the rite), and
  the gate as #212's 4x4 glyph block, staged like a build site (`data/tui.toml`).
- **Art:** the Bevy client's sprites for Aang (a 64x96 sheet), Appa (48x32) and the gate's
  stages (a 64x80 cell per stage, four roof legends) were hand-authored pixel art; both the art
  pipeline and the sprites themselves were removed with the client in #281. The TUI needs no
  art, only the glyph and colour entries in `data/tui.toml` above.

## Follow-up issues

Filed without `ready`, and each sized for one PR. Blocked-by links are on the issues.

| # | Issue | Blocked by |
|---|---|---|
| #232 | Sim: population growth (newcomers join when there's a spare bed and food) | none |
| #233 | Sim: lifetime tallies and bending mastery (`Unit::bends`) | none |
| #234 | Sim and data: `data/endeavor.toml`, the signs, the Ready latch and events | #233 |
| #235 | Client: Endeavor screen (`G`), side-panel section and toasts | #234 |
| #236 | Sim: the Harmony Gate site and its stages (`BeginEndeavor`, pillar tasks) | #234, #207, #208 |
| #237 | Sim: the Avatar as a guest (arrival, AI, four abilities, needs, leaving) | #236 |
| #238 | Sim: the Harmony Rite, completion and farewell | #237 |
| #239 | Sim: per-unit nation (modifiers, abilities and characters per unit) | none |
| #240 | Sim: the United City (newcomers of all four nations) | #232, #238, #239 |
| #242 | TUI: Endeavor screen, side panel, gate and Avatar glyphs | #234, #237 |
| #243 | Client: the gate's stage sprites, the Avatar, the Avatar State and Appa on the map | #237, #244, #246, #210 |
| #244 | Art: Aang's character sheet (64x96, walk and bend rows) | none |
| #245 | Art: Aang's portrait (32x32, plus the Avatar State) | #244 |
| #246 | Art: the Harmony Gate (4x4, stage sprites, four roofs) | none |
| #247 | Art: Appa resting (48x32) | none |
| #248 | Client: a mixed-nation settlement (bend picker by caster, people by nation) | #240 |

## Owner decisions (2026-10-25)

Recorded on #224 (https://github.com/gizmo385/four-nations/issues/224#issuecomment-6393164172).
The owner accepted all four recommendations. The alternatives stay below as the rationale.

1. **Avatar: Aang with Appa, and no companions in v1.** Aang fits the unity story and the
   series' title. Companions would each need a sheet, a portrait and a role. *Considered:* Korra
   (a better fit for the Spirit Portal, concept B), or each nation's own Avatar (Aang, Korra,
   Kyoshi, Roku), which is four times the art for a visitor who stays an hour.
2. **Endeavor: one shared Harmony Gate, styled per nation, whose payoff is the United City**,
   with no tech teaching. Your own nation's pillar is raised by your own benders. This stretches
   vision.md's "one player nation" to "one settlement, many peoples". The research tree stays
   your nation's. *Considered:* per-nation wonders (concept C), and the Spirit Portal (B, kept
   as a possible sequel).
3. **Control: Aang is a guest with his own AI.** He claims Avatar tasks and any bend you
   designate, but you never drag him around or give him chores. *Considered:* direct control,
   which would make him a super-worker and break indirect control, and a fully scripted helper,
   which would make him scenery.
4. **Length: about 3.5 to 4 hours of play to qualify, plus about an hour for the project**, with
   the default targets in the signs table. *Considered:* shorter (2.5 h: population 12, no farm
   or ore signs), and longer (6 h or more), which needs more mid-game content than exists today.
