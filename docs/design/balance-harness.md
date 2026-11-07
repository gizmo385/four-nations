# Balance harness: a verdict, not a sweep

Status: **accepted** (#489). The owner took every recommendation on 2026-11-07 as a starting
default, to retune once the harness has run for a while: see "Owner decisions". The follow-up work
is under "Follow-up issues".

The owner (2026-11-06): judging balance with `balance --economy --seeds 1-8`, one scripted
opening per nation checked against median bands, is blunt. This doc proposes a harness whose
output is a **verdict**: for each design target, did it pass, on which sites, and for every miss,
whose fault it is (the numbers, the map, the scripted player, or a sim bug). It changes no game
numbers.

## Decisions in brief

1. **A 32-seed panel with a site profile.** Every run starts on one of seeds 1-32 (1-8 stay a
   subset, so history is comparable). Each seed gets a profile computed from the supply survey,
   so "hard site" is a measured fact per sign, not an exception written into a doc.
2. **Play styles are data, and one is the reference.** `steady` (the economy opening with a bed
   cap and healing) is what targets are judged against. `careless` and `branch_first` exist only
   for the targets that need them (harm's close calls, pacing's research order).
3. **Every run gets a health check.** Detectors watch for the scripted player's own failures
   (idle with work in reach, frozen stores, no research set) and for sim locks (a settler
   stranded or stuck in a task). A flagged run is evidence about the harness or the sim, never
   about balance.
4. **Every miss is classified.** Each target miss lands in one of five buckets: sim lock, harness
   gap, hard site, outlier (unexplained, needs a look) or balance. Only balance misses and
   outliers count against a target.
5. **Targets live in `data/harness.toml`,** each with the doc section it comes from, and pass on a
   share of the valid runs on sites where they're feasible. A change is also compared run by run
   against a committed baseline.
6. **A shared care measure.** Deaths, close calls and minutes with a need at zero, kept as tallies
   on `World` so this harness and #517's agent score read the same numbers.
7. **Three cost tiers.** CI keeps today's cheap determinism checks. A quick tier (12 seeds, about
   2 minutes) runs on every balance change. The full tier (about 15 minutes) runs before a design
   doc's numbers are accepted.

## What the sweep gets wrong, in numbers

On 2026-11-07 I ran the economy opening on seeds 1-32 instead of 1-8 (all four nations, Normal,
180 minutes, `main` at a6ccbbe), next to the supply survey at radius 40. A 180-minute run takes
about 6 seconds in release on the owner's laptop, so the 128 runs took about 3.5 minutes on four
cores.

- **Most misses are the map.** 15 of 32 default sites have less than the ore sign's 40 ore within
  40 tiles. On the other 17, **Earth meets the ore sign by minute 120 on 17 of 17**, and Fire and
  Air on 16. Seen through the 1-8 sweep, Earth's "6 of 8" looks like a balance miss, but it's two
  poor sites.
- **The rest are a handful of specific runs.** Every ore miss on an ore-rich site is one of three
  cases: seed 18 (Fire, Water and Air miss it with 57 ore in reach; Earth meets it at minute 67),
  Water on seed 7 (#397) and Water on seed 30. Each one is a question about the player or the sim,
  not about the ore numbers.
- **More seeds find new failures.** On seed 19, Earth, Fire and Water keep 0 stone and frozen
  stores for 165 minutes with 418 stone in reach, and Earth never grows past 6. Water on seed 30
  loses a settler at minute 52. The 1-8 sweep can't see either. Both are filed below.
- **A count out of 8 hides who failed and why.** Water misses the stone sign on six stone-rich
  sites: seeds 3 and 4 are #512 (idle at 80% and up, likely #490's stranding), and 18, 19, 27 and
  30 were unknown until today.

Conclusion: the number to watch is "passes on sites where the target is feasible, with the
player and the sim behaving", and the list of runs that didn't fit is as useful as the score.

## 1. Sites: a panel and a profile

**The panel** is seeds 1-32, Small, the embark's `Survey::default_site`. A fixed panel means a
before/after comparison is paired: the same site, run twice. A data change still perturbs every
run chaotically (one item more here changes who walks where for the rest of the game), so a
single seed's delta means little, but 32 deltas side by side do. A **holdout** panel (seeds
1001-1032) runs on demand, so tuning can't quietly overfit seeds 1-32.

**The site profile** is computed per (seed, nation) from the supply survey, which is cheap (all
32 seeds in under half a second):

| Field | Source | Used for |
|---|---|---|
| `stone`, `ore` within 40 tiles | `balance --supply` (with the nation's tunnelling reach, `--bore-limit`) | stores signs' feasibility |
| `fresh_water_steps`, `food_sources` within 15 | `Survey` (embark outlook) | food and thirst targets |
| `open_tiles` within 8 of the anchor | flood from the anchor | "cramped": stranding risk (#490) |

A site is **hard for a sign** when its supply is below the sign's need (`ore` < 40, `stone` < the
stone sign). The cut is a data field per target (owner question 1). The profile is per nation
because reach is: Earth's tunnelling and mine shafts get ore that a surface survey doesn't count,
which is why Earth meets ore on 7 of the 15 ore-poor sites.

Hard-site runs aren't thrown away. They're judged against an optional softer expectation (for
example #440's "ore by minute 150 with caravans") and reported as their own share: "ore is hard
on 15 of 32 default sites" is a design fact the owner should see (question 4).

## 2. Players: styles in data

One scripted player standing in for everyone means its gaps read as balance. The fix isn't many
players judged equally. It's **one reference player that behaves the way a steady person does**,
plus a few variants that exist to test one specific target each.

```toml
# data/harness.toml (read by fn_harness, never by fn_sim)
[[style]]
id = "steady"                 # the reference: every target uses it unless it names another
opening = "economy"           # fn_sim::autoplay::Economy
farms = true
healing = true                # researches herbalism, one Herb Garden per 8 people (#394)
beds = "to_target"            # stop at the population sign plus `bed_margin` (#513)
bed_margin = 2
research = "shared_first"

[[style]]
id = "careless"               # harm.md's careless colony: no farms, no healing, beds unlimited
opening = "economy"
farms = false
healing = false
beds = "ahead"

[[style]]
id = "branch_first"           # pacing.md's "a player who goes branch first"
inherits = "steady"
research = "branch_first"

[[style]]
id = "idle"                   # difficulty.md's no-input row
opening = "none"
```

Each field maps to a builder already on `Economy` (`without_farms`, `with_branch_first`) or one
that #394 and #513 add (healing, the bed cap). No `if nation == ..`: a style is the same for every
nation.

**Scripted agents (`fn_agent play --script`, #483) are a later style kind,** `opening = "script"`
with a library path. They're the best stand-in for "a different player" once #518 produces a
governor script worth running, and they cost nothing per run but CPU. They stay informational
(shown, not gating) until one has a track record (question 2).

## 3. Run health: telling a harness gap from a sim lock

A **probe** samples the world once per in-game minute during every run (cheap: a few counts per
unit) and raises flags. Thresholds are in `data/harness.toml` `[detectors]`.

| Flag | Fires when | Class |
|---|---|---|
| `stranded` | a settler's idle reason is `Unreachable`, or it has no path to any water or stockpile, for 3 minutes running | sim lock |
| `stuck_need` | a settler holds the same need task for 3 minutes while that need doesn't rise (#386's signature) | sim lock |
| `idle_with_work` | idle share is 60% or more for 30 minutes while an unmet stores sign's material is in reach | harness gap |
| `stores_frozen` | stores don't change for 30 minutes while a stores sign is unmet and its material is in reach | harness gap |
| `research_idle` | no research is set for 10 minutes while a tech is available | harness gap |
| `chi_idle` | a bender sits at full chi with no bend designated for half the time after its first bend is known | harness gap |
| `refused` | the player's commands are refused 20 times in a row | harness gap |
| `overgrow` | population is 25% over the population sign while the food sign is unmet | harness gap |

Every **death gets an autopsy** in the run record: cause and `weakened_by`, minutes since the
settler last ate and drank, its idle reasons over its last 10 minutes, food within 15 tiles at the
moment of death, and whether it could reach a stockpile and water. #487 took an implementer over
an hour with throwaway trace programs. With this it's one line: "Starvation, Unreachable for 14
minutes before death, 52 food within 15 tiles, no path to a stockpile", which is a sim lock.

**`balance trace`** reproduces one flagged run and prints its per-minute probe rows over a window,
plus an ASCII crop around the anchor at a chosen minute (the `mapgen` renderer over the live map,
with units, buildings and blocking resources marked). The verdict prints the exact command for
every flagged run and every outlier.

## 4. The care measure (shared with #517)

#517 scores an agent run partly on "how well it cared for its people". This harness needs the same
thing to tell a careful colony from a careless one (harm.md, #394, #486). One definition serves
both, and it lives in the sim so a replayed log (#517's `fn_agent verify`) and a harness run
compute identical numbers.

`World::care()` returns tallies that only go up, updated every tick, beside `Tallies`:

| Tally | Meaning |
|---|---|
| `person_minutes` | minutes lived, summed over settlers (the denominator) |
| `depleted_minutes[need]` | person-minutes with hunger, thirst or energy at 0 (taking damage) |
| `severe_minutes[flag]` | person-minutes per severe flag: Starving, Parched, Exhausted (below half the need's `low`) and Dying, the People row's Bad statuses. A settler showing two flags counts in both, so this is a breakdown, not a total |
| `severe_any_minutes` | person-minutes with at least one severe flag; #517's "suffering" share is this ÷ `person_minutes` |
| `cold_minutes` | person-minutes sleeping rough in the cold (exposure applying) |
| `close_calls` | episodes of a settler entering Dying (health below `close_call_health`, 25) who lives; re-arms once back above `rearm_health` (50). The Avatar leaving unfed (`AvatarLeft`) also counts once |
| `lowest_health` | the lowest health any settler reached |

The close-call thresholds and the severe flags follow the owner's #520 answer, so #517's score
and this harness count the same episodes. Deaths (`World::deaths`, with `weakened_by`) and harm
events already exist and aren't duplicated.
The thresholds go in `data/harm.toml` `[care]`. Consumers derive rates: **depleted minutes per
person-hour** and **close calls per 10 person-hours**. Rates per person-hour are what keep a tiny,
safe colony from scoring well on care alone; whether care trades against progress is #517's
call, not this doc's.

## 5. Targets and the verdict

Targets move out of the design docs' prose tables into data. Each names the doc section it comes
from, so a doc can say "see `stone_by_120`" and the number lives in one place.

```toml
[[target]]
id = "stone_by_120"
doc = "materials.md#6-sign-targets-272"
style = "steady"
preset = "normal"
metric = "sign_minute.stone"      # or "pop_at.120", "deaths", "care.close_calls_per_10ph", ...
at_most = 120.0
share = 0.85                      # of valid runs on feasible sites (owner question 1)
hard_when = { supply = "stone", below = 1.0 }   # x the sign's need
hard_expect = { at_most = 180.0, share = 0.5 }  # optional softer bar on hard sites

[[target]]
id = "careless_close_calls"
doc = "harm.md#is-a-careful-normal-colony-safe"
style = "careless"
metric = "care.close_calls"
at_least = 1
share = 0.5
```

**How a run counts.** For each target and nation, every run lands in exactly one bucket, checked
in this order:

1. **Sim lock**: a sim-lock flag fired before the target's deadline.
2. **Harness gap**: a player-health flag fired before it.
3. **Hard site**: the profile says the target is infeasible here. Judged against `hard_expect`.
4. **Outlier**: a miss where the site is feasible, no flag fired, and at least two other nations
   met it on the same seed. It counts as a miss (the harness doesn't hide what it can't explain)
   and prints a `balance trace` command.
5. **Balance**: everything else, pass or miss.

A target **passes** when passes / (balance + outlier runs) is at least `share`, **fails** below
it, and reports **no data** under `min_runs` (8). Flagged runs never count either way, but their
number is printed, so a harness that flags half its runs can't pass quietly.

**Sensitivity.** Pairs of targets that must separate (careful vs careless care, Normal vs Hard
deaths) are checked as pairs: if both styles produce the same care numbers, the verdict says the
check can't fail (#486's problem).

### What it prints

Illustrative: the pass counts below are computed from the real 2026-11-07 run on seeds 1-32,
with the flags assigned by hand where an open issue already explains the run (the detectors
don't exist yet). The care and baseline lines are mocked. Water's stone target fails: one
balance miss (seed 27, where only Earth meets it) and two outliers to look at.

```
balance verdict  steady  normal  180 min  panel 1-32  main a6ccbbe  vs baseline 3127de9

target              share  earth       fire        water       air         doc
stone_by_120        85%    20/20 pass  17/18 pass  14/17 FAIL  19/21 pass  materials #6
ore_by_120          85%    16/16 pass  14/15 pass  12/14 pass  16/17 pass  materials #6
no_deaths_by_180    95%    31/31 pass  29/29 pass  27/28 pass  32/32 pass  difficulty
  hard: ore by 180  50%     8/15 pass   5/14 fail   4/14 fail   6/15 fail  #440
careless_calls      50%    no data: careless isn't run in the quick tier       harm

hard sites   ore below need on 15 of 32: 2 4 6 10 11 13 16 20 21 22 25 26 28 29 31
             stone below need on 11 of 32 (met by 120 anyway on 38 of 44 runs)
sim lock     water 3      stranded from 104, first death 140      balance trace water 3
harness gap  fire 3 4     idle_with_work from 45                  #512
             water 4      idle_with_work from 45                  #512
             water 7      research_idle from 51                   #397
             earth fire water 19   stores_frozen from 15          balance trace earth 19
outlier      water 18     stone: earth, fire, air met it          balance trace water 18
             water 30     stone, ore: the other three met both    balance trace water 30
balance miss fire water air 18 ore   57 in reach, only earth met it
             fire water air 27 stone 278 in reach, only earth met it
care         depleted min/ph  0.0  0.0  1.9  0.0   close calls/10ph  0.0  0.1  0.6  0.0
vs baseline  stone sign minute: earlier on 21, later on 6, same on 5; median -4.0 (moved)
             ore sign minute: earlier on 9, later on 8, same on 15; median 0.0
verdict      2 pass, 1 fails (water stone), 1 no data; ore's hard bar fails 3 of 4; 2 outliers
```

## 6. Regression: paired against a baseline

Every verdict run writes its run records (one JSON object per style, nation and seed: metrics,
profile, flags, care, autopsies) to `target/balance/LABEL.json`. The **accepted baseline** is
committed as `crates/harness/baseline.json` and re-blessed with `--bless`, the same way the map
goldens are. A balance PR's commit says it re-blessed and pastes the verdict.

`--against FILE` compares two record sets seed by seed: for each metric, how many runs moved
earlier, later or not at all, and the median delta. A metric is reported as **moved** when the
median delta is past the metric's `noise` (data) and at least twice as many runs moved one way as
the other. That's a sign test, simple enough to read and with no dependencies. The `noise` value
comes from a one-off **jitter run**: every panel seed re-run with the opening's step offset by one
tick. That changes nothing about balance, so whatever moves is the chaos floor.

## 7. Cost tiers

Measured: 6 seconds per 180-minute run in release, single-threaded, on the owner's 8-core laptop.
Runs are independent, so the runner uses four threads (the laptop's performance cores).

| Tier | What | Time | When |
|---|---|---|---|
| CI | `crates/sim/tests/balance.rs` as today, plus detector and verdict unit tests on hand-built worlds | under 10 s, no sweep | every `cargo xtask ci --fast` |
| quick | seeds 1-12, all nations, `steady`, Normal, compared to the baseline | about 2 min | every change to `data/*.toml` balance numbers, autoplay or the sim's economy |
| full | seeds 1-32 x all nations x `steady`, `careless`, `branch_first` on Normal; `steady` and `idle` on Hard and Sandbox at 12 seeds | about 15 min | before accepting a design doc's numbers, and in every balance tuning issue |
| holdout | the full tier on seeds 1001-1032 | about 15 min | on demand, when a tuning pass has iterated on the panel |

`cargo xtask balance [--full | --holdout | --bless]` builds release and runs the tier. The budget
is owner question 3.

## 8. Code shape

- **`fn_sim::balance` stays pure.** It gains one entry point, `run(&RunSpec, &Defs) -> RunRecord`,
  where `RunSpec` holds the seed, nation, preset, minutes, style (as plain fields, not TOML) and
  detector thresholds. The record holds the site profile, per-season rows, sign minutes, the
  decision timeline, flags, care tallies and autopsies. Today's `run` and `run_economy` become thin
  wrappers so `tests/balance.rs` keeps passing. The probe and detectors live here because they read
  `World` every minute.
- **A new crate, `crates/harness` (`fn_harness`),** owns the `balance` binary (moved out of
  `fn_sim/src/bin`), reads `data/harness.toml`, runs the panel on threads, sorts records by
  (style, nation, seed) so output never depends on thread order, writes JSON and prints the
  verdict. It's a separate crate because it does file I/O and threads, which `fn_sim` must not.
  The old modes stay as subcommands: `supply`, `mastery`, `economy` (the per-season table) and
  `trace`.
- **Scenario starts (#481)** aren't used by the panel, which always starts at embark. A later
  target can name a scenario to test the gate project without paying for the first 60 minutes.

## How the open symptoms would be classified

| Issue | Symptom | Bucket | What shows it |
|---|---|---|---|
| #513 | food sign rarely met, population 21-24 | harness gap | `overgrow`; the `steady` style's bed cap is the fix |
| #512 | Fire/Water at 40 stone and 0 wood on seeds 3, 4 | sim lock if `stranded` fires (#490), else harness gap | profile says stone-rich (724, 208); Earth meets it on the same seeds; `idle_with_work` from minute 45 |
| #443 | Earth and Air rarely bend, no 3rd master | harness gap | `chi_idle`. If no bend target exists in reach at all, it's a content gap and gets a design issue |
| #397 | Water on seed 7: no branch, no ore | harness gap (likely) | `research_idle`; also an outlier, since the other three mine 54-57 with 72 ore in reach |
| #394 | careful never dies, careless has close calls | balance | it's the consumer: the care measure and the `careless` style are what it needs |
| #487 | Water starves on seed 3 with farms | sim lock | `stranded`, and the autopsy: Unreachable before death, food within 15 tiles |
| #486 | `--no-farms` never loses anyone | balance (structural) | `careless` runs are valid on feasible sites, care shows no close calls, and the sensitivity pair fails |
| #396 | ore short on seeds 2 and 6 | hard site | profile: 5 and 17 ore in reach, below 40 |
| #384 | seeds 3, 4 collapse (fixed) | sim lock | `stranded`: dehydration in a pocket a study hall sealed |
| #386 | Air locks in Eat beside food (fixed) | sim lock | `stuck_need` |

New from the 32-seed run: seed 19's frozen stores would print as a harness gap (`stores_frozen`)
until its cause is known, seed 18's ore and seed 27's stone (only Earth meets either) as balance
misses on feasible sites, and Water on seeds 18 and 30 as outliers. They're filed as #542.

## Follow-up issues

Filed without `ready`. The issues that wait on an owner answer are blocked by its `decision`.

| # | Issue | Blocked by |
|---|---|---|
| #533 | Sim: care tallies on `World` (shared with #517) | |
| #534 | Harness: `RunRecord`, site profile, probe, detectors and autopsies in `fn_sim::balance` | #533 |
| #535 | Harness: `balance trace` for a flagged run (probe rows and an ASCII crop) | #534 |
| #536 | Autoplay: play styles as data (`steady`, `careless`, `branch_first`, `idle`) | #513 |
| #537 | Harness: `fn_harness` crate, `data/harness.toml` targets and the verdict | #534, #536 |
| #538 | Harness: baseline, `--against` and the jitter run | #537 |
| #539 | xtask: `cargo xtask balance` tiers | #537, #538 |
| #540 | Harness: a script-player style (`fn_agent play --script`) | #537 |
| #541 | Harness: the first verdict, re-classifying the open symptom issues | #539, #535 |
| #542 | Balance harness: new failures on seeds 18, 19, 27 and 30 | |

## Owner decisions

The owner took every recommendation on 2026-11-07, as starting defaults that `data/harness.toml`
can retune per target once the harness has a track record.

1. **What share of runs must pass, and what makes a site hard** (#543): 85% of valid runs on
   feasible sites, and a site is hard for a sign when its reachable supply (counting brick and
   other nation sources for stone) is below the sign's need.
2. **Which play styles count toward the verdict** (#544): `steady` gates every target;
   `careless` and `branch_first` gate only the targets that name them; scripted agents are shown
   but don't gate.
3. **The cost budget** (#545): CI unchanged; the quick tier (about 2 minutes) by hand on every
   balance change, pasted into the commit; the full tier (about 15 minutes) before a design's
   numbers are accepted.
4. **Hard sites** (#546): they stay in the game, judged against a softer bar, and the embark
   outlook warns about them.

## Not chosen

- **Random seeds each run.** Fresh seeds every time make before/after comparisons unpaired, so a
  change needs far more runs to show. The holdout panel covers overfitting instead.
- **Percentile bands as the targets** (p10/p50/p90 within a range). They're printed in the JSON,
  but "passes on 85% of feasible sites" reads directly as a design promise, and a band on a mixed
  population of sites mostly measures the site mix.
- **Many styles judged equally.** Every new style adds its own gaps, and a target that must hold
  for all of them is only as good as the weakest script.
- **Detectors that fix the player mid-run** (re-designating when idle, for example). That hides
  harness gaps instead of reporting them.
