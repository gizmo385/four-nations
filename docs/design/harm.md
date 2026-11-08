# Harm and healing: sources of harm, so healing has something to heal

Status: **proposed** (#385). The owner decisions are listed at the end under "Open questions for
the owner", and the follow-up issues under "Follow-up issues".

Today nothing hurts a settler except neglect. Health falls only while hunger, thirst or energy
sits at 0, and it comes back by itself at 2 a minute. So Herbalism and Healing unlock nothing, and
the Healing Hut (#375) and Healing Water would only speed up recovery from near-starvation, which
the 2026-11-05 playtests never came near. The owner's Normal target is **close calls**
(materials.md, question 2): a settlement that grows carelessly loses someone, and a careful one
never does. This doc proposes three sources of harm that fit the tone, the four nations and the
"no warfare" rule, and gives every healing feature a job against them.

## Decisions in brief

1. **Three sources, each tied to something the player can see and change.**
   - **Exposure**: sleeping rough on cold land in winter.
   - **Injury**: heavy work (felling, quarrying, mining).
   - **Sickness**: drinking sea water.

   Each one costs about **40 health** on Normal, so one event leaves a settler at about 60 and two
   leave them at about 20.
2. **On Normal, harm wounds but never kills by itself.** It stops at 10 health. A settler dies
   only when harm and neglect add up. On **Hard**, harm can kill outright and does 1.5 times the
   damage. On **Sandbox** it's off. These are two new preset knobs, `harm` and `harm_kills`.
3. **Harm lingers.** A settler who is sick or cold doesn't regenerate. A settler below 80 health
   takes no heavy work. The second rule is what makes healing an economic lever and not only a
   safety net.
4. **Every healing feature gets a job.** Herbalism grows **herbs**, which heal 20 and cure
   sickness, for every nation. Water's **Healing Hut** (#375, numbers unchanged) also treats
   sickness. **Healing Water** heals a crowd at once and cures sickness. #125's healer job doesn't
   come back.
5. **Nations meet harm differently through data that already exists.** Fire's warmth answers
   exposure. Earth's `earthworks` cuts its quarrying and mining risk. Water heals. Air raises
   shields, but its weak backs get hurt most.
6. **Every harm is announced before it can kill**: a hover line, a People flag, an alert and a
   reason in the selection bar. A death says what weakened the settler.
7. **No new threats.** There are no wildlife, raids or contagion, as the owner decided for
   difficulty.md (question 1).

## Why nothing hurts today, in numbers

A throwaway probe replayed the economy opening (`autoplay::Economy`, the engine behind
`balance --economy`) on Normal for seeds 1 to 8, all four nations, over 135 minutes. It counted
what each proposed source would have seen:

| What | Seeds 1–8 (Normal, economy opening, 135 min) |
|---|---|
| Default site's climate band | **warm on every seed**, so no cold at home |
| Minutes asleep on cold land with no bed or warmth | 0 on most runs; 8 to 16 on 6 of 32 (a worker out in the hills) |
| Drinks from sea-only shore, of about 140 | 0–10 on most seeds; **seed 4: 28–55; seed 7: 83–139** |
| Heavy picks per settlement | about 50–500 fellings, 60–160 quarry picks, 9–32 vein picks |
| Lowest health seen by Water, Earth or Fire | **100**: nobody was ever hurt |

Food per head entering winter 2 was 10 to 37 for every nation except Air on seeds 1 to 8
(`balance --economy --seeds 1-8 --minutes 180`), apart from seeds 3 and 4, which #384 is
investigating. With food that plentiful, "close calls" can't come from hunger alone. It needs
harm that lingers and then meets an ordinary lean spell.

**Found along the way:** Air settlers die on seeds 1, 5, 6, 7 and 8 *with food in stores*. A unit
locks into an Eat task beside a stocked stockpile and stands there until it starves, thirst
included. That's a sim bug, not balance, and it's filed as #386. It
isn't counted against this design.

## Rules every source shares

| Rule | Value (data) | Why |
|---|---|---|
| One harm event on Normal | about 40 health | One leaves a settler hurt (60), two leave them fragile (20). Big enough to read, small enough to survive |
| Floor while the preset's `harm_kills` is off | 10 health (`harm.toml [settings].floor`) | The Normal guarantee: harm alone never kills, so a fed colony never loses anyone |
| No natural regen | while sick or cold | Harm lingers until it's treated, warmed, or waited out |
| Too hurt for heavy work | below 80 health (`fit_for_heavy`) | An injury takes a quarrier off the rock until healed. That labour is what herbs and the hut buy back |
| Damage scale | preset `harm`: 0 / 1.0 / 1.5 | One knob, the way `strain` works |
| A death names its weakener | `Death::weakened_by` | "died of hunger, weakened by frostbite", so a careless loss is explained |

The existing need damage (10 a minute per empty need) and natural regen (2 a minute while no
need is low) are unchanged. Harm stacks with them.

## The three sources

| | Exposure | Injury | Sickness |
|---|---|---|---|
| **Cause** | Asleep in winter on a cold-band tile, not heated, not in a bed (`sleep_bonus` > 1) | Finishing a harvest marked `risk`: felling, quarrying, mining | A drink from a tile whose only water is sea |
| **Seen coming** | The winter warning; the "Cold" status line; an alert when someone lies down rough | A risk line on the resource's hover and on the designation tool | The embark Water line ("sea only"); the tile hover; the drink plan prefers fresh water |
| **Avoided by** | A bed within reach (30 tiles), a campfire, Hearthflame, Inner Fire, an igloo, Earth Tent, Air Shield | Healthy workers (nobody below 80 takes the job); Earth's `earthworks` | Fresh water within 15 tiles' detour (river, lake, ford, Draw Water) |
| **Treated by** | Warmth (regen resumes); herbs, hut, Healing Water | Herbs, hut, Healing Water, rest | Herbs (cure), the hut (4x faster), Healing Water (cure) |
| **Cost, Normal** | 4 a minute asleep: about **32 a night** (8 min) | **40** per accident | 3 a minute for 12 min: **36**, no regen |
| **Cost, Hard** | 6 a minute, and Hard's colder nights last about 16 min: about **96**, which can kill | **60** | **54** |
| **Expected rate, Normal** | 0 at a warm site; on cold land, once a winter per settler who sleeps away from beds | about 0.3 (Earth) to 0.8 (Air) a season per settlement | about 0.75 a season on seed 7's shore; under 0.3 elsewhere |

### Exposure: sleeping rough in the cold

difficulty.md made cold drain needs only ("cold never damages health directly"). This proposal
keeps that for anyone awake. Cold now hurts only **a sleeper in the open**, which is a discrete,
visible event the player can fix. The test is the one `tick_needs` already runs for cold sleep,
plus "not in a bed":

```
exposed = asleep && World::is_cold(pos) && bed_sleep_bonus(pos) <= 1.0
```

A bed (1.4) or an igloo (1.6) counts as shelter. Lying within any heat source's radius rules cold
out already, and that includes #362's Earth Tent and Air Shield. So the answer to exposure is
exactly the kit difficulty.md and bending.md built for winter. That kit had no stakes until now.

- **Rate.** A rough sleeper on cold land regains energy at 15 × 0.5 = 7.5 a minute, so the sleep
  from 30 to `sleep_until` (90) takes 8 minutes. At 4 a minute that's 32 health on Normal. On
  Hard, `winter_severity` 1.5 halves the sleep rate again (to 3.75), so the night runs about
  16 minutes at 6 a minute. That kills unless someone acts. The player has those 16 minutes,
  because the alert fires when the settler lies down: a campfire takes 15 s of work, and Earth
  Tent or Hearthflame can be cast on the spot.
- **No regen while cold**, awake or asleep, so a winter of field work in the hills wears people
  down slowly, but only on cold land.
- **Where it bites.** Every default site in the probe is warm, so exposure is a **choice of
  site**: settle a polar or cool site (the Water Tribes' canon home, and +1 or +2 on the embark
  outlook already) and winter shelter matters. Elsewhere it reaches only workers who range into
  cold hills.

### Injury: heavy work

`data/jobs.toml` gains an optional `risk` on a harvest: the chance that finishing one pick
injures the worker.

| Harvest | `risk` | Air (earthworks 0.7) | Water / Fire | Earth (2.5) |
|---|---|---|---|---|
| Oak, fir (felling, no modifier) | 0.01 | 1% | 1% | 1% |
| Stone outcrop, boulder (quarrying) | 0.02 | 2.9% | 2.2% / 2% | 0.8% |
| Gold vein, gold seam (mining) | 0.03 | 4.3% | 3.3% / 3% | 1.2% |

- **Chance = `risk` ÷ the harvest's own `modifier`** (earthworks for stone and ore). This is
  pillar 1 through numbers that already exist: Earth quarries most and most safely, and Air's
  weak backs get hurt most. There's no `if nation`.
- **Draw.** One roll per finished risky pick, from a new stream
  `rng::stream(seed, "harm.injury")` kept on the world as `World::rng` is. It's drawn only while
  `harm > 0`, so `World::difficulty == None` worlds, and every golden, draw nothing.
- **Effect.** 40 × `harm` damage, then the 80 threshold keeps the worker off heavy work. Natural
  regen takes them from 60 back to 80 in 10 minutes. Herbs do it at once. The hut does it in
  2.5 minutes (1.25 for Water). A settler can't take a second injury while below 80, so on Normal
  an injury alone never takes anyone below 40.
- **Rate.** In the probe's heavy runs (seed 1: about 140 fellings, 155 quarry picks and 31 vein
  picks over 9 seasons), a settlement takes about 3 injuries (Earth) to 7 (Air). That's one every
  season or two: an occasional event, never a drumbeat. A player who quarries less sees fewer.
- **Recipes are safe.** Earth's Mine Shaft and Deep Mine are braced workings, and crafting carries
  no risk in v1.

### Sickness: sea water

The sea is salt. A drink at a shore tile whose only water neighbours are `Terrain::Water` (sea)
carries a chance of sickness. Rivers, lakes and fords are fresh.

- **The drink plan prefers fresh water.** `plan_drink` takes the nearest fresh-water drinking
  spot if its path is within `fresh_detour` (15) tiles of the nearest spot of any kind, and
  otherwise drinks where it stands. Air's speed makes the detour cheapest.
- **Chance.** 5% a sea drink. On seed 7's shore, where about 90% of drinks were sea, that's about
  7 sicknesses in 135 minutes for 15 people, roughly one a season. On most other seeds it's under
  one a year.
- **Effect.** It lasts 12 minutes at 3 × `harm` a minute (36 on Normal), with no regen. It
  doesn't spread, and it doesn't stop work: a sick settler is just a settler losing health with
  a timer on it.
- **Answers.** Settle by fresh water (the embark report says so), Water's Draw Water (a ford is
  fresh), or herbs. A shared well was considered and left out: herbs already answer it for
  everyone, and a well anywhere would erase distance-to-water from the map.

## Healing: a job for each feature

| Feature | Who | Today | Proposed job |
|---|---|---|---|
| Natural regen | everyone | 2 a minute | Unchanged, but paused while sick or cold. The slow baseline |
| **Herbs** | `herbalism` (shared, 120) | nothing | A hurt (below 60) or sick settler with herbs in stores takes one, like a meal: **+20 health and cures sickness** |
| **Herb Garden** | `herbalism` | — | A farm plot that grows `herbs` (2 per 300 s, about 6 a season, plenty for one settlement). Data only, through `[building.farm]` |
| **Healing Hut** | `healing` (Water, 120) | #375: 8 a minute × `water_work` below 60, to 90 | **Unchanged numbers.** It gains one rule: a sick settler also goes in, and sickness runs out 4 times faster inside. Healing inside ignores the cold-regen pause |
| **Healing Water** | `healing` (Water) | designed in #125's comment | As designed (+30 × `water_work` to every hurt unit within 2 tiles, 40 chi), plus **cures sickness**. It's valid while someone in range is hurt or sick |
| Healer job (#125) | — | proposed | **Not revived.** The hut works unstaffed, and Water's benders are its healers through Healing Water |

So **#375 lands as it's being built**. The hut's 60/90 band lines up with the 80 heavy-work line:
an injured quarrier (60) walks in and comes out (90) fit for the rock. The sickness rule and the
cold exception are a small follow-up on top (#391), not a change to its acceptance criteria.

**Herbalism is the every-nation answer, and Healing is Water's edge.** Herbs fix one settler at a
time from a garden someone has to sow. Water's hut heals two at 16 a minute with nobody tending
it, and Healing Water fixes a whole crowd after a bad night. That's the Water Tribes' pillar
("healing", vision.md).

## By nation

| Nation | Exposure | Injury | Sickness | Healing |
|---|---|---|---|---|
| Fire | Best: campfire radius × heating 2.5, Hearthflame, Inner Fire on the move | Average | Average | Herbs |
| Earth | Earth Tent at a far quarry (30 min) | **Safest** (÷2.5) | Average | Herbs |
| Water | Igloo (1.6); polar sites are home | Average | **Draw Water** makes fresh fords | **Herbs, hut, Healing Water** |
| Air | Air Shield over a crew (no sleep bonus) | **Most hurt** (÷0.7) | Fast to reach fresh water | Herbs |

## Difficulty

| Knob (`data/difficulty.toml`) | Sandbox | **Normal** | Hard |
|---|---|---|---|
| `harm` (damage scale; 0 means no harm at all, no draws) | 0 | 1.0 | 1.5 |
| `harm_kills` (harm may take health below the floor, to 0) | no | no | **yes** |

Sandbox has no seasons, so it has no exposure either. With `harm = 0` there's no injury roll,
sickness or herb-seeking, and the world plays exactly as today. Hard's extra danger also arrives
through `winter_severity`, which lengthens a cold night, and nothing else.

## Is a careful Normal colony safe?

**Yes, by construction.** With `harm_kills` off, harm stops at 10 health, so a death needs an
empty need. In the economy opening with farms, no need empties for Water, Earth or Fire on seeds
1, 2 and 5 to 8 (food per head entering winter 2 is 10 to 37). Their lowest health over
135 minutes was 100. Harm adds hurt settlers, not deaths, and a colony that keeps eating loses
nobody. The two exceptions are both outside this design: seeds 3 and 4 (#384) and Air's Eat
freeze (#386).

**The worst case for a careful colony** is an injured settler (60) who falls sick (24, no regen
for 12 minutes). They still eat at hunger 40, and hunger takes 10 minutes more to reach 0, so
they die only if they get no food for that long. A careful colony never goes that long.

**Close calls for a careless one.** A careless colony doesn't heal, quarries with whoever is
free, sleeps its far workers rough on cold land, and drinks sea water. It spends long stretches
with settlers at 20 to 60. An empty need then kills in 2 to 6 minutes instead of 10, so an
ordinary lean spell in winter costs someone. **Honest caveat:** today's food glut means even
`--no-farms` runs barely starve (0 or 1 deaths on seeds 1, 2 and 5 to 8, the Air one again the
Eat bug). Careless deaths need #353's leaner food as well as this design. #394 checks both
targets after both land. The numbers live in `data/harness.toml` (`[[target]]`, #537) and
`balance verdict` judges them on a 32-seed panel, so this table names the targets instead of
repeating them. "Careful" is the `steady` play style and "careless" is `careless` (no farms, no
healing).

| Play style and preset, 180 min | Target |
|---|---|
| Careful, Normal, herbs researched | `no_deaths_by_180`; harm events above 0 and a lowest health below 50 are allowed, not gated |
| Careless, Normal | `careless_deaths_by_180` (read `weakened_by` from the autopsies), and `careless_close_calls` |
| Careful, Hard | `hard_deaths_by_180` |
| Sandbox | no harm events at all (not a harness target yet: a run record carries no harm tally) |

## Readable before it kills

The agent playtests' lesson (#337, #377) is that anything unexplained reads as a bug. Each source
shows before, during and after.

| When | Exposure | Injury | Sickness |
|---|---|---|---|
| Before | On cold sites, the winter warning adds "Sleeping rough on cold land causes frostbite: keep beds or a fire near work". The cold status line reads "Cold: no healing; frostbite if it sleeps here" | Resource hover and the harvest tool: "Risk: 2% injury a pick" | Embark Water line "sea 2 steps · fresh 31"; an outlook negative "only sea water near"; tile hover "Sea water: may cause sickness" |
| During | People flag **Freezing**. Alert (once per sleep): "Kya is sleeping rough in the cold at (40, 12)" | Alert: "Tyro was hurt quarrying at (88, 40)"; #337 idle reason "Too hurt for heavy work (58/80)" | People flag **Sick**. Alert: "Kya fell sick from sea water" |
| After | "Kya died of hunger, weakened by frostbite"; on Hard, "Kya froze to death at (40, 12)" | "…weakened by an injury"; on Hard, "Tyro died of injuries" | "…weakened by sickness"; on Hard, "Kya died of sickness" |

Harm alerts get their own kind (`Kind::Harm`), and fn_agent's `end_turn` stops on them as it
does on deaths, so the agent playtests notice them. The People row already has "Hurt" and
"Dying" (#310). It gains **Sick** and **Freezing**. The selection bar's left column names the
condition, its cause and its clock:

```
+----- * EARTH KINGDOM * ------+
| > Running x1           46:40 |
+- PEOPLE ----------------- 6 -+
| Quarry 2  Haul 1  Idle 3 [N] |
| Hurt 2  Sick 1               |
+- WINTER · Y1 --- 13:20 left -+
| Food in stores   38 · 6/head |
| Winter here: mild            |
+- ALERTS --------------- [L] -+
| Tyro was hurt quarrying at ~ |
| Kya fell sick from sea water |
| Winter. Wild plants rest,~   |
+- CURSOR ---------- (88, 40) -+
| Stone                        |
| Quarry: 5 stone, 12 s        |
| Risk: 0.8% injury a pick     |
| H to harvest                 |
+------------------------------+
+- Tyro - Earth Kingdom -----------------------------------------------+
| Injured quarrying: no heavy work < 80        Health ######....  58%  |
| Healing 2/min; herbs in stores: 4            Hunger ######....  63%  |
| At (83, 71)                                  Thirst #######...  70%  |
| Bends 0 of 25 to master                      Energy ###.......  32%  |
| [Esc] close  [Tab] next  [F] follow          Chi    ########## 100%  |
+----------------------------------------------------------------------+
```

*(Built from `fn_tui 7 earth --difficulty normal --frame --ascii --ticks 28000 --keys '<tab>'
--size 104x30`, cropped to the side panel and selection bar, with the People flags, alerts,
cursor and selection rows edited to show the proposal.)*

## Data and sim changes in one place

```toml
# data/harm.toml (new; read by Defs like seasons.toml, strict keys)
[settings]
floor = 10.0           # with the preset's harm_kills off, harm never takes health below this
fit_for_heavy = 80.0   # below this, a unit claims no harvest with a `risk`

[exposure]
damage_per_minute = 4.0   # asleep, cold, not in a bed with sleep_bonus > 1; times preset harm

[injury]
damage = 40.0             # per accident, times preset harm; chances are `risk` in jobs.toml

[sickness]
minutes = 12.0
damage_per_minute = 3.0   # times preset harm; no natural regen while sick
fresh_detour = 15         # path tiles a thirsty unit walks out of its way for fresh water
[[sickness.water]]
terrain = "water"         # the sea
chance = 0.05             # per drink

[[remedy]]
item = "herbs"
health = 20.0
cures_sickness = true
seek_below = 60.0         # a hurt unit below this (or any sick unit) takes one when no need is low
```

- `data/difficulty.toml`: `harm` and `harm_kills` on every preset, validated like `strain` and
  `strain_kills`.
- `data/jobs.toml`: an optional `risk` in `[0, 1)` on a harvest, plus a `herbs` item.
- `data/buildings.toml`: `herb_garden` appended (`[building.farm]` with `crop = "herbs"`), and
  #375's `[building.healing]` gains `sickness_rate = 4.0`.
- `data/research.toml`: `herbalism` unlocks `herb_garden`, and `healing` keeps #375's hut and
  adds the `healing_water` ability.
- **State**, all `serde(default)` with no `SAVE_VERSION` bump: `Unit::sick_until: Option<u64>`,
  `Unit::last_harm: Option<(HarmKind, u64)>`, `Death::weakened_by: Option<HarmKind>`, new
  `DeathCause` variants `Exposure`, `Injury` and `Sickness` (reachable only with `harm_kills`),
  `World::harm_events` for alerts (like `season_events`), and the `harm.injury` stream.
- **Determinism.** The rolls happen in unit-id order on the tick a pick or drink completes, from
  their own streams (`harm.injury`, `harm.sickness`). A world with no difficulty, or with
  `harm = 0`, draws nothing, so no golden changes.

## Not proposed

| Idea | Why not |
|---|---|
| Wildlife, raids, angry spirits | The owner's difficulty.md decision 1: no hostile threats. The spirit stays an unfiled idea |
| Spoiled food | difficulty.md rejected spoilage as invisible loss, and raw food must always be safe (needs.toml's owner rule) |
| Contagion | Hard to read and hard to avoid without quarantine tools. One-person sickness is enough |
| Construction accidents | Building is the game's verb. Risk belongs to the heavy harvests, where Earth's lead lives |
| Blizzards, Hard-only weather | Floods (#199) are the planned weather event. Hard already gets longer cold nights through `winter_severity` |
| A well | See Sickness: herbs answer it for everyone, and a well would remove water distance from the map |

## Fit with other work

- **#375 Healing Hut**: lands unchanged. #391 adds the sickness rule and the cold exception.
- **#125**: closes once #375, #391 and #392 (Healing Water) land. The healer job is
  dropped.
- **#362 shelters**: Earth Tent and Air Shield become exposure's answers with no change.
  Bending.md's "Earth sleeps warm at a far quarry in winter" now has stakes.
- **#384 (seeds 3 and 4)**: referenced, not duplicated. The harness issue (#394) waits on it.
- **#353 food retune**: careless deaths on Normal need its leaner food as well as this design.
- **#337 idle reasons**: one new reason, "Too hurt for heavy work".
- **#199 floods**: if they ever reach units, drowning would be a fourth source under the same
  rules. That isn't filed.
- **difficulty.md**: its "cold never damages health directly" becomes "cold never damages anyone
  awake" (a note points here).

## Follow-up issues

Filed without `ready`. The ones that wait on an owner answer are labelled `needs-owner`.

| # | Issue | Blocked by | Owner question |
|---|---|---|---|
| #387 | Sim and data: harm core (`data/harm.toml`, `harm`/`harm_kills`, floor, regen pause, `weakened_by`) and exposure | none | 1, 3 |
| #388 | Sim and data: injury from heavy harvests (`risk`, `fit_for_heavy`, idle reason) | #387 | 2 |
| #389 | Sim and data: sickness from sea water, and drinkers prefer fresh water | #387 | 4 |
| #390 | Sim and data: Herbalism grows herbs (Herb Garden; taking herbs heals and cures) | #387, #389 | |
| #391 | Sim and data: Healing Hut treats sickness and heals in the cold | #375, #389 | |
| #392 | Sim and data: Healing Water (the `heal` effect; cures sickness) | #387, #389 | |
| #393 | TUI: harm made readable (flags, alerts, hover risk, selection-bar condition, embark fresh water, agent stops) | #387, #388, #389 | |
| #394 | Balance: harness reports harm, and careful and careless targets on Normal and Hard | #387, #388, #389, #390, #384, #386 | |
| #386 | Bug: settlers lock into an Eat task beside stocked food until they starve (Air, economy opening) | none | |

## Open questions for the owner

1. **May cold hurt health directly?** difficulty.md chose "cold kills only through needs".
   *Recommended: yes, but only for a settler asleep in the open on cold land in winter*, with an
   alert when they lie down. It gives #362's shelters and Fire's warmth stakes and makes a polar
   site a real choice. The alternative keeps cold as needs-only and leaves healing idle on cold
   sites.
2. **Should injuries be chance-based?** *Recommended: yes, a small chance per heavy pick (1–3%,
   divided by the work's modifier), shown on hover*. They never kill on Normal, and they cost
   labour until healed. The alternative is injuries only when working tired or cold, which is
   fully avoidable but so rare that the hut and herbs would sit idle in warm sites.
3. **On Hard, may harm kill outright?** *Recommended: yes (`harm_kills`).* Hard is "real risk",
   and every source still announces itself first (a rough night on Hard gives about 16 minutes to
   react). The alternative keeps Normal's floor and only scales damage, which makes Hard a little
   slower, not more dangerous.
4. **Sea water sickness: in, and at what rate?** *Recommended: in, 5% a sea drink, with drinkers
   walking up to 15 tiles further for fresh water.* It gives Herbalism its natural job and Water's
   Draw Water a second one. It does touch seed 7's shore start, the owner's usual test map, at
   about one sickness a season. The alternative drops sickness, and then herbs only heal.
