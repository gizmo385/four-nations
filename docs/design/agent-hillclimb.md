# Scoring agent runs, and hill-climbing one strategy

Doc status: current

Status: **accepted** (#517, #518). The owner took every recommendation on 2026-11-07: see
"Owner decisions". The follow-up work is under "Follow-up issues".

The owner (2026-11-07): "The quality of a run isn't *just* how many of the signs that it's
finished, but also how many turns its taken to do that and how well its taken care of its
people." And: "I want to encourage more wholistic game planning by the agent and basically allow
it to hillclimb on game progress."

The two go together. Hill-climbing needs a number that says whether an attempt beat the last
one, and that number is only worth climbing if one strategy carries from attempt to attempt. Part
1 defines the score. Part 2 makes one strategy the thing the agent improves, and makes attempts
cheap enough to repeat.

## Decisions in brief

1. **Score = 1000 × progress × care**, compared only within a **track**: the same start (seed,
   nation, difficulty, prelude) at a fixed horizon (540 turns, 3 game hours, by default).
2. **Progress** is 14 milestones: the nine signs, Ready and the four stages. A milestone met by
   the horizon earns 1.0 if met at turn 0, falling linearly to 0.6 at the horizon. That's how
   turns taken counts. An unmet sign earns half its fraction (ore 39/40 earns 0.49), so near
   misses still give the climb a gradient, but finishing a sign always beats being close.
3. **Care** is a factor from 0 to 1. Each death costs 0.15, each close call 0.03, and time spent
   in a severe need state (Starving, Parched, Exhausted, Dying) costs twice its share of
   person-time. Care is multiplied by progress, so a tiny, safe colony scores low (little
   progress) and so does a big careless one (care cuts everything). Mild flags are reported but
   not scored.
4. **The score lives in `fn_sim::score`**, a pure function of `World` history plus a per-turn
   care sample, with weights in `data/score.toml`. The weights aren't part of the data digest,
   so retuning them never invalidates a log. `fn_agent score LOG` replays a log and prints the
   scorecard. The balance harness can reuse the same care measure (#489) and gives a **par**:
   the stock Economy opening's score on the same start.
5. **One strategy means one governor script plus a plan.** A new `plan` tool holds the model's
   intent as data (goal, sign order, research queue, build queue, targets). The governor, a
   saved script, reads the plan every loop. Most wakes then become a plan edit, not a new
   script. Inline one-off code becomes read-only (probes look, they don't act; #522).
6. **Hill-climb on the clean score.** After each attempt, the governors and plans it saved replay
   **from turn 0 with no model** (`fn_agent play --horizon`), at no token cost. The best of them
   on three screen seeds goes on to a gate of twelve, where it and the champion are compared on
   the same seeds. It becomes the champion when the gain is clear, not just positive (see
   "Measuring a difference", #571). The next attempt starts from the champion, and regressions
   are dropped while their notes are kept.
7. **Generalisation is checked, not assumed.** The model plays one seed. Seeds come in three
   tiers that never overlap: a screen of about three the model tests on, a gate of about twelve
   that decides promotion, and about thirty owner-only seeds, none in the balance panel 1-32,
   reported to the owner and never shown to the agent or selected on.
8. **`cargo xtask agent-run --track ID --attempts N`** runs the loop and writes a ledger. The
   studio shows each attempt's scorecard at the end of its replay, and `attempt 3/5, best 312`
   on the strip.

---

## Part 1: the score

### The evidence run

`~/fn-agent-runs/20261107-143532-water-7633-normal`: Water, seed 7633, Normal, scripted, 86
round trips, $0.97, stopped at turn 528 (176:00). Replayed turn by turn here with `Host::resume`
and an `observe` after each turn (23 s on a release build). This is its Endeavor screen at the
end (`fn_tui --watch run.jsonl --frame --keys G --ascii`, cropped):

```text
| The Harmony Gate                                                        3 of 9 signs || > x1     Winter Y3 4:00 left |
| PEOPLE                                                                               || People 21            Idle 18 |
| + Population                           ##########     21/16                          || Food 34 - 1/head  mild       |
|   Everyone has a bed                   #########.     18/21                          || Wood 246  Stone 412          |
| BUILDINGS                                                                            || Gold Ore 33  Tool 11  +1     |
|   One of every building                #########.     10/11                          || Research: none set       [T] |
|   Not yet raised: Igloo, Irrigation Channel                                          || Gate 3/9 - 3 beds        [G] |
| KNOWLEDGE                                                                            ||                              |
| + Water Tribes branch                  ##########       6/6                          ||                              |
| STORES AND HARVESTS                                                                  ||                              |
|   Food in stores                       ##........    34/150                          ||                              |
| + Stone in stores                      ##########   412/200                          ||                              |
|   Ore mined                            ##########     39/40                          ||                              |
| BENDING                                                                              ||                              |
|   Bending masters (8 bends in 3 forms) ..........       0/3                          ||                              |
|   Ancient scrolls read                 ..........       0/3                          ||                              |
```

Things the agent's own prose got wrong, which a computed score fixes: it reported "ore 33 of
40" (it was 39, one short), and "3 of 9 met: stone, branch, population and beds" (that's four
names, and beds had slipped to 18/21). Beds were first met at turn 13 and lost again as people
arrived. Population was met at turn 360, stone at 417 and the branch at 438. Nobody died, and no
severe flag ever showed. The mild flags summed to Hungry 4.7% of person-time, Thirsty 5.5% and
Tired 0.3%. Food per head entering the three winters was 4.6, 1.0 and 3.0. Idle share per
15-minute window ran from 33% to 78%, and averaged about 50%.

### Tracks: what is comparable

A score is only compared with scores from the same **track**: the same start (seed, nation,
difficulty, map size, site, optional prelude) scored at the same **horizon** in turns. A run
that stops early is scored on what it reached, with no extrapolation. That's why the evidence
run is scored at 540 although it stopped at 528. A run that goes past the horizon is scored as it
stood at the horizon. That fixed horizon turns "how many turns it took" into "how early each
milestone landed", which is easier to score than "turns to finish". Most runs never finish.

Across tracks, the scorecard prints a **par**: the score of `fn_sim::autoplay::Economy` on the
same start and horizon (#572). Par is privileged (autoplay reads `World`), so it measures what a
stock scripted opening reaches on this start, reading the world directly. That is not a
discovery yardstick: the opening scrolls out to `SCROLL_RADIUS` = 90 tiles and knows where every
resource is, which a player has to find. So par is not a target a fair player should hit, and it
is not a ceiling either (on seed 7 the stock opening fails and a good player beats it, #397).
Every output says it in those words. It's a yardstick only, and selection never uses it.

- **The start must match.** An agent plays `World::new` (the default start: `fn_agent play`
  without `--start`, which is what a track's `site = "default"` means), and the balance harness
  plays `World::found` at `Survey::default_site`. Par is made on the first of these
  (`fn_sim::balance::par_start`, held to the world a `Host` builds from the same header by
  `par_starts_from_the_world_an_agent_plays`), or `World::found` when the track names a site.
  Measured on the build of #572, water, normal, small, 540 turns: seeds 7633 / 7 / 42 / 1 / 5 give
  290 / 296 / 287 / 369 / 446 on the agent's start, and the harness's 517 / 276 / 541 / 561 / 582
  (the figures of the harness review) on `World::found`. They are two different games, and the
  harness's figures are the wrong yardstick for an agent's score (which start the tracks should
  play is the owner's call, #591). #582 (the connected spawn) moved
  one of the five: seed 42 was 462 before it and is 287 after; the other four are unchanged.
- **Not in the model's prompt.** Par is drawn from data the player couldn't see, and the owner
  doesn't want agents handed answers, so no prompt, scorecard text or ledger text a model reads
  carries it (`fn_agent score --no-par` is what the harness asks for there; the ledger's
  `ledger_text(.., false)` leaves it out). The issue's "may include par for train seeds" is
  settled as no.
- **Cost.** A par plays 540 turns of the opening: about 20 s. `fn_agent par` computes it once per
  build and start and keeps it under `$FN_AGENT_CACHE` (else `$XDG_CACHE_HOME/four-nations`, else
  `~/.cache/four-nations`), in `par/BUILD/`, never in a run folder, so the run browser's listing
  (#585) doesn't change. A campaign measures par after its last attempt, on all three tiers, in
  parallel, and prints how long that took; the owner tier's par costs one pass the first time on
  a build. `fn_agent score` adds it to the log's own start unless `--no-par` is given.
- **Controls.** Beside par the owner's tables show a **do-nothing** control
  (`loop { wait(10); }`: what a colony earns if nobody ever gives an order) and, with `--control
  GOVERNOR.rhai[@PLAN.json]` (the `agent-compare` syntax), a fixed governor and plan, all with no
  model, on the same seeds. They are played cleanly like any strategy and kept per build and
  tier (`$FN_AGENT_CACHE/controls/BUILD/`). The campaign's ledger records par and the controls in
  an optional top-level `references` key.
- **What par didn't reach.** The scorecard says which milestones par had not met by the horizon,
  and the owner's table which it had not met on any seed and on some. That is worded as what the
  stock opening reached, not as a proof that the rest can't be reached: there is no data-driven
  bound on the stages (hauling, travel and the Avatar's arrival all depend on the settlement), so
  the doc doesn't claim one. No score number changes.

### Progress

Milestones, in `data/endeavor.toml` order: the nine signs, then Ready, then the four stages
(foundation, pillars, arch, rite). Each has weight 1 by default, so 14 in all. A three-hour
track can't reach the stages, but every run on that track has the same ceiling.

- **Met by the horizon:** credit = `1 − (1 − late_credit) × t / H`, where `t` is the turn it was
  first met (`World::endeavor_events`' own `SignMet`, `Ready` and `StageDone`) and
  `late_credit = 0.6`. A sign met at turn 0 (in a prelude, say) earns 1.0. One met at the
  horizon earns 0.6.
- **A sign met once that has slipped by the horizon** (beds above) earns only its partial
  credit. This matches the Endeavor screen and Ready's "all at once" rule. Once Ready latches,
  every sign keeps its first-met credit, as the game does.
- **Unmet:** `partial_credit × min(1, value / target)` at the horizon, with
  `partial_credit = 0.5`. Overshoot (stone 412/200) earns nothing extra.
- **Progress** = credits summed ÷ weights summed, from 0 to 1.

### Care

`care = max(0, 1 − 0.15 × deaths − 0.03 × close_calls − 2.0 × suffering)`

- **Deaths:** `World::deaths` within the horizon. A death costs the same in a colony of 6 as in
  one of 21: each one is a person.
- **Close calls:** a settler entering Dying (health below 25%) who then lives counts once until
  their health recovers above 50%. The Avatar leaving unfed (`AvatarLeft`) also counts once.
  Both thresholds live in `data/harm.toml` `[care]`, because the count is kept on `World` (#533),
  shared with the balance harness (balance-harness.md, "4. The care measure").
- **Suffering:** person-minutes with any severe flag ÷ all person-minutes
  (`severe_any_minutes() / person_minutes()`), from `World::care()` (#533). "Severe" means below half a need's `low` (`data/needs.toml`): the same Bad status
  the People row shows as Starving, Parched or Exhausted, plus Dying. Because it's a share, a
  growing colony isn't penalised for having more people.
- **Not scored, reported:** mild flags (Hungry, Thirsty, Tired, Hurt), which every settler passes
  through on the way to a meal; harm events (injured, sick, sleeping rough), because a careful
  colony still gets hurt sometimes (harm.md's "careful" target is harm events above 0 with no
  deaths); and food per head entering each winter. Newcomers who never came aren't care: the
  population sign already rewards growth.

**Why multiply, not add.** Care as a factor means neglect discounts every gain, and a run with
no progress can't buy a score by being safe. Two deaths cost 30% of whatever the run achieved.
That's the owner's "how well it's taken care of its people" as a condition on progress, not a
separate prize.

### Worked example

Track: Water, seed 7633, Normal, default site, horizon 540.

| Milestone | First met | At the horizon | Credit |
|---|---|---|---|
| Population | t360 | 21/16 | 0.733 |
| Stone in stores | t417 | 412/200 | 0.691 |
| Branch complete | t438 | 6/6 | 0.676 |
| Ore mined | | 39/40 | 0.488 |
| One of every building | | 10/11 | 0.455 |
| Everyone has a bed | t13, slipped | 18/21 | 0.429 |
| Food in stores | | 34/150 | 0.113 |
| Bending masters | | 0/3 | 0 |
| Ancient scrolls read | | 0/3 | 0 |
| Ready, four stages | | | 0 |
| **Progress** | | 3.58 of 14 | **0.256** |

Care: 0 deaths, 0 close calls, 0% suffering, so **1.00**. **Score 256.** Par (the Economy
opening on the same start, `balance --economy --seeds 7633 --nations water --minutes 180`) meets
every sign but food, scrolls at minute 16 and masters at minute 103, for about **507** (no
deaths; its suffering isn't measured yet, so care is taken as 1.0). The agent reached half of
par, and the two signs it never started account for half of the gap. *(That par was made on the
harness's `World::found` start. On the start the agent played, `World::new`, par for this seed
is 290; see "Tracks: what is comparable".)*

The issue's comparisons, on the same track:

- **The same three signs, 40 turns sooner:** +0.03 credit each, so **262**. Speed matters, but
  less than finishing a sign.
- **Five signs (ore and kinds also met, at t500) but two deaths:** progress 0.279 × care 0.70
  = **195**, below 256. With no deaths those five signs would score **279**.
- **Beds held at the end** (met since t13): **296**. Three beds were worth 40 points, the
  scorecard's most actionable line.

### The scorecard

`fn_agent score LOG [--horizon N] [--json]` (composed to match the proposal; numbers from the
evidence run):

```text
Water Tribes · seed 7633 · Normal · small · default site            horizon 540 turns (180:00)
Played 528 turns (176:00); stopped early (stopped by SIGINT)

SCORE 256        progress 0.256 × care 1.00        par 290

PAR 290          progress 0.290 × care 1.00 · deaths 0
  what a stock scripted opening reaches on this start, reading the world directly; not a target
  a fair player should hit, and not a ceiling.
  par met 4 of 14 milestones by turn 540; it had not met: One of every building, ..., The Harmony Rite

PROGRESS                      met    credit   at the horizon
  + Population                t360    0.73    21/16
  + Stone in stores           t417    0.69    412/200
  + Branch complete           t438    0.68    6/6
    Ore mined                         0.49    39/40       one short
    One of every building             0.45    10/11       Igloo, Irrigation Channel
    Everyone has a bed        t13     0.43    18/21       slipped: 3 beds short
    Food in stores                    0.11    34/150
    Bending masters                   0.00    0/3         never started
    Ancient scrolls read              0.00    0/3         never started
    Ready, 4 stages                   0.00
                                      3.58 of 14

CARE 1.00         deaths 0 · close calls 0 · suffering 0.0% of person-time
  reported only:  Hungry 4.7% · Thirsty 5.5% · Tired 0.3% · harm events (count)
                  food per head entering winter: 4.6 · 1.0 · 3.0

DIAGNOSTICS
  idle share by 15 min  48 41 38 54 43 33 65 52 51 35 65 78 %    (par: 14 0 11 ... 74)
  research idle from  t438 (tree done)
  scripts  32 names, 25 one-shot · one-shot scripts ran 307 of 528 turns, 164 of 433 acts
```

The replay above read only what `observe` shows, so harm events aren't counted here. The real
scorecard counts them from `World::harm_events`. `--json` gives the same fields for the ledger
and for #489. Diagnostics are everything the climb
can act on that isn't scored, and all of them are values a player can see (the People row's
Idle count, the Endeavor screen, the alerts) or the agent's own actions.

### Where it's computed

- **`fn_sim::score`** (new): `ScoreDef` (from `data/score.toml`) and
  `Scorecard::compute(&World, &Defs, &ScoreDef, horizon)`. It's pure, deterministic and reads no
  clock. Suffering and close calls come from `World::care()`, the tally #533 adds for this score
  and the balance harness alike, so there's no separate tracker here. Everything else is already
  in `World`'s history (`endeavor_events`, `deaths`, `endeavor_signs` at the horizon). `data/score.toml` is loaded on its own, not into `Defs`, so
  `Defs::id_hash` (the run log's data digest) never changes when a weight does: old logs rescore
  under new weights.
- **`fn_agent score LOG`** replays the log (as `Host::resume` does) and computes the scorecard
  at the horizon through a new `surface::referee` read. That's the privileged module
  agent-play.md planned (#318), on the parity lint's allow-list and never reachable from a tool
  handler. `--append` writes a `score` commentary line to the log, which replay ignores.
  As built (#525): `--horizon` counts turns and defaults to the log's last turn. The replay stops
  before the first `Turn` line past it, so the world is never past the horizon; a horizon past
  the log's end scores the turns it reached against the full horizon. The reported-only values
  (mild flags, food per head as each winter begins, idle share, research idle) are sampled at the
  end of every replayed turn, so they are exact to one 20-second turn. Script churn is counted
  from the log's `script`, `wake`, `turn`, `keys` and `plan` lines. `par` is `null` in `--json`
  with `--no-par`, and in `play --horizon`'s scorecard, which never computes one (#572).
  The scorer lives in `fn_tui::surface::referee` and is named only in
  `crates/agent/src/scorecard.rs`; the parity lint rejects the word in every other agent file.
- **`fn_agent par`** (#572) is where an agent's par comes from: `balance::par` on the start the
  header describes, cached per build and start (`crates/agent/src/par.rs`). **`balance --economy
  --score`** prints the same scorecard per run on the harness's own start, which is where par
  first came from.

```toml
# data/score.toml: the run score (docs/design/agent-hillclimb.md). Read by fn_sim::score, by
# fn_agent and by the balance harness, never by the game rules. Not part of the data digest:
# changing a weight never invalidates a run log.
[progress]
late_credit = 0.6       # a milestone met at the horizon earns this much of one met at turn 0
partial_credit = 0.5    # an unmet sign earns this × min(1, value/target) at the horizon
default_weight = 1.0    # every sign, Ready and every stage, unless listed below
# [[progress.weight]]   # per-milestone overrides by id, e.g. { id = "scrolls", weight = 0.5 }

[care]
death = 0.15
close_call = 0.03
suffering = 2.0            # × the share of person-minutes with a severe flag
# close-call thresholds live in data/harm.toml [care], beside World::care() (#533)
severe = ["starving", "parched", "exhausted", "dying"]
```

### Parity: when the agent sees it

Every input is something a player sees: the Endeavor screen's values, the People row's flags,
the alerts' deaths. So showing the score is parity-safe. Even so, the agent gets the scorecard
**at the end of an attempt** (in the next attempt's prompt), not as a live number in `observe`.
The components are already on screen mid-run, and a live scalar invites playing the number
instead of the settlement. Races (#318) keep their own ranking (goal tick first). A race preset
could rank by score later, but this doc doesn't propose it.

### Fit with the balance harness (#489)

#489 wants to tell "careful never dies" from "careless has close calls" (harm.md, #394). The
close-call and suffering definitions above are that measure. They're counted once, on `World`
(#533), so `balance` reads the same numbers with no agent in the loop. This doc only
fixes the definitions and the module boundary. How the harness turns them into verdicts is
#489's.

---

## Part 2: one strategy, hill-climbed

### The "before"

The evidence run logged 52 `script` lines under 32 names. Twenty-five of them were one-shot
inline scripts (`[findwater]`, `[findwater2]`, `[findwater3]`, `[igloo]`, `[dock]`,
`[more-farms]`). Only `keepgoing` was revised and reused (v1 to v7, 14 runs). One-shot scripts
ran **307 of 528 turns**. Its notes rediscover the same lessons three times: designations drain
and leave people idle, `build_near` wastes refusals on mountain and water, and the stockpile
fills. Research ran out at turn 438, and nothing ever started bending or scrolls (par reads its
first scroll at minute 16). Each wake was answered locally ("build a dock now") instead of by
changing a standing plan, so nothing the agent learned survived into the next stretch of play,
or into a next game.

### Within a run: a governor and a plan

**The plan is intent, held as data.** A new tool, `plan {set?, merge?}`, reads or edits a JSON
map that scripts read with `plan()`. It's capped at 8 KB like `memory`. Every change writes a
`plan` log line, so `--resume`, `--carry` and the studio all see it. Keys are the agent's own,
with a documented convention that the scorecard and studio display:

```json
{"goal": "all nine signs by minute 150, nobody past Hungry",
 "order": ["beds", "stone", "scrolls", "population", "ore", "building_kinds", "masters",
           "branch", "food"],
 "research": ["farming", "stoneworking", "pottery", "fishing", "prospecting", "cartography"],
 "build": [{"building": "Fishing Dock", "near": "coast"}, {"building": "Igloo"},
           {"building": "Irrigation Channel"}],
 "targets": {"beds_ahead": 2, "food_per_head": 6, "stockpile_free": 0.3, "max_idle": 3,
             "stone_cap": 220}}
```

`memory` stays what scripts remember (state). `plan` is what the model wants (intent). Keeping
them apart is what makes the clean replay below possible: replaying from turn 0 starts with an
empty `memory` but keeps the plan.

**The governor is how, held as code.** By convention one saved script named `governor` owns the
whole settlement: one loop of duties (water, food, beds, stockpile, research, the build queue,
the next sign in `plan().order`, idle hands), each reading its numbers from the plan. The model
revises its own governor, it doesn't replace it. The agent is given no example governor
(#638): `sdk_reference` carries the function reference, the convention and the layout rules, and
no governor source, because a governor handed to every model is copied by every model (the first
governor of the 2026-11-09 earth-3h campaign shared 67 of 98 lines with the old example, and each
later attempt starts from the champion, so campaigns were measuring "the example plus edits").
Each model devises its own duties. A small runnable governor still lives under
`crates/agent/tests/reference/` as a test fixture, and is never shown to an agent. What a
governor tends to look like, as a model might write it (an illustration for this document, not
something the agent sees):

```rust
// governor: one loop of duties. Intent lives in plan(); this is the how.
let camp = memory.camp ?? observe().cursor;
memory.camp = camp;
loop {
    let p = plan();
    let g = observe();
    if g.research.label.contains("none set") {
        for t in p.research { if act([#{step: "research", tech: t}], "plan").ok { break; } }
    }
    food_duty(p.targets, camp);          // helpers the agent writes and keeps in this script
    beds_duty(p.targets, camp);
    build_queue_duty(p.build, camp);     // one entry at a time, retried until it stands
    sign_duty(p.order);                  // the first unmet sign in the plan's order
    if idle() > p.targets.max_idle { wake(`${idle()} idle: the plan has no work for them`); }
    wait(3);
}
```

**Most wakes become plan edits.** "Build a dock" is a `plan {merge: {build: [...]}}`, and the
governor builds it. "Food is falling after frost" is raising `targets.food_per_head`. The model
writes code only when the governor can't do something at all. That change is a revision, and the
studio's code pane already shows revisions as diffs (agent-studio.md).

**Probes look, they don't act** (#522). Inline `run_script {code}` keeps every read (`observe`,
`look`, `cells`, `can_build`, `screen`, `log`) but refuses `act`, `keys` and `wait`, with the
error "probes can't act: put this in the governor, or in the plan". Surveys like `[findwater]`
stay cheap, and every action comes from a versioned script the climb can keep.

**The scorecard counts churn**: scripts saved, inline runs, the governor's share of turns, and
plan edits. The agent sees its own habits next to its score.

### Across runs: attempts and a champion

**Tracks are data.** `data/tracks.toml` is read by `xtask` and `fn_agent`, never by the sim:

```toml
# Hill-climb tracks (docs/design/agent-hillclimb.md). A track is a start family scored at a
# fixed horizon. The model plays play_seed. Seeds come in three tiers that may not overlap
# (#571): screen_seeds, which the model tests on and which rank an attempt's versions;
# gate_seeds, which decide promotion; owner_seeds, which are reported to the owner only.
[[track]]
id = "water-3h"
name = "Water, the first three hours"
nation = "water"
difficulty = "normal"
map_size = "small"
site = "default"
# prelude = { opening = "economy", minutes = 60 }   # optional, as data/scenarios.toml
horizon_turns = 540
play_seed = 7633
screen_seeds = [7633, 7, 42]
gate_seeds = [1101, 1102, 1103, 1104, 1105, 1106, 1107, 1108, 1109, 1110, 1111, 1112]
owner_seeds = [2001, 2002, ..., 2030]   # 30 of them, written out in the file
collapse_below = 120                     # a score under this is a collapse
```

**Clean evaluation** (`fn_agent play --script governor.rhai --plan plan.json --horizon 540
--seed N --nation X`) replays a strategy from turn 0 with no model. Event wakes are ignored.
When the governor returns or calls `wake`, it runs again with its `memory` kept, and at least one
turn passes between runs, so a script that returns at once can't spin. After an `error` or
`runaway`, the colony coasts with no new orders to the horizon. Every clean run ends at the
horizon and is scored there, so a crash costs score instead of making the run incomparable. The
evidence run's 528 turns replayed in 23 s, so six seeds take about 2.5 minutes of CPU and no
tokens.

As built (#528): the loop is `fn_agent::clean::run`. `--horizon` counts turns and can't be
combined with `--turns` or `--wake-on`; `--plan FILE` (a JSON object within `plan_bytes`) also
works with `--turns`, and is written to the log as a `plan` line. A `done` or `asked` run that
used no turn is followed by one forced `end_turn`. After an `error` or `runaway` the rest coasts
one turn at a time. The log has a `script` line and a `wake` line per run. The stdout JSON is the
last run's report (its `observe` is refreshed to the horizon) plus `turns`, `restarts` (script
starts after the first), `stopped_by` (`{reason, why, turn}` or `null`) and `scorecard`: exactly
what `fn_agent score LOG --horizon TURNS --json` prints for that log, built by replaying it, so
the scorecard costs about as much CPU again as the run. The exit is nonzero only when
`stopped_by` is set.

As built (#561): the model hears about stops two ways. The ledger text in the attempt prompt marks
a stopped clean run with `*` and adds one line per attempt (reason, how many screen seeds, the turn
range, the first one's `why`), and says that a stopped run coasts, so an early stop costs most of
the score; gate and owner-only seeds never appear in it. And during an attempt `fn_agent serve --trial-seeds
LIST --trial-turns N` offers `test_governor`: the saved governor and the current plan played from
turn 0 on a screen seed (the play seed by default), for up to the horizon, in a world of its own
built from the live game's header with the seed swapped. It scores the way `play --horizon` does,
so the stop reason, turn and score are the clean evaluation's own. The live game, its log and its
replay are untouched (the only trace is the `call` line). The result is one short JSON object. It
is bounded to 6 calls per attempt, counted from the log's `call` lines so chunked sessions share
the count; a full 540-turn test took 4 s to 9 s in the release build. The harness hands over the
track's screen seeds only.

**The attempt loop** (`cargo xtask agent-run --track water-3h --attempts 5`):

1. **Attempt 1** is a normal `--scripts` run on `play_seed` to the horizon, with the governor and
   plan prompts.
2. **After every attempt**, the runner scores the played log (the **played score**), cleanly
   evaluates the governors and plans the attempt saved on the screen seeds (the **clean score**,
   their mean), runs the best of them on the gate seeds, and, if it was promoted, on the
   owner-only seeds. It appends a row to `ledger.json`.
3. **Selection and promotion** (#571, "Measuring a difference" below): the pair with the best
   screen mean is the attempt's candidate. It becomes the champion (`champion/handoff.json`)
   when its mean gain over the champion on the gate seeds, paired by seed, has a 95% bootstrap
   interval wholly above zero and it does not collapse on more gate seeds than the champion.
   Otherwise the champion stays, the attempt's notes are kept (so the next attempt doesn't retry
   the same idea), and its code and plan are dropped. Runs are deterministic, but a seed is one
   draw from a skewed distribution, so a difference on a few seeds is not a difference in the
   strategy; the gate asks for one that holds across the seeds.
4. **The next attempt** starts fresh on `play_seed`, with the champion's governor, plan and notes
   **preloaded** by `fn_agent serve --carry-in`. That replaces today's `--carry`, which asks the
   model to retype every script. The first prompt carries the ledger's training rows and the
   champion's scorecard for `play_seed`, and asks for one thing: raise the clean score by
   improving the governor and plan. The governor plays until something surprises it, so a good
   champion costs few round trips.
5. The loop stops after N attempts or at `--max-usd`, then prints the ledger.

As built (#569): the live game stops at the horizon (`fn_agent serve --horizon N`, passed with the
trial flags on every chunk). `end_turn` and `run_script` play no turn past it; a governor that
runs into it ends with reason `horizon`, not `budget`, `error` or `runaway`, and every such result
says the game has reached its horizon and what to do (test, save, notes, stop). A saved script
started at the horizon isn't run; a probe is. `test_governor`, `save_script`, `plan` and `notes`
keep working, so the time after the horizon is for the thing that is scored. The session then gets
80 more tool calls (30 before #570; counted from the log, so a resume doesn't repeat them) and later calls are
refused with "stop now". `--chunk-calls` only asks the model to stop; this cap, `--max-usd` and
the chunk loop's own rules are what bound an attempt. The log holds no turn past the horizon, so
it replays and scores as it would have without the stop. An attempt is finished when the game is
at its horizon (after the chunk that got there), when a chunk changes nothing (no turn, script,
plan, note or test), or at `--max-usd`; a chunk that played no turns but edited or tested is the
same attempt carrying on, and its handoff says so instead of "turn 0 so far". Every prompt of a
campaign (attempt 1's, later attempts' and every handoff) carries one shared passage on what
decides the attempt (the clean score from turn 0 on other maps) and how to test it
(`test_governor`).

As built (#570): `test_governor` returns more than a number, and an attempt keeps the best
governor it saved, not the last.

- *The result.* Still the clean evaluation's own stop reason, turn and score, and now `progress`
  and `care`, the signs in one line (the turn each was met, how far an unmet one got), the deaths
  with cause and turn as the alert says them, and a `trace`: 12 columns at most (a 540-turn run
  is 12 slices of 45 turns, 15 game minutes), one row each for the turn, people alive, idle share,
  settlers hungry and thirsty (the People row's mild or severe word) and the player's own `act`
  calls that did nothing. The first line of the trace says what each row is, and the most common
  refusal follows it. That is the summary, about 1.2 KB. `detail: true` adds food in stores,
  tired, hurt and deaths per slice, the five most common refusals with their first and last turn,
  and the last six distinct lines the governor's own `log()` wrote (the runner keeps its last 30;
  lines are cut at 90 characters): about 2 KB, under 3.2 KB at the very most (pinned by a test). `turns` with a small number zooms the same
  12 columns in on the start. Every number is something a player could see: the People row,
  the stores, the idle count, the death alerts, and the notices a failed designation or build
  shows. The signs are the scorecard's, as `fn_agent score` prints them for the champion. Nothing
  names a map but the one tested, and the tool knows no gate or owner-only seed.
- *Reading it, on the failure that prompted this* (seed 42 under the review's champion
  governor, 3 deaths and 3 people at the end): the summary shows three deaths of starvation at
  turns 78, 105 and 128, everybody idle (96% in the first slice) and hungry and thirsty from
  turn 30, 7 food in stores at turn 45, and refusals that are noise (a repeated designation
  "already designated", "Bore Tunnel isn't usable yet"). A model can see *that* the start failed
  and *when*, and that the colony is idle, not that no water is within reach of the start; for
  that it adds `log()` lines to the governor and reads them in the detail form of a short test,
  which the tool's description says. The trace doesn't name the cause by itself.
- *The limit.* `data/agent.toml`'s `[attempt] max_tests = 30` (was 6). A test costs seconds and
  one call, and the fix it asks for another, so 30 tests are 60 calls. `HORIZON_GRACE_CALLS`, the
  calls a session has after the game reaches its horizon, is 80, and a test pins it at
  `2 x max_tests + 20`: every test as a pair, plus the plan, the notes and a last save. A model
  that spends its tests before the horizon has all of the 80 for finishing up.
- *Parity.* `--trial-seeds` is passed by `agent-run --track` and by nothing else (a test reads
  the sources to pin that no other code assigns `Opts::trial`). Testing on the play seed is a
  lookahead copy of the model's own game: right for climbing, wrong for a race, which has no
  track and so never has the tool.
- *Which version an attempt hands on.* When an attempt ends, `agent-run` reads every governor
  version it saved from `run.jsonl` and evaluates each on the screen seeds, with no model.
  A candidate is a pair, a governor and the plan it is judged with, because the clean run uses
  both. The pairs are the states the model held: for each version, the plan as it stood when the
  model moved on from it (the next save of the governor, or the end), and the plan at each
  `test_governor` call while it was current, which are the pairs the model was shown a score for.
  Same code and same plan is one pair. The best by screen mean is the attempt's candidate (a tie
  goes to the later pair), runs the gate seeds, and, if it is promoted (#571), the owner-only
  seeds; it is what the champion keeps, with that pair's plan; `attempt-K/handoff.json` still
  holds the attempt's own final state and `chosen-handoff.json` the pair that became champion.
  The gate and owner-only scores play no part in choosing the pair (the choice takes screen
  means, nothing else; a test swaps the other tiers' numbers and the choice doesn't move).
- *Cost.* Evaluating one pair is three screen runs: 29 s for the first campaign's champion
  governor (13 s, 15 s and 2 s on seeds 7633, 7 and 42, release build), up to 3 to 4 minutes for
  a heavy one, and 2 to 103 s a run in the review. Run one after another, 12 pairs on 3 seeds
  and 12 gate seeds for the candidate (48 runs) are about an hour at the 70 s a run measured
  below. At most 12 pairs are screened: the final one always, then the tested ones newest first,
  then the untested ones newest first. Below 12 every saved version is screened; above it the
  oldest untested ones are not. The order is what makes it affordable: the screen is small and
  runs every pair, the 12-seed gate runs for the candidate only (the champion's gate scores are
  kept), and the 30-seed owner-only tier runs only for a promoted candidate, all on a parallel
  queue (below). The ledger's `tried` column reads "3/9" for 3 screened of 9 saved,
  `eval/versions.json` has each pair's screen mean and the seconds each stage took (screen,
  gate, owner-only), and the campaign prints them.

```text
ledger · water-3h · horizon 540                                          (owner-only tier: owner only)
 #  governor  plan  tried  played  screen 7633/7/42  mean  gate  vs champ [95%]  collapses  gate deaths  owner-only (30 seeds)                            kept   cost
 1  v9        r6     4/9    256    231 / 174 / 205    203   188              -       2/12            3  205 · collapse 3/30 · deaths 9 · stopped 0  champ  $0.97
 2  v12       r9     6/14   301    288 / 240 / 251    260   236   +48 [+12, +91]       1/12            2  241 · collapse 2/30 · deaths 5 · stopped 0  champ  $0.41
 3  v14       r11    3/3    322    301 / 198 / 243    247   229    -7 [-41, +30]       1/12            3                                           -  no     $0.36
```

*(Row 1's played score is the evidence run. Every other number shows the format and isn't
measured. The prompt gets the same table without the `gate deaths`, `owner-only` and `cost`
columns. In row 3 the screen mean is nearly as high as row 2's, and the gate says the gain is
not there.)*

`cargo xtask agent-eval --track ID --champion DIR` reruns only the clean evaluation. After a
balance change, that re-scores the current champion for free: the regression loop
agent-scripting.md promised, now with a number.

As built (#529): `data/tracks.toml` has strict keys and is validated on load (the tiers may not overlap, and a gate or owner-only seed may
not be in 1-32 or be `play_seed`, #571). `agent-eval` reads `DIR/handoff.json` (the file
`agent-run --carry` writes): the script named `governor`, and the plan if there is one; a handoff
with no `governor` is an error. It runs `fn_agent play --horizon` once per seed of every tier
(screen, gate and owner-only since #571; train and held-out before, one at a time), in parallel,
and prints the score, progress, care and deaths per seed, each tier's mean and its failure counts
(`--json` prints the same table as JSON; `--out DIR` keeps each seed's log as `seed-N.jsonl`;
`--tiers` picks tiers, `--threads` the parallelism). A run whose script was stopped by an error or a runaway is scored at the
horizon as usual and flagged in the table, not failed. `fn_agent play` gained `--map-size` and
`--prelude OPENING:MINUTES` so a track's whole start can be passed on its command line; the
reusable core is `agent_eval::evaluate_many`, for the attempt loop (#530) and `agent-compare`.

As built (#530): `cargo xtask agent-run --track ID --attempts N [--model M] [--max-usd X]
[--chunk-calls C]` makes a campaign folder under `$FN_AGENT_RUNS` (`DATE-campaign-ID`) with
`attempt-K/` run folders (each a `--scripts` run of the track's whole start, played to the
horizon, with `eval/` (see #570 below) and its own `handoff.json` beside the usual files),
`ledger.json` and `champion/` (`handoff.json`, `meta.json` and `scorecard.txt`, the champion's
clean run on `play_seed`). After each attempt it runs `fn_agent score --append`, evaluates the
final `governor` and plan, writes the champion if the candidate passes the gate (#571; it was
"the train mean is strictly higher" until then), and only then appends the ledger row, so an attempt with no row is one a crash
cut short: it isn't counted and its folder is never reused. An attempt that never saved a
`governor` gets a row with dashes and isn't kept. The ledger row's plan revision counts plan
edits across the campaign (a preloaded plan isn't an edit); its governor version is the saved
script's own, which carries on from the champion's. `serve --carry-in FILE` (with `--scripts`,
a fresh session only) writes the handoff's scripts, notes and plan as log lines before the first
call, and `serve` gained `--map-size` and `--prelude` so a track with a site or a prelude is
played as it is evaluated. The carried-in governor is readable with `scripts {name}` (#560);
the first attempt of the campaign wrote a v21 from scratch because it couldn't read v16, so the
prompt asks for a small revision saved under the same name. The later attempts' first prompt is the script prompt
(`default_script_prompt`) under a block with the ledger's screen and gate columns, the champion's
scorecard and the one ask; owner-only numbers appear only in `ledger.json` and the table printed
at the end. `--max-usd` is a per-attempt `--max-budget-usd` of what's left of the invocation's
cap, and the loop stops once it is spent. `--max-usd` is required for `--track` and for
`--resume` too (#568; what bounds a chunk is in agent-play.md, "What bounds a run's spend"); an
attempt whose chunk leaves no readable cost stops the campaign and gets no row. `agent-run --resume DIR [--attempts N]` reads the
ledger (which holds the track as it began), refuses a folder with no ledger or a tracks file
whose entry differs in anything but its label, and numbers new attempts after every row and
every `attempt-K` folder. `--tracks FILE` swaps the tracks file, for tests. Still to do: the
`attempt` and `champion` fields of the `score` line (the watcher issue).

**The build (#566).** A log's data digest covers data only, so the first campaign's attempt 1
(`score` line 355, care 0.997) rescored to 294 (care 0.83) under the same digest: the sim had
changed under it. Every binary now knows its build (`fn_agent build-id`, `fn_tui::buildid`): the
last commit that touched code or data (`crates/` but the harness, `data/`, the manifests), plus
`+HEX` when there are uncommitted changes to those paths (a hash of the diff, so the same edit is
the same build and an afternoon of unrelated runs on one dirty tree is not a new build per run),
or `unknown` without git. It is baked in by `crates/tui/build.rs`, so a commit or an edit
re-runs a build script and recompiles `fn_tui` and `fn_agent` incrementally, and nothing touches
`fn_sim`. The run log's header, each `score` line and each ledger row and champion record carry
the build, and all of them read back without it. `fn_agent score` warns on stderr when the log's
build isn't its own, and refuses nothing: comparing across builds is often the point. Every
`agent-run --resume` asks `fn_agent build-id` and, if the champion's recorded build differs
(or none was recorded), evaluates it again first with no model (`reeval-N/` in the campaign
folder). The new mean replaces the champion's in `champion/meta.json` and every attempt after
it is compared with that. The old mean stays in `ledger.json` (`reevals`) and both show in the
ledger text, the prompt included. The run browser (`score-cache.json`) is keyed on the build
too, so after a rebuild each run scores again once in the background, and a logged score some
other build computed carries a `~`.

### Measuring a difference (#571)

The first campaigns promoted a candidate whose mean over three seeds was higher by any amount.
That is true of the arithmetic ("runs are deterministic, so any difference on the same seeds is
real") and false of the strategy: the seeds are three draws from a skewed distribution. The
review of 2026-11-08 (`dist.txt`, 24 fresh seeds each of two governors) found:

- one governor's score on one seed has a standard deviation of 61 to 120, and about a fifth of
  seeds collapse below 120 where a working run scores 250 to 400 (the distribution is bimodal);
- the mean of three seeds of one governor ranged from 148 to 361 (5th to 95th percentile), so
  the old rule (any strictly higher mean over three seeds) promoted a candidate that changed
  nothing half the time, the sign of a mean difference being a coin toss;
- the standard error of a mean is about 70 at 3 seeds, 35 at 12 and 22 at 30;
- the held-out seeds 1, 5 and 8 sit inside the balance panel 1-32.

**The tiers** (`data/tracks.toml`, validated on load: no seed in two tiers, none of the gate or
owner-only seeds in 1-32, `play_seed` in the screen at most):

| tier | seeds | what it does | who sees it |
|---|---|---|---|
| screen | 3 (`7633, 7, 42`) | `test_governor`'s maps; ranks the 12 or fewer governor and plan pairs of an attempt | the model |
| gate | 12 (`1101-1112`) | the candidate and the champion are compared on these, paired | the model sees the aggregates (gate mean, gain with its interval, collapse count), never a seed |
| owner-only | 30 (`2001-2030`) | reported to the owner; what `agent-compare` measures | the owner |

**Selection and promotion use them in this order.**

1. *Rank on the screen.* Every pair of the attempt (at most 12) runs on the 3 screen seeds, all
   in one parallel queue. The best screen mean is the attempt's candidate (a tie goes to the
   later pair). This step only picks which pair goes on; it promotes nothing. Picking the best
   of twelve by three seeds is a biased estimate of that pair, which is why nothing is decided
   on those seeds.
2. *Judge on the gate.* Only the candidate runs the 12 gate seeds. The champion's gate scores
   come from `champion/gate.json`, made when it was promoted (or when a resume on a new build
   re-evaluated it) and run again only then. Candidate and champion are paired by seed. The
   candidate is promoted when **both**:
   - the 95% percentile-bootstrap interval of the mean difference (candidate minus champion;
     2000 resamples from a fixed-seed generator, so the same scores always give the same
     interval) lies wholly above zero; and
   - the candidate has no more collapses (a gate-seed score below the track's `collapse_below`,
     120) than the champion.
   The first governor of a campaign has no champion and is promoted as it is; its gate scores
   become the champion's.
3. *Owner-only.* A promoted candidate also runs the 30 owner-only seeds. This decides nothing; a
   candidate that is not promoted is not run on them. The ledger row keeps the result, and a
   test checks that no seed, score, stop reason or count of this tier appears in a prompt or in
   the choice of the pair.

The rule is deliberately plain. The interval asks for a gain that holds across the seeds rather
than one that hangs on a few: with the whole difference carried by one seed, a third of the
resamples (11/12 to the 12th power) leave it out, their mean is zero, and the lower end of the
interval is zero, so it is not promoted; the gain has to be spread over about four of twelve
seeds before it clears. The collapse condition covers what a mean of a bimodal score hides: a
candidate can raise the mean by 100 while failing on one more seed, which trades a robust
governor for a flaky one. Deaths and stopped scripts are counted and reported beside the
mean (`deaths 5 on 3 seeds`, `stopped 1`) but decide nothing: a death already costs score, and a
stopped script scores as the collapse it is.

**What the gate can detect, in plain numbers.** Twelve paired seeds with a standard deviation of
74 on the difference (the evidence: 73.7 between the two governors on 24 seeds, a mean of +125)
have a standard error of 21. Synthetic data, a fixed generator, 10 000 candidates per cell
(`cargo test -p xtask --release -- --ignored --nocapture power_table`; a smaller version of the
test is in the suite):

| true gain per seed | 0 | +20 | +40 | +60 | +80 | +100 | +125 |
|---|---|---|---|---|---|---|---|
| promoted, differences Normal(gain, 74) | 4.8% | 20% | 51% | 82% | 96% | 99.6% | 100% |
| promoted, bimodal seeds (a fifth collapse) | 4.5% | 20% | 49% | 79% | 94% | 98% | 99.5% |

So a candidate that changes nothing is promoted about 1 time in 20 (the interval's lower end is
a nominal 2.5% tail; a percentile bootstrap on 12 values runs about twice that, as it is known
to), a +40 gain about half the time, and about +55 is the 80% mark. Below +20 the gate mostly
says no, so most small real improvements are rejected and the campaign stalls instead of
drifting; if most candidates change little, a fair share of the promotions will still be noise.
A gain like the review's +125 is found every time. The old rule (strictly higher mean on 3
seeds) promoted a candidate that changed nothing about half the time. Since the champion's gate
scores were part of what promoted it, they are a little lucky, which makes the bar for the next
candidate slightly higher than the true gain it has to beat. The owner-only tier has no such
bias: it is for judging a campaign, not for selecting in it. With 30 seeds the standard error
of a paired difference is about 13, so `agent-compare` resolves a difference of about 27 (its
95% interval's half-width). Both tables come from synthetic data, not from the game: how the
real distribution differs (heavier tails, correlated seeds) moves them.

**Parallel evaluation.** `agent_eval::evaluate_many` runs the queue of (pair, seed) runs on
`--threads N` threads (`$FN_EVAL_THREADS`, else half the cores, at most 12, fewer if
`MemAvailable` allows fewer at 100 MB each: 24 cores gives 12, the owner's 8-core laptop 4). A
run is one core and 27 to 30 MB at its peak (measured), so memory is not the limit, other
processes are. Each table lists its rows in tier and seed order, so the output is the same
whatever the thread count and whichever run finishes first (a test runs the same evaluation on
1, 2, 5, 14 and 64 threads, with a stub whose later seeds finish first). A failing run stops the
queue, and the error reported is the earliest in queue order.

*Measured*, release `fn_agent` at build c1ddaff, on the 24-core desktop with 12 threads, while
other agents' builds and a campaign kept the load average between 4 and 13 (so these are slower
than an idle machine would be):

| what | runs | wall-clock |
|---|---|---|
| one pair, the 3 screen seeds, 1 thread / 3 threads | 3 | 213 s / 115 s (the slowest run is the floor; the tables are byte-identical) |
| an attempt's screen: 12 pairs (one governor, 12 plans) on 3 seeds | 36 | 310 s |
| the gate for the candidate, 12 seeds | 12 | 241 s (one seed took about 4 minutes by itself) |
| the owner-only tier for a promoted candidate, 30 seeds | 30 | 343 s |
| **a whole promoted attempt** | **78** | **894 s, 15 minutes** |
| an attempt that is not promoted (screen and gate) | 48 | about 9 minutes (551 s) |
| `agent-compare`, two governors on the 30 owner-only seeds | 60 | 362 s |

At the 71 s a run that the serial row implies, 78 runs one after another are about 90 minutes
and 48 about 57, so 12 threads buy about 6 times, short of 12 because a stage ends when its
slowest run does and the machine was shared. The wall-clock of a stage is that of its slowest
run: a gate cannot finish before the heaviest of its seeds. Runs are queued in the order of
the track, not longest first, because nothing says in advance which will be long.

**Re-evaluation on a new build** (#566) uses the tiers too: a resume on another build runs the
champion on the screen and the gate (not the owner-only tier), replaces its means and
`champion/gate.json`, and keeps the old and new means in the ledger's `reevals`, so the next
attempt is compared with the champion as this build scores it.

**`cargo xtask agent-compare [--tier owner] [--threads N] CONDITION...`** compares conditions
with no model. A condition is `[LABEL=]PATH[,PATH...]`: a campaign folder (its `champion/` is the
strategy), a folder with a `handoff.json`, or `GOVERNOR.rhai[@PLAN.json]`, a fixed governor
and plan. Every strategy runs on the tier's seeds (default owner-only), and the command prints
the mean, standard deviation, minimum and failure counts per strategy, and for each later
condition its paired difference from the first, with a 95% bootstrap interval and wins, losses
and ties. For a condition of several campaigns' champions the per-seed score is their average,
and the min, median and max of the champions' means are printed beside the interval, because
the interval covers seeds and not how much one campaign differs from another. `--json`
prints the same; `--per-seed` adds a score table. It needs `--track` unless a campaign folder
names one. A campaign folder from before the tiers works here: only its champion is read.

*First use* (2026-11-08, build c1ddaff): the review's `champion-governor.rhai` against
`champ-nowait.rhai` with their plans on the 30 owner-only seeds, which the review had measured on
24 other seeds at +125. Now: champion 129.6 (sd 67, 9 collapses of 30, 27 deaths on 10 seeds),
no-wait 256.9 (sd 109, 4 collapses, 12 deaths on 6 seeds), paired difference **+127.4, 95%
interval [+101.6, +154.6], 28 wins, 1 loss, 1 tie**; 6 minutes 18 seconds. The review's finding
holds on fresh seeds and a changed sim; a second evaluation of the no-wait governor on the same
seeds gave the same mean, spread and failure counts.

**Old ledgers** are refused by `agent-run --resume` with a message that says the ledger is from
before the seed tiers (its track has train and held-out seeds and no gate), that a promotion
can't be compared with its rows, and what to do (start a new campaign; the old folder's champion
still goes into `agent-compare`). A track whose seeds changed was already refused on resume.

### Fairness: lookahead and held-out seeds (#521, extended by #571)

Climbing on one seed teaches the map (where the water is, which outcrop is reachable) as much as
the game. Three layers keep that honest:

- **The model plays only `play_seed`.** It never sees another seed's map.
- **Selection is on seeds it did not play.** A plan full of `play_seed` coordinates scores badly
  on seeds 7 and 42, so a governor that finds its own sites wins. That pushes the agent toward
  the porcelain the evidence run's SDK gaps asked for (find water, prefer plains). The model
  tests on the screen (`play_seed`, 7, 42); the gate that promotes a candidate is twelve other
  seeds it is never told.
- **`owner_seeds` are never selected on and never shown to the agent.** The ledger shows them
  to the owner, and a growing gap between gate and owner-only means overfitting.

**The record of #521 (2026-11-07, the owner took the recommendation)**: selection by the mean
over three training seeds, with the model playing one of them, and three held-out seeds shown to
the owner only. **#571 extends it (2026-11-08)** after the harness review found that three seeds
cannot carry a promotion (the mean of three seeds of one governor ranged from 148 to 361 between
the 5th and 95th percentile, about a fifth of seeds collapse below 120, and the held-out seeds
1, 5 and 8 sit inside the balance harness's panel 1-32, so they were not independent of the
game's tuning). What stays: the model plays one seed, no seed outside the screen is shown to it,
and a tier is reported to the owner and never selected on. What changes: the held-out tier is
replaced by an owner-only tier of 30 fresh seeds (2001-2030, none in 1-32 or in the balance
harness's own holdout 1001-1032); the selection that three seeds could not carry moves to a gate
of 12 fresh seeds (1101-1112); the three seeds stay as the screen. The tiers may not overlap, and
`data/tracks.toml` refuses a gate or owner-only seed in 1-32. The owner-only tier plays no part
in selection and no seed, score, reason or count from it reaches a prompt (tests pin it).

For races (#318), a racer may not carry a strategy climbed on its race's own seed. If scripted
races ever allow carried libraries, they rank in their own bracket, as #474 already does for
scripts.

### Cost

| | Round trips | Tokens | CPU |
|---|---|---|---|
| Attempt 1 (as the evidence run) | ~86 | ~$1 | |
| Later attempts (governor plays until surprised) | target 20–40 | ~$0.25–0.50 (Sonnet's scripted rate, agent-scripting.md) | |
| Clean evaluation of one pair, 3 screen seeds × 540 turns | 0 | $0 | ~30 s to 4 min |
| The gate for a candidate (12 seeds), and for a promoted one the owner-only tier (30 seeds) | 0 | $0 | see "Measuring a difference" |
| A 5-attempt campaign | | ~$2–3 | see "Measuring a difference" |

Clean evaluation runs in parallel (#571): half the cores by default, so 4 runs on the owner's
8-core laptop, each about 30 MB. CPU is what a campaign costs, not memory: see "Measuring a
difference".

### Watching attempts

- The scored log ends with a `score` line (`fn_agent score --append`). When the watcher replays
  it, the feed shows the scorecard's top block as a card, and the strip reads `run finished ·
  score 256`.
- The campaign runner writes `attempt` and `champion` fields into each attempt's `score` line,
  so the strip carries them with no extra file. `cargo xtask agent-run watch CAMPAIGN_DIR`
  replays attempts in order.
- `plan` lines appear in the feed as diffs, the same way `save_script` revisions do.

Today's strip at the end of the evidence run (`fn_tui --watch run.jsonl --frame --at 528 --ascii
--size 120x24`, last row), then the proposal:

```text
 AGENT water  FOLLOW  turn 528  176:00  SCRIPT [winter]    x1 acts _*________ 3 1[2]3
 AGENT water  FOLLOW  turn 528  176:00  attempt 1/5  run finished · score 256 · best 256  x1 acts _*________ 3 1[2]3
```

---

## Not chosen

- **Ranking lexicographically** (signs, then turns, then deaths), as races do. It's clear, but
  it never trades a death against a sign, and the owner wants care weighed in.
- **Progress as the area under a live progress curve**, sampled every turn. It rewards early
  progress smoothly, but stores sloshing around a target make it noisy, and it's harder to read
  than "met at t360".
- **Climbing inside one run** with checkpoints and retries from turn T. That's lookahead on the
  same world, it branches the log, and it measures the patch rather than the strategy. Clean
  replay from turn 0 measures the strategy.
- **Model-free search** (mutating the plan's numbers and keeping what scores better). Clean
  evaluation is free, so this is a cheap later extension, a tuner over `targets`. It isn't
  proposed now, because the owner wants the agent planning.
- **Normalising scores across nations or seeds.** Par is shown for context only. Selection
  stays within a track.

## Follow-up issues

In dependency order. Decisions gate the items noted.

- #523 `fn_sim::score`: `data/score.toml`, `Scorecard` (blocked by #533, the shared care tally)
- #525 `fn_agent score LOG`: replay, `surface::referee`, scorecard text and JSON, the `score`
  log line (#523)
- #526 `balance --economy --score`: the scorecard per run, and par for `fn_agent score` (#523)
- #524 `fn_agent`: the `plan` tool, `plan()` binding, `plan` log line, restored by `--resume`
  (no blockers)
- #527 `fn_agent`: inline probes are read-only (decision #522)
- #528 `fn_agent play --horizon` and `--plan`: clean evaluation to a fixed horizon (#525, #524)
- #529 `data/tracks.toml` and `cargo xtask agent-eval`: clean scores on train and held-out seeds
  (#528, decision #521)
- #530 `cargo xtask agent-run --track --attempts`: the attempt loop, ledger, champion,
  `serve --carry-in` (#529)
- #531 Script-mode prompts teach one governor and a plan, and the briefing states the score
  rule (#524, #523, decision #522)
- #532 Watch: the `score` line as a card, plus attempt and best on the strip (#525, #530)
- #571 Measurement: seed tiers, parallel evaluation, the paired promotion rule with failure
  counts, `agent-compare` (extends #521; see "Measuring a difference")
- #572 Controls: par on the scorecard, in `agent-eval` and the campaign ledger (never a prompt),
  and the do-nothing and fixed-governor controls

## Owner decisions

The owner took every recommendation on 2026-11-07.

1. **The score's shape and weights** (#519): 1000 × progress × care, as above. 14 milestones of
   weight 1, a late credit of 0.6, a partial credit of 0.5, deaths 0.15, close calls 0.03,
   suffering 2.0, at a 540-turn horizon.
2. **What counts as care** (#520): deaths, close calls (Dying, or the Avatar leaving unfed) and
   severe-need time. Mild flags, harm events and food security are reported but not scored.
   The tallies are kept on `World` by #533, shared with the balance harness.
3. **What the climb selects on** (#521): the mean clean score over three training seeds, with
   the model playing one of them and three held-out seeds shown to the owner only. *Extended
   2026-11-08 by #571 (see "Fairness"): the three seeds are now the screen, promotion is
   decided on a gate of twelve fresh seeds, and the held-out three are replaced by thirty
   owner-only seeds outside the balance panel.*
4. **How hard to push toward one strategy** (#522): the plan tool, plus read-only inline probes,
   with churn reported on the scorecard.
