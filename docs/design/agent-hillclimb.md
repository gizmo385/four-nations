# Scoring agent runs, and hill-climbing one strategy

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
6. **Hill-climb on the clean score.** After each attempt, the final governor and plan replay
   **from turn 0 with no model** (`fn_agent play --horizon`) on the track's training seeds, at
   no token cost. The best mean clean score is the champion. The next attempt starts from the
   champion, and regressions are dropped while their notes are kept.
7. **Generalisation is checked, not assumed.** The model plays one seed. Selection uses the mean
   over three training seeds. A held-out panel of three more is reported to the owner and never
   shown to the agent or selected on.
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
same start and horizon. Par is privileged (autoplay reads `World`), so it measures what the rules
allow a player who knows the game, not parity-limited play. It's a yardstick only, and selection
never uses it.

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
par, and the two signs it never started account for half of the gap.

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

SCORE 256        progress 0.256 × care 1.00        par 507 (Economy opening)

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
  from the log's `script`, `wake`, `turn`, `keys` and `plan` lines. `par` is `null` until #526.
  The scorer lives in `fn_tui::surface::referee` and is named only in
  `crates/agent/src/scorecard.rs`; the parity lint rejects the word in every other agent file.
- **`balance --economy --score`** prints the same scorecard per run, which is where par comes
  from.

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
revises it, it doesn't replace it. `sdk_reference` shows the shape, as a frame and not a
strategy:

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
# fixed horizon. The model plays play_seed; selection uses the mean clean score over
# train_seeds; holdout_seeds are reported to the owner only.
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
train_seeds = [7633, 7, 42]
holdout_seeds = [1, 5, 8]
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
a stopped clean run with `*` and adds one line per attempt (reason, how many train seeds, the turn
range, the first one's `why`), and says that a stopped run coasts, so an early stop costs most of
the score; held-out seeds never appear in it. And during an attempt `fn_agent serve --trial-seeds
LIST --trial-turns N` offers `test_governor`: the saved governor and the current plan played from
turn 0 on a train seed (the play seed by default), for up to the horizon, in a world of its own
built from the live game's header with the seed swapped. It scores the way `play --horizon` does,
so the stop reason, turn and score are the clean evaluation's own. The live game, its log and its
replay are untouched (the only trace is the `call` line). The result is one short JSON object. It
is bounded to 6 calls per attempt, counted from the log's `call` lines so chunked sessions share
the count; a full 540-turn test took 4 s to 9 s in the release build. The harness hands over the
track's train seeds only.

**The attempt loop** (`cargo xtask agent-run --track water-3h --attempts 5`):

1. **Attempt 1** is a normal `--scripts` run on `play_seed` to the horizon, with the governor and
   plan prompts.
2. **After every attempt**, the runner scores the played log (the **played score**), cleanly
   evaluates the final `governor` and plan on every training seed (the **clean score**, their
   mean), and runs the held-out panel. It appends a row to `ledger.json`.
3. **Selection:** if the mean clean score beats the champion's, the attempt's governor, plan and
   notes become the champion (`champion/handoff.json`). Otherwise the champion stays, the
   attempt's notes are kept (so the next attempt doesn't retry the same idea), and its code and
   plan are dropped. Runs are deterministic, so any difference on the same seeds is real, not
   noise.
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
30 more tool calls (counted from the log, so a resume doesn't repeat them) and later calls are
refused with "stop now". `--chunk-calls` only asks the model to stop; this cap, `--max-usd` and
the chunk loop's own rules are what bound an attempt. The log holds no turn past the horizon, so
it replays and scores as it would have without the stop. An attempt is finished when the game is
at its horizon (after the chunk that got there), when a chunk changes nothing (no turn, script,
plan, note or test), or at `--max-usd`; a chunk that played no turns but edited or tested is the
same attempt carrying on, and its handoff says so instead of "turn 0 so far". Every prompt of a
campaign (attempt 1's, later attempts' and every handoff) carries one shared passage on what
decides the attempt (the clean score from turn 0 on other maps) and how to test it
(`test_governor`).

```text
ledger · water-3h · horizon 540                                   (held-out: owner only)
 #  governor  plan  played  clean 7633/7/42      mean   held-out 1/5/8   kept   cost
 1  v9        r6     256    231 / 174 / 205      203    188 / 160 / 197  champ  $0.97
 2  v12       r9     301    288 / 240 / 251      260    233 / 201 / 240  champ  $0.41
 3  v14       r11    322    301 / 198 / 243      247       ...           no     $0.36
```

*(Row 1's played score is the evidence run. Every other number shows the format and isn't
measured.)*

`cargo xtask agent-eval --track ID --champion DIR` reruns only the clean evaluation. After a
balance change, that re-scores the current champion for free: the regression loop
agent-scripting.md promised, now with a number.

As built (#529): `data/tracks.toml` has strict keys and is validated on load (a held-out seed may
not repeat a train seed or `play_seed`). `agent-eval` reads `DIR/handoff.json` (the file
`agent-run --carry` writes): the script named `governor`, and the plan if there is one; a handoff
with no `governor` is an error. It runs `fn_agent play --horizon` once per train seed and then once
per held-out seed, one at a time, and prints the score, progress and care per seed, the train mean
and the held-out scores (`--json` prints the same table as JSON; `--out DIR` keeps each seed's log
as `seed-N.jsonl`). A run whose script was stopped by an error or a runaway is scored at the
horizon as usual and flagged in the table, not failed. `fn_agent play` gained `--map-size` and
`--prelude OPENING:MINUTES` so a track's whole start can be passed on its command line; the
reusable core is `agent_eval::evaluate`, for the attempt loop (#530).

As built (#530): `cargo xtask agent-run --track ID --attempts N [--model M] [--max-usd X]
[--chunk-calls C]` makes a campaign folder under `$FN_AGENT_RUNS` (`DATE-campaign-ID`) with
`attempt-K/` run folders (each a `--scripts` run of the track's whole start, played to the
horizon, with `eval/seed-N.jsonl` and its own `handoff.json` beside the usual files),
`ledger.json` and `champion/` (`handoff.json`, `meta.json` and `scorecard.txt`, the champion's
clean run on `play_seed`). After each attempt it runs `fn_agent score --append`, evaluates the
final `governor` and plan, writes the champion if the train mean is strictly higher (a tie keeps
the champion), and only then appends the ledger row, so an attempt with no row is one a crash
cut short: it isn't counted and its folder is never reused. An attempt that never saved a
`governor` gets a row with dashes and isn't kept. The ledger row's plan revision counts plan
edits across the campaign (a preloaded plan isn't an edit); its governor version is the saved
script's own, which carries on from the champion's. `serve --carry-in FILE` (with `--scripts`,
a fresh session only) writes the handoff's scripts, notes and plan as log lines before the first
call, and `serve` gained `--map-size` and `--prelude` so a track with a site or a prelude is
played as it is evaluated. The carried-in governor is readable with `scripts {name}` (#560);
the first attempt of the campaign wrote a v21 from scratch because it couldn't read v16, so the
prompt asks for a small revision saved under the same name. The later attempts' first prompt is the script prompt
(`default_script_prompt`) under a block with the ledger's train columns, the champion's
scorecard and the one ask; held-out numbers appear only in `ledger.json` and the table printed
at the end. `--max-usd` is a per-attempt `--max-budget-usd` of what's left of the invocation's
cap, and the loop stops once it is spent. `agent-run --resume DIR [--attempts N]` reads the
ledger (which holds the track as it began), refuses a folder with no ledger or a tracks file
whose entry differs in anything but its label, and numbers new attempts after every row and
every `attempt-K` folder. `--tracks FILE` swaps the tracks file, for tests. Still to do: the
`attempt` and `champion` fields of the `score` line (the watcher issue).

### Fairness: lookahead and held-out seeds (#521)

Climbing on one seed teaches the map (where the water is, which outcrop is reachable) as much as
the game. Three layers keep that honest:

- **The model plays only `play_seed`.** It never sees another seed's map.
- **Selection is the mean over `train_seeds`.** A plan full of `play_seed` coordinates scores
  badly on seeds 7 and 42, so a governor that finds its own sites wins. That pushes the agent
  toward the porcelain the evidence run's SDK gaps asked for (find water, prefer plains).
- **`holdout_seeds` are never selected on and never shown to the agent.** The ledger shows them
  to the owner, and a growing gap between train and held-out means overfitting.

For races (#318), a racer may not carry a strategy climbed on its race's own seed. If scripted
races ever allow carried libraries, they rank in their own bracket, as #474 already does for
scripts.

### Cost

| | Round trips | Tokens | CPU |
|---|---|---|---|
| Attempt 1 (as the evidence run) | ~86 | ~$1 | |
| Later attempts (governor plays until surprised) | target 20–40 | ~$0.25–0.50 (Sonnet's scripted rate, agent-scripting.md) | |
| Clean evaluation, 6 seeds × 540 turns | 0 | $0 | ~2.5 min |
| A 5-attempt campaign | | ~$2–3 | ~13 min |

Clean evaluation runs one seed at a time, which matters on the owner's 8 GB laptop.

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

## Owner decisions

The owner took every recommendation on 2026-11-07.

1. **The score's shape and weights** (#519): 1000 × progress × care, as above. 14 milestones of
   weight 1, a late credit of 0.6, a partial credit of 0.5, deaths 0.15, close calls 0.03,
   suffering 2.0, at a 540-turn horizon.
2. **What counts as care** (#520): deaths, close calls (Dying, or the Avatar leaving unfed) and
   severe-need time. Mild flags, harm events and food security are reported but not scored.
   The tallies are kept on `World` by #533, shared with the balance harness.
3. **What the climb selects on** (#521): the mean clean score over three training seeds, with
   the model playing one of them and three held-out seeds shown to the owner only.
4. **How hard to push toward one strategy** (#522): the plan tool, plus read-only inline probes,
   with churn reported on the scorecard.
