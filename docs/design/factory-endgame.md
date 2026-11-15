# The Republic era: a factory endgame for the United City

Doc status: current

Status: **exploration** (owner's idea, 2026-11-15). Nothing here is decided and no issues are
filed yet. The owner's choices are under "Open questions for the owner". Once they are answered,
this doc becomes a proposal and its phases get design issues of their own, the way
research-horizons.md split into phase docs.

The owner's idea: an endgame expansion where the game "becomes more factory-esque", overlapping
the open power and steel work. This doc explores what that means for a colony sim run by
indirect control, how it grows out of systems that already ship, and how the four nations keep
playing differently once the game is about throughput instead of survival.

## Decisions in brief (proposed)

1. **The factory era starts at the Harmony Gate.** The United City is where every nation's
   people already live together, and where pacing.md left a "lighter loop" with no new system.
   Canon backs it: Aang's United Republic grows into Republic City, with lightning power plants,
   earthbender-pushed trains and a metalbending city at Zaofu. Nothing before the gate changes or
   becomes required.
2. **The game's question shifts from "are we alive?" to "are the lines running?"** The new
   pressure is throughput: items per minute through a chain, against standing orders from the
   four nations. People stay in the loop as operators, tenders and plant crews. They are never
   replaced.
3. **Four layers, each built on something that ships today:**
   - **Machines**: recipe buildings that run without a crafter. Fire's Factory (`great_works`,
     #725) already does this. Every nation gets its own version, with its own placement rule.
   - **Freight**: paired links that move goods without anyone carrying them. Chutes, Wind Gates,
     Balloon Docks and Sky Lifts are the pattern. Each nation gets its own kind.
   - **Power with a load**: a new machine tier that draws on a source's capacity. Old buildings
     keep the speed-up rule. Plants are crewed by benders, as Republic City's are.
   - **Goods**: two new intermediates, steel (#968) and fittings, so chains are three steps deep
     instead of one.
4. **A goal you supply, not a box you tick.** Standing contracts from the four nations ask for
   sustained rates ("6 steel a minute for 10 minutes"), and pay in draw, teachings and named
   newcomers. A capstone great work can follow.
5. **Industry pushes back, peacefully.** Machines and plants unsettle the spirits nearby. Unrest
   creeps over the land as spirit vines that slow work and block ground, and balance (groves,
   meditation, spirit water) pushes it back. This is the bridge to the Spirit Portal, the
   sequel endgame.md and pacing.md already name.
6. **Nations differ in how they make, move and power goods**, not only in how fast. Fire makes
   (dense yards, adjacency chains). Earth moves heavy loads far (freight rail) and extracts.
   Water runs everything along water (flumes, mills). Air moves light loads anywhere, fast, and
   powers the most machines.

## Why

What the late game has today, from the docs and data on `main`:

- **The United City is the same loop, wider** (pacing.md, section 4, as built in #442 and #761):
  caravans from all four nations at `united_scale` 1.5, requests that reward draw, and population
  milestones every 8 people forever. Its goal is a number going up.
- **Phase 6 gives the United City research** (six fusions, borrowed techs), but each fusion
  unlocks one building or bend. After about 6,000 points there is nothing left to ask for.
- **Power is a speed-up with a handful of consumers.** power.md chose that on purpose (question
  2: "Unpowered means slower, never broken") and said to revisit after the gate, "where a
  power-hungry building might make sense".
- **Steel has no purpose** (#968). The Steelworks is a better smelter that Earth's Metalbend
  glut idles (#958).
- **Production is one step deep.** Ore to metal to tools is the longest chain. Everything moves
  on someone's back, so a bigger colony means more haulers, not a better layout.
- **Fire's Factory already proves the fantasy.** `great_works` automates recipe buildings in a
  Lightning Station's yard, fed by Supply hauls (`crates/sim/src/world/automate.rs`). It is one
  school option for one nation.

A factory era answers all of these with systems the code already has.

## Principles

The seven rules in research-horizons.md still hold. Five more for this era:

1. **People stay the point.** Pillar 2 is indirect control. A machine runs on its own, but it
   wears and needs a tender. A plant needs a crew. Freight needs loading at one end. A colony of
   40 should be busier, not idle.
2. **Rates are readable.** Pillar 4 means every machine, link and stockpile can say what it
   makes and moves per minute, and what it is short of. A factory you can't read is a factory you
   can't fix.
3. **Unpowered is slow, overloaded is slow, nothing is broken.** Power.md's rule survives with
   one addition: a source shared by too many machines slows all of them (a brownout). No building
   ever stops because of power.
4. **Spend what is in glut, bottleneck on what is scarce.** Wood, food and labour pile up late.
   Ore, metal and tools are scarce. Chains should eat the first and be limited by the second.
5. **Reuse before invent.** Every layer starts as a generalisation of a shipped mechanic
   (`automate`, `chute`, `pair`, `fuel`, `[building.power]`, `stock_limit`, United requests).
   The new sim work is listed per phase.

## What already ships to build on

| Mechanic | Where | What the factory era does with it |
|---|---|---|
| Recipe buildings with inputs, output and `stock_limit` | `[building.recipe]`, #124, #350 | The machine. Gains a second and third step (steel, fittings) |
| A second recipe per building | `alt_recipe`, horizons-phase-3.md | Machines that switch product, so a line can be retooled |
| Power as reach and boost | `[building.power]`, #428 | Stays for every existing building. A machine tier adds `draw` |
| Automation in a yard, fed by Supply hauls | `automate` upgrade, #725 | Generalised: each nation gets an `automate` on its own source, with its own radius and rate |
| Fuel bunkers kept full by Refuel hauls | `[building.fuel]`, #675 | Plant fuel, and the tending bill (tools) for machines |
| One-way haul link | Chute (`omashu_chutes`, phase 1) | The template for freight links |
| Paired buildings | `pair` (Wind Gate, Balloon Dock, Sky Lift), #690 | Freight stations come in pairs |
| Tracks | Rail, Paved Road, Metal Rail | Earth's freight runs on them |
| Residents as research conditions | `requires.residents`, phase 6 | Plant crews and nation-specific machines want residents of a nation |
| Caravan requests that reward draw, steered by nation | #442, #761 | Grow into standing contracts measured as rates |
| Milestones every 8 people | `[milestone_repeat]`, #929 | Unchanged. Contracts become the real goal beside them |
| Harness targets and scorecards | `data/harness.toml`, `data/score.toml` | Throughput is easy to measure, which suits the agent tooling |

## Layer 1: machines

**Rule (proposed).** The `automate` upgrade (#725) gains siblings, one per nation, each on that
nation's own power source and each with its own shape. A finished recipe building in an
automating reach runs with no crafter at `rate` times plain speed, fed by Supply hauls, exactly
as the Factory does now. The new part is **wear**: every `wear_batches` batches the machine stops
and posts a **Tend** job that consumes one tool. A tender is any settler, and their nation's
modifier for the recipe sets how long tending takes.

| Nation | Machine style | Source and shape | Rate | Why it plays differently |
|---|---|---|---|---|
| Fire | **The yard** (ships as `great_works`) | Lightning Station, 4-tile yard | 1.0 | Densest and fastest. Adds the **chain rule** below |
| Earth | **The face** | Hot Spring Mill, along the rock | 0.6 | Earth automates extraction: Mine Shaft, Deep Mine and Crystal Grotto beside the rock run themselves. Many small mills, so a long face, never a yard |
| Water | **The race** | Tide Mill, and any mill on a flowing channel | 0.75 | Machines automate only along water (bank, canal, aqueduct). The layout is a line down a river, not a square |
| Air | **The loft** | Wind Turbine, anywhere | 0.5 | Slowest machines, but the widest reach, so Air automates the most buildings at once and moves goods fastest (layer 2) |

**Fire's chain rule.** In a Fire yard, a machine whose output is another machine's input, and
whose footprints touch, hands the batch across directly. No haul, no stockpile. Smelter touching
Steelworks touching Forge is a line that needs only ore and wood brought in. This is Fire's
"production density" made into a layout puzzle, and it keeps `great_works` the best automation in
the game.

**Fire's school stays special.** Other nations reach automation through a post-gate shared tech
(working name `mechanisms`, 1,000 points plus steel, needs the gate complete) whose `automate`
lands on whichever of that nation's sources it names. `great_works` stays a pre-gate school,
rate 1.0 against 0.5 to 0.75, and alone has the chain rule. A borrowed `great_works` (phase 6's
`teachable`) would let a United City with Fire residents build a Fire yard.

**Wear is the tool sink.** Tools are scarce and every machine eats them, so the Workshop and
Forge become the factory's heartbeat, and a player chooses how many machines they can keep fed.
Wear also keeps settlers in the loop without micromanagement. Whether machines should wear at all
is question 4.

## Layer 2: freight

Today every item moves on a settler's back. At 40 settlers and four chains that means most of
the colony hauls, and the layout question is only "put things near the stockpile". Freight is
what makes layout a factory question.

**Rule (proposed).** A **freight link** is a pair of stations, like a Wind Gate pair. The near
station has a small hold. Haulers drop goods into it (a short walk), and the link moves a stack
to the far station every `interval` seconds, where it spills as stacks for ordinary hauls, or,
if a machine touches the far station, straight into that machine's inputs. The hauling AI treats
a link as it treats a chute: one more edge in the route, taken when it is cheaper. Each link
reads `Moving 12 a minute` and `Waiting: hold empty`.

| Nation | Link | Placement rule | Throughput | Operator |
|---|---|---|---|---|
| Earth | **Freight line**: a cart on rail, pushed by earthbending (canon's Ba Sing Se train) | Both stations on one connected rail line, any length | Highest: a 20-stack every 30 s | A bender rides it. Without one, it runs at a third |
| Water | **Flume**: a water channel carrying crates | Downhill or downstream only, along river, canal or aqueduct | Medium, free running | None |
| Air | **Wind tube**: gliders and air currents | Straight line, up to 30 tiles, any terrain | Low (5-stacks) but fastest (every 10 s) | None, but only light goods: no stone, ore or metal |
| Fire | **Conveyor yard**: Fire has no long link | The chain rule inside the yard, and Balloon Docks (already shipped) for distance | n/a | n/a |

Fire's weakness at moving goods is the point: Fire builds a dense yard and brings ore to it,
Earth runs one long line from the mines, Water strings the factory down the river, and Air spreads
light industry over the whole map.

**The alternative is tile conveyors** (Factorio-style belts drawn tile by tile). They are the
purest factory fantasy, but they are a new spatial system, a new render problem for the glyph map,
and they make settlers irrelevant to logistics. Paired links reuse #690 and keep haulers at the
ends. This is question 2.

## Layer 3: power with a load

**Rule (proposed).** A new `draw` field on **machine-tier** buildings only (the factory era's
new buildings, plus any building while it is automated). A source gains `capacity`. Inside a
source's reach, if the machines' total draw exceeds its capacity, every machine it feeds slows by
`capacity / draw` (a brownout). Existing buildings and every pre-gate rule stay as power.md
decided: reach, boost, no stacking, unpowered is slow.

**Plants are crewed.** The factory era's sources are **plants** with seats, like a study hall.
A plant makes capacity only while crewed, and how much depends on who sits there:

| Plant | Crew | Capacity | Placement | Character |
|---|---|---|---|---|
| **Lightning Plant** (Fire) | Fire benders, adept or master (Republic City's power stations are exactly this) | Highest, scales with the crew's rank | Anywhere, with a fuel bunker for the crew's tea and rest (wood) | Big and central. Takes your best benders off bends |
| **Wind Farm** (Air) | None | Varies by season, highest in autumn and winter | Open ground, spaced apart | Free, but you plan around the calendar (`calendar`, phase 1) |
| **Hydro Mill** (Water) | One tender | Steady | On the bank or a canal, uses the Tide Mill's rule | Reliable, and pulls the factory to the water again |
| **Geothermal Vent** (Earth) | None | Small, but cheap and stackable | Touching rock, as the Hot Spring Mill | Breadth, as power.md set for Earth |

A United City can build any plant whose tech it has learned or borrowed, and crew it with the
right residents. Who lives in the city decides what power it can make, which is phase 6's
question carried into industry. Whether load exists at all is question 3.

## Layer 4: goods and chains

Two new items, kept deliberately few:

| Item | Made at | From (provisional) | Spent on |
|---|---|---|---|
| **Steel** | Steelworks | 2 metal and 2 wood | Machine-tier buildings, freight links, plants, contracts, and the phase-6 big builds (#968's favourite) |
| **Fittings** | Forge, second recipe | 1 steel and 1 wood, makes 2 | Tending (in place of a tool for machines), freight stations, contracts |

**Steel answers #968.** The Steelworks stops being a better smelter and becomes the one place
steel comes from. Its input is metal, so Earth's metalbent glut (#958) is finally worth
something: it feeds steel. Its `stock_limit` counts steel, not metal, so it runs. The Airship
Dock, Sky Lift and Lightning Station swap some metal for steel, as the #968 thread preferred. The
scripted opening's `united_steel_made` target then measures real production.

The resulting chains, three steps deep:

```
gold ore --Smelter/Metalbend--> metal --Steelworks--> steel --Forge--> fittings
   wood ---------------------------^------------^-----------^
stone --Kiln--> brick (plants, stations)
```

Existing items (wares, scroll copies, glow crystals, spirit water) stay as they are and become
contract goods, which gives them a standing buyer.

**#890 fits here.** Option A (the scroll-billed schools take scroll copies from the Archive) makes
those bills a production chain rather than a ruin search. That is the same direction as this
doc, and an automated Archive in the factory era makes scroll copies a real line. A is still the
right answer on its own merits.

## The goal: contracts, then a great work

**Standing contracts (proposed).** The United City's caravan requests grow into contracts, still
in `data/trade.toml`. A contract names a nation, a good, a rate and a duration: "The Fire Nation
asks for 6 steel a minute for 10 minutes." The rate is measured at a **Trade Depot** (a building
haulers and freight links deliver to). Meeting it pays draw steered to that nation (#761), a
teaching, or a named newcomer such as a master of that nation. Missing it costs nothing but the
reward. Contracts are data, so their size grows with each population milestone.

Contracts make the factory's question "can I sustain this rate?", which is the part of factory
games that is fun, and they reuse caravans, draw steering and milestones.

**A capstone great work (later).** A second endeavor that asks for sustained deliveries in
stages, like the Harmony Gate's four stages but measured in rates. Candidates, all canon and
peaceful: a **Grand Railway** linking the city to the four nations' capitals, a **Lightning
Grid** that powers the whole map, or a **City Tower**. This is optional. Contracts alone are an
endless loop, which may be what the owner wants. Question 5.

## Pressure: the spirits push back

**Rule (proposed, later phase).** Every machine and plant adds **unrest** to the tiles around it.
Unrest spreads slowly and fades slowly. Above a threshold, **spirit vines** grow on walkable
tiles: they slow movement, block building, and make settlers who work among them uneasy (a mood
hit if mood exists, otherwise slower work). Balance pushes it back: standing trees and orchards
absorb unrest, meditation and spirit seats (Air, the High Temple's aura) calm it, and spirit water
(Water) clears vines. Nothing dies from it, and nothing attacks.

That gives the factory era the pressure factory games get from pollution, without war, and it is
true to the setting: industry against the spirits is the whole of Korra's second season. It also
grows straight into the Spirit Portal, which endgame.md (concept B) and pacing.md (question 4,
#464) set aside as the sequel because it "needs spirit AI and a new resource". Unrest and vines
are that resource. Whether the factory should push back at all is question 6.

## How the four nations stay different

| | Fire | Earth | Water | Air |
|---|---|---|---|---|
| **Makes** | Densest yard, the chain rule, rate 1.0 | Automated extraction along the rock face | Machines strung along water | Slow machines, but the most of them |
| **Moves** | Short: chains inside the yard, balloons for distance | Freight line on rail, the heaviest loads | Flumes, downhill and downstream | Wind tubes, fastest, light goods only |
| **Powers** | Lightning Plant, crewed by its best benders | Many small geothermal vents | Hydro Mills, steady | Wind Farms, free but seasonal |
| **Weak at** | Moving goods any distance | Power (breadth, never depth) | Anything away from water | Heavy goods: stone, ore and metal walk |
| **United City lever** | Borrow a Freight line from Earth residents | Borrow a Lightning Plant from Fire residents | Borrow Wind Farms from Air residents | Borrow a Fire yard |

Each column is a different factory shape: a square, a line along rock, a line along water, and a
scatter. Borrowed techs (phase 6) let a United City mix them, at 2x and with residents, so the
mix is the late decision.

## How this fits the open power lane

Nothing in the lane needs to wait on this doc. Two issues lean on it.

| Issue | Effect |
|---|---|
| #447, #446, #448 (power track, Earth's source) | Unchanged. The Hot Spring Mill becomes Earth's automation source and the model for the Geothermal Vent |
| #449 (Steam Engine costs brick) | Unchanged |
| #450 (`spirit_oasis` widens Water's reach) | Unchanged. Water's reach rung carries over to its race |
| #451, #429 (POWER section, readouts) | Unchanged. Load adds one line later ("Load 14 of 20") |
| #432 (power balance) | Unchanged. Its targets are pre-gate |
| #433 (Steam Engine burns wood) | **Leans yes.** Running costs are the factory era's language. A fuelled engine is the first taste of a plant |
| #488 (Air's Windmill as an early source) | Unchanged |
| #413 (late tier pays off) | Unchanged. This doc picks up power.md's "revisit after the gate" |
| #835 (minute 50 to 80 gap) | **Not addressed.** That gap is before the gate, and this era starts after it |
| #968 (steel) | **Answered here, provisionally:** steel from metal at the Steelworks, spent on the machine tier and the phase-6 big builds |
| #890 (scroll-billed schools) | **A** (scroll copies) fits this direction, and is right on its own |

## Phases

Each phase ships something playable. The cheapest and most reused come first.

| Phase | Ships | New sim work | Waits on |
|---|---|---|---|
| F0. The lane | The open power issues, #968 steel, #890 | As filed | Nothing new |
| F1. Read the factory | A **Production** screen: each item's made and used per minute over the last few minutes, and each machine's status. Rates on the Here panel | Small: a rolling tally per item and building | Nothing. Worth doing even without the rest |
| F2. Machines for everyone | `mechanisms`, the per-nation `automate` siblings, wear and Tend, Fire's chain rule, fittings | Small to medium: `automate` already exists; shapes along water and rock; a Tend job | F0's steel |
| F3. Freight | Freight line, Flume, Wind tube | Medium: a link edge in hauling (the chute's), station holds, the Air weight rule | F1 for readouts |
| F4. Power with a load | `draw`, `capacity`, brownout, crewed plants | Medium: seats on a source, load summed per reach | Question 3 |
| F5. Contracts | The Trade Depot, rate-measured contracts in `trade.toml` | Small to medium: a rate window at a building, reward paths exist | F1 |
| F6. Spirits | Unrest, vines, balance answers, then the Spirit Portal's own design | Large | A United City playtest |

F1 and F5 together are already a factory game in miniature: the player can see rates and has
a reason to raise them, with the Factory and ordinary hauling as the only tools.

## Risks

- **Settlers become irrelevant.** Automation plus freight could leave a colony of 40 with nothing
  to do. Wear, crews and station loading are there to stop it. The harness's idle-hands measure
  (pacing.md) should check it per phase.
- **The glyph map gets unreadable.** Dense yards in 1-cell glyphs need a production overlay (rates
  and links), the way power has a reach tint. The egui window helps, but the terminal is still a
  frontend.
- **Session length is unknown.** No one has played the United City for long (#752). Contracts and
  phases should be tuned after that playtest, as phase 6's numbers are.
- **Fire's school loses its edge.** Mitigated by rate 1.0 and the chain rule staying Fire's.
- **Map room.** Small maps (192x128) may cramp Water's lines and Earth's freight. Medium may be
  the right default for this era. Measure first.
- **Determinism and cost.** Links and machines are per-tick state with no RNG, as Supply hauls
  are. Unrest spreading is a grid update and needs a budget.

## Open questions for the owner

1. **Where does the factory era start?** *Recommended: A.*
   - **A. After the Harmony Gate**, as the United City's loop. Canon fit (the Republic), every
     nation's people are present, and nothing pre-gate changes.
   - B. As a parallel track from the midgame, competing with the gate for resources.
   - C. A separate scenario or game mode that starts industrial.
2. **How do goods move?** *Recommended: A.*
   - **A. Paired freight links**, one kind per nation, built on the chute and Wind Gate pattern.
     Haulers still load and unload.
   - B. Tile conveyors drawn tile by tile. The purest factory feel, the most new code, and
     settlers stop mattering to logistics.
   - C. Settlers only, with better roads and stockpiles. Least new, least factory.
3. **Does power get a load?** *Recommended: A.*
   - **A. A machine tier with `draw` and crewed plants.** Overload slows, never stops. Old
     buildings keep power.md's speed-up rule.
   - B. Power stays a speed-up forever. Machines are just faster in reach.
   - C. A full grid for every building. Reverses power.md's question 2 and hurts Earth most.
4. **Who works the machines?** *Recommended: A.*
   - **A. Machines run themselves but wear**, and a settler tends them with a tool or fittings.
   - B. Fully automated: no wear, no tending.
   - C. Crafters still work every batch, and machines only run faster.
5. **What is the factory for?** *Recommended: A.*
   - **A. Standing contracts from the four nations, measured as rates**, then a capstone great
     work later.
   - B. One great work with staged deliveries, and nothing after it.
   - C. No goal beyond milestones: a sandbox.
6. **Does industry push back?** *Recommended: A, as the last phase.*
   - **A. Spirit unrest and vines**, answered by balance, leading into the Spirit Portal.
   - B. No pressure. The factory era is a pure builder.

Two answers in the open lane that this doc leans on: **#968**, steel made from metal at the
Steelworks and spent on the machine tier (proposed above), and **#890**, option A.

## Follow-up issues

None filed yet. Once the questions are answered: one design issue per phase (F1 to F6, F0 is
already filed), a comment on #968 with this doc's steel proposal, and a comment on #433 noting it
leans yes.
