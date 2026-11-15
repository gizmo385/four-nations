//! The pipeline planner (docs/design/factory-endgame.md, "The pipeline planner", #1029): pick a
//! good and a rate, and the chain that makes it is laid out from left to right, raw goods first
//! and the goal last. Each step names its machine, how many it takes at that rate and how busy
//! they'd be, and how many stand now; each input says how much a minute moves and how it
//! travels: straight across where the two machines can touch, by haulers where they can't (a
//! mine must stand at the rock face), and from stores for a raw good. Pure arithmetic over the
//! recipes and the city's rates; it changes nothing.

use std::collections::BTreeMap;

use fn_sim::building::{BuildingDef, BuildingId, RecipeDef};
use fn_sim::defs::Terrain;
use fn_sim::jobs::ItemId;

use crate::rows::{Marks, Row, Tone, fit};
use crate::session::Session;

/// The rate a plan opens at, a minute, and the step `+` and `-` change it by.
pub const RATE_STEP: f32 = 0.5;
/// The most a plan can ask for, a minute.
pub const MAX_RATE: f32 = 20.0;
/// How deep a chain is followed before it stops, against a recipe loop in the data.
const MAX_DEPTH: usize = 10;

/// The open planner: the good, the rate asked for, and the step under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanEdit {
    pub goal: ItemId,
    pub per_min: f32,
    pub cursor: usize,
}

impl PlanEdit {
    pub fn new(goal: ItemId) -> Self {
        Self { goal, per_min: 1.0, cursor: usize::MAX }
    }
}

/// How an input reaches the step that takes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Travel {
    /// The two machines can stand touching: batches pass straight across, no hauling.
    Touching,
    /// They can't touch, and settlers carry it; `why` says what keeps them apart.
    Haulers { why: String },
    /// A raw good (wood, fish), carried from stores.
    Stores,
}

/// One input of a step.
#[derive(Clone, Debug, PartialEq)]
pub struct StepInput {
    pub item: ItemId,
    pub name: String,
    pub per_min: f32,
    /// The step that makes it, by index into `Plan::steps`, or `None` for a raw good.
    pub from: Option<usize>,
    pub travel: Travel,
}

/// One machine in the chain.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub item: ItemId,
    pub building: BuildingId,
    /// The machine's name for this nation.
    pub machine: String,
    /// What it makes, and how much of it the plan needs a minute.
    pub made: String,
    pub per_min: f32,
    /// How many machines that takes, and how busy they'd be (0 to 1).
    pub needed: u32,
    pub busy: f32,
    /// How many stand now set to make it.
    pub built: u32,
    /// The tech it still needs, if it isn't known yet.
    pub locked: Option<String>,
    pub inputs: Vec<StepInput>,
}

/// A raw good the chain takes from stores, with how much a minute in all and each step's part.
#[derive(Clone, Debug, PartialEq)]
pub struct BusGood {
    pub item: ItemId,
    pub name: String,
    pub per_min: f32,
    pub takers: Vec<(usize, f32)>,
}

/// The whole chain.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub goal: String,
    pub per_min: f32,
    /// "By machine, in the Lightning Station's yard" or "By hand".
    pub how: String,
    /// Raw goods' makers first, the goal's last.
    pub steps: Vec<Step>,
    pub bus: Vec<BusGood>,
}

/// The building and recipe the plan makes `item` with: one the nation can build now if any,
/// else one it may yet unlock (with the tech's name). `None` for a raw good.
fn maker(
    session: &Session,
    item: ItemId,
) -> Option<(BuildingId, &BuildingDef, &RecipeDef, Option<String>)> {
    let (world, defs) = (&session.world, &session.defs);
    let mut later = None;
    for (i, def) in defs.buildings.iter().enumerate() {
        if !crate::build::available_to_nation(session, def) {
            continue;
        }
        let recipe = def
            .recipe
            .iter()
            .chain(def.alt_recipes.iter().map(|a| &a.recipe))
            .find(|r| r.output.item == item);
        let Some(recipe) = recipe else { continue };
        let id = BuildingId(i as u16);
        if world.is_unlocked(def.unlocked_by) {
            return Some((id, def, recipe, None));
        }
        if later.is_none() {
            let tech = def.unlocked_by.map(|t| defs.tech(t).name.clone());
            later = Some((id, def, recipe, tech));
        }
    }
    later
}

/// The best automating yard rate `def` could run at in this city (a source whose tech is known
/// and that the nation builds), or `None` to run by hand. The yard's own `beside` rule (the
/// rock, the water) is the player's to meet when placing it.
fn yard_rate(session: &Session, def: &BuildingDef) -> Option<(f32, String)> {
    if !def.uses_power {
        return None;
    }
    let (world, defs) = (&session.world, &session.defs);
    defs.buildings
        .iter()
        .filter(|s| crate::build::available_to_nation(session, s))
        .filter_map(|s| {
            let (tech, a) = s.automate_upgrade.as_ref()?;
            world
                .research()
                .is_known(*tech)
                .then(|| (a.rate, s.display_name(world.nation).to_owned()))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
}

/// How much one machine making `recipe` turns out a minute, at `rate` times the plain speed
/// and the nation's modifier.
fn per_machine(session: &Session, recipe: &RecipeDef, rate: f32) -> f32 {
    let speed =
        recipe.modifier.as_deref().and_then(|m| session.world.modifiers().get(m)).unwrap_or(1.0);
    let secs = recipe.work.max(0.1) / (rate * speed).max(0.01);
    recipe.output.count as f32 * 60.0 / secs
}

/// Where a building must stand, in words, if it has a rule: "at the rock face", "by the
/// water".
fn standing_rule(def: &BuildingDef) -> Option<&'static str> {
    let beside = def.beside.as_ref()?;
    if beside.iter().any(|t| matches!(t, Terrain::Mountain | Terrain::Tunnel)) {
        Some("at the rock face")
    } else if beside.iter().any(|t| {
        matches!(
            t,
            Terrain::Water | Terrain::River | Terrain::Lake | Terrain::Coast | Terrain::Ford
        )
    }) {
        Some("by the water")
    } else {
        Some("where its ground is")
    }
}

/// Why two machines can't touch: "The Deep Mine must stand at the rock face, so it can't touch
/// the Smelter."
fn apart(fixed: &BuildingDef, rule: &str, other: &BuildingDef, nation: fn_sim::Nation) -> String {
    format!(
        "The {} must stand {rule}, so it can't touch the {}.",
        fixed.display_name(nation),
        other.display_name(nation)
    )
}

/// Adds `per_min` of `item` to what the plan needs, and what its inputs need, in turn.
/// `order` gets each made good once, after everything it takes, so makers come before takers.
fn demand(
    session: &Session,
    item: ItemId,
    per_min: f32,
    depth: usize,
    need: &mut BTreeMap<ItemId, f32>,
    order: &mut Vec<ItemId>,
) {
    *need.entry(item).or_default() += per_min;
    if depth >= MAX_DEPTH {
        return;
    }
    let Some((_, _, recipe, _)) = maker(session, item) else { return };
    let batches = per_min / recipe.output.count.max(1) as f32;
    for input in &recipe.inputs {
        demand(session, input.item, batches * input.count as f32, depth + 1, need, order);
    }
    order.retain(|&i| i != item);
    order.push(item);
}

/// The chain that makes `per_min` of `goal` a minute.
pub fn plan(session: &Session, goal: ItemId, per_min: f32) -> Plan {
    let (world, defs) = (&session.world, &session.defs);
    let mut need = BTreeMap::new();
    let mut order = Vec::new();
    demand(session, goal, per_min, 0, &mut need, &mut order);
    // A good whose maker comes later in `order` than a step taking it can only be a loop;
    // `order` puts every maker first otherwise.
    let index: BTreeMap<ItemId, usize> = order.iter().enumerate().map(|(i, &it)| (it, i)).collect();
    let mut how = "By hand".to_owned();
    let mut steps = Vec::new();
    for &item in &order {
        let Some((id, def, recipe, locked)) = maker(session, item) else { continue };
        let total = need[&item];
        let yard = yard_rate(session, def);
        if let Some((_, source)) = &yard {
            how = format!("By machine, in the {source}'s yard");
        }
        let rate = yard.as_ref().map_or(1.0, |y| y.0);
        let each = per_machine(session, recipe, rate);
        let needed = ((total / each) - 1e-4).ceil().max(1.0) as u32;
        let busy = (total / (needed as f32 * each)).min(1.0);
        let built = world
            .buildings()
            .filter(|b| {
                b.id == id && world.recipe_at(b.anchor, defs).is_some_and(|r| r.output.item == item)
            })
            .count() as u32;
        let batches = total / recipe.output.count.max(1) as f32;
        let inputs = recipe
            .inputs
            .iter()
            .map(|c| {
                let from = index.get(&c.item).copied();
                let travel = match from.and_then(|_| maker(session, c.item)) {
                    None => Travel::Stores,
                    Some((_, src, _, _)) => {
                        let (theirs, ours) = (standing_rule(src), standing_rule(def));
                        match (theirs, ours) {
                            _ if theirs == ours => Travel::Touching,
                            (Some(rule), _) => {
                                Travel::Haulers { why: apart(src, rule, def, world.nation) }
                            }
                            (None, Some(rule)) => {
                                Travel::Haulers { why: apart(def, rule, src, world.nation) }
                            }
                            (None, None) => Travel::Touching,
                        }
                    }
                };
                StepInput {
                    item: c.item,
                    name: defs.item(c.item).name.clone(),
                    per_min: batches * c.count as f32,
                    from,
                    travel,
                }
            })
            .collect();
        steps.push(Step {
            item,
            building: id,
            machine: def.display_name(world.nation).to_owned(),
            made: defs.item(item).name.clone(),
            per_min: total,
            needed,
            busy,
            built,
            locked,
            inputs,
        });
    }
    // `from` indexes `order`; steps skip raw goods, so map it onto `steps`.
    let step_of: BTreeMap<ItemId, usize> =
        steps.iter().enumerate().map(|(i, s)| (s.item, i)).collect();
    for s in &mut steps {
        for input in &mut s.inputs {
            input.from = step_of.get(&input.item).copied();
            if input.from.is_none() {
                input.travel = Travel::Stores;
            }
        }
    }
    let mut bus: Vec<BusGood> = Vec::new();
    for (i, s) in steps.iter().enumerate() {
        for input in s.inputs.iter().filter(|i| i.from.is_none()) {
            match bus.iter_mut().find(|b| b.item == input.item) {
                Some(b) => {
                    b.per_min += input.per_min;
                    b.takers.push((i, input.per_min));
                }
                None => bus.push(BusGood {
                    item: input.item,
                    name: input.name.clone(),
                    per_min: input.per_min,
                    takers: vec![(i, input.per_min)],
                }),
            }
        }
    }
    Plan { goal: defs.item(goal).name.clone(), per_min, how, steps, bus }
}

/// Moves the pointer by `dx` steps, clamped. A fresh plan's pointer is on its goal.
pub fn step(session: &Session, edit: &mut PlanEdit, dx: i32) {
    let n = plan(session, edit.goal, edit.per_min).steps.len();
    if n == 0 {
        return;
    }
    let at = edit.cursor.min(n - 1) as i32;
    edit.cursor = (at + dx).clamp(0, n as i32 - 1) as usize;
}

/// Changes the rate by `delta` steps of [`RATE_STEP`], between one step and [`MAX_RATE`].
pub fn adjust(edit: &mut PlanEdit, delta: i32) {
    edit.per_min = (edit.per_min + delta as f32 * RATE_STEP).clamp(RATE_STEP, MAX_RATE);
}

fn rate(v: f32) -> String {
    format!("{v:.1}")
}

/// The screen's text, `width` columns wide: the goal and rate, the chain as a row of boxes from
/// left to right (a list, one step a row, where they don't fit), the raw goods from stores, and
/// the step under the pointer: what it takes, from where, and why each input travels as it does.
pub fn lines(session: &Session, edit: &PlanEdit, width: usize, marks: &Marks) -> Vec<Row> {
    let p = plan(session, edit.goal, edit.per_min);
    let mut out = vec![Row::pair(
        format!("{}, {} a minute", p.goal, rate(p.per_min)),
        Tone::Title,
        p.how.clone(),
        Tone::Dim,
    )];
    if p.steps.is_empty() {
        out.push(Row::default());
        out.push(Row::text(
            format!("Nothing here makes {}: it's gathered, not made.", p.goal.to_lowercase()),
            Tone::Dim,
        ));
        return out;
    }
    let cursor = edit.cursor.min(p.steps.len() - 1);
    out.push(Row::default());
    let n = p.steps.len();
    let col = (width / n).min(26);
    if col >= 14 {
        chain_columns(&p, cursor, col, marks, &mut out);
    } else {
        chain_list(&p, cursor, &mut out);
    }
    if !p.bus.is_empty() {
        out.push(Row::default());
        for b in &p.bus {
            let parts: Vec<String> = b
                .takers
                .iter()
                .map(|&(i, v)| format!("{} {}", p.steps[i].machine, rate(v)))
                .collect();
            out.push(Row::spans(vec![
                (format!("{} {} a min", b.name, rate(b.per_min)), Tone::Strong),
                (format!("  by haul from stores: {}", parts.join(", ")), Tone::Dim),
            ]));
        }
    }
    out.push(Row::default());
    detail(&p, cursor, width, marks, &mut out);
    if let Some(s) = p.steps.iter().find(|s| s.built < s.needed) {
        out.push(Row::default());
        let what = if s.locked.is_some() { "needed, not known yet" } else { "needed" };
        out.push(Row::spans(vec![(
            format!("{}: {} {what}, {} built.", s.machine, s.needed, s.built),
            Tone::Warn,
        )]));
    }
    out
}

/// The chain as columns `col` wide, one a step: the amount it sends on above its arrow, its
/// machine, the count and how busy, and how the output travels on below.
fn chain_columns(p: &Plan, cursor: usize, col: usize, marks: &Marks, out: &mut Vec<Row>) {
    let n = p.steps.len();
    // The name takes most of the column; the arrow to the next step the rest, then a space
    // for that step's pointer.
    let name_w = (col * 3 / 5).max(10).min(col - 3);
    let mut above = Vec::new();
    let mut boxes = Vec::new();
    let mut counts = Vec::new();
    let mut below = Vec::new();
    for (i, s) in p.steps.iter().enumerate() {
        let tone = if i == cursor { Tone::Accent } else { Tone::Strong };
        let pointer = if i == cursor { ">" } else { " " };
        let last = i + 1 == n;
        // Where this step's output goes: the next step that takes it, or the goal.
        let (amount, travel) = match p.steps.iter().skip(i + 1).find_map(|t| {
            t.inputs.iter().find(|inp| inp.from == Some(i)).map(|inp| (inp.per_min, &inp.travel))
        }) {
            Some((v, t)) => (v, Some(t)),
            None => (s.per_min, None),
        };
        above.push((
            fit(&format!("  {} {}", rate(amount), s.made.to_lowercase()), col, marks.ellipsis),
            Tone::Dim,
        ));
        boxes
            .push((format!("{pointer}{:<name_w$}", fit(&s.machine, name_w, marks.ellipsis)), tone));
        let arrow: String = if last {
            String::new()
        } else {
            format!("{}> ", marks.h.to_string().repeat(col.saturating_sub(name_w + 3).max(1)))
        };
        boxes.push((arrow, Tone::Dim));
        let count = match &s.locked {
            Some(tech) => format!(" needs {tech}"),
            None => format!(" {} at {:.0}%, {} built", s.needed, s.busy * 100.0, s.built),
        };
        let count_tone =
            if s.locked.is_some() || s.built < s.needed { Tone::Warn } else { Tone::Text };
        counts.push((format!("{:<col$}", fit(&count, col, marks.ellipsis)), count_tone));
        let (word, wt) = match travel {
            Some(Travel::Touching) => ("touching", Tone::Ok),
            Some(Travel::Haulers { .. }) => ("haulers", Tone::Warn),
            Some(Travel::Stores) => ("from stores", Tone::Dim),
            None => ("goal", Tone::Accent),
        };
        below.push((format!("{:<col$}", format!(" {word}")), wt));
    }
    let pad = |spans: Vec<(String, Tone)>| Row::spans(spans);
    out.push(pad(above.into_iter().map(|(t, tone)| (format!("{t:<col$}"), tone)).collect()));
    out.push(pad(boxes));
    out.push(pad(counts));
    out.push(pad(below));
}

/// The chain as a list, one step a row, where the columns won't fit.
fn chain_list(p: &Plan, cursor: usize, out: &mut Vec<Row>) {
    for (i, s) in p.steps.iter().enumerate() {
        let pointer = if i == cursor { "> " } else { "  " };
        out.push(Row::spans(vec![
            (pointer.to_owned(), Tone::Accent),
            (s.machine.clone(), if i == cursor { Tone::Strong } else { Tone::Text }),
            (format!("  {} {}", s.needed, rate(s.per_min)), Tone::Dim),
            (format!(" {} a min", s.made.to_lowercase()), Tone::Dim),
        ]));
    }
}

/// The step under the pointer: its machine, how many it takes and stand, what it makes, and
/// each input with where it comes from and how it travels.
fn detail(p: &Plan, cursor: usize, width: usize, marks: &Marks, out: &mut Vec<Row>) {
    let s = &p.steps[cursor];
    out.push(Row::pair(
        s.machine.to_uppercase(),
        Tone::Title,
        format!("{} {} a minute", rate(s.per_min), s.made.to_lowercase()),
        Tone::Strong,
    ));
    let have = match &s.locked {
        Some(tech) => format!("{} needed. Research {tech} first.", s.needed),
        None => format!(
            "{} needed, {} built, each {:.0}% busy at this rate.",
            s.needed,
            s.built,
            s.busy * 100.0
        ),
    };
    out.push(Row::text(have, Tone::Text));
    if s.inputs.is_empty() {
        out.push(Row::text("Takes nothing: it works the ground it stands on.", Tone::Dim));
    }
    for input in &s.inputs {
        let from = match input.from {
            Some(j) => format!("from the {}", p.steps[j].machine),
            None => "from stores".to_owned(),
        };
        let how = match &input.travel {
            Travel::Touching => "touching, so no hauling".to_owned(),
            Travel::Haulers { why } => format!("by haulers. {why}"),
            Travel::Stores => "by haulers".to_owned(),
        };
        let text = format!(
            "Takes {} {} a minute {from}, {how}",
            rate(input.per_min),
            input.name.to_lowercase()
        );
        for line in crate::rows::wrap(&text, width.saturating_sub(2), 3, marks.ellipsis) {
            out.push(Row::text(format!("  {line}"), Tone::Dim));
        }
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    fn item(s: &Session, id: &str) -> ItemId {
        s.defs.item_id(id).unwrap()
    }

    fn republic(nation: Nation) -> Session {
        let mut s = Session::new(7, nation, 64, 64);
        let defs = s.defs.clone();
        s.world.start_republic(&defs);
        s
    }

    #[test]
    fn steel_takes_metal_from_a_smelter_and_ore_from_a_deep_mine_by_haulers() {
        let s = republic(Nation::Fire);
        let p = plan(&s, item(&s, "steel"), 1.0);
        let names: Vec<&str> = p.steps.iter().map(|s| s.machine.as_str()).collect();
        let last = p.steps.last().unwrap();
        assert_eq!(last.item, item(&s, "steel"), "the goal is last: {names:?}");
        let metal = last.inputs.iter().find(|i| i.item == item(&s, "metal")).expect("takes metal");
        let smelter = &p.steps[metal.from.expect("metal is made")];
        assert_eq!(smelter.item, item(&s, "metal"));
        // Every maker comes before what takes from it.
        for (i, st) in p.steps.iter().enumerate() {
            for inp in &st.inputs {
                if let Some(j) = inp.from {
                    assert!(j < i, "{} before {}", p.steps[j].machine, st.machine);
                }
            }
        }
        // Wood is raw: it rides the bus from stores.
        assert!(p.bus.iter().any(|b| b.item == item(&s, "wood")), "{:?}", p.bus);
    }

    #[test]
    fn twice_the_rate_needs_twice_the_input_and_the_counts_round_up() {
        let s = republic(Nation::Fire);
        let one = plan(&s, item(&s, "steel"), 1.0);
        let two = plan(&s, item(&s, "steel"), 2.0);
        let metal = |p: &Plan| p.steps.iter().find(|s| s.made == "Metal").map(|s| s.per_min);
        assert!((metal(&two).unwrap() - 2.0 * metal(&one).unwrap()).abs() < 1e-4);
        for st in &two.steps {
            assert!(st.needed >= 1 && st.busy > 0.0 && st.busy <= 1.0, "{st:?}");
        }
    }

    #[test]
    fn a_mine_at_the_rock_face_reaches_the_smelter_by_haulers_and_says_why() {
        let s = republic(Nation::Fire);
        let p = plan(&s, item(&s, "metal"), 1.0);
        let smelter = p.steps.last().unwrap();
        let ore = smelter.inputs.iter().find(|i| i.item == item(&s, "gold_ore")).unwrap();
        if ore.from.is_some() {
            let Travel::Haulers { why } = &ore.travel else { panic!("{:?}", ore.travel) };
            assert!(why.contains("rock face"), "{why}");
        } else {
            // Fire has no Deep Mine of its own: ore is raw for it, from stores.
            assert_eq!(ore.travel, Travel::Stores);
        }
    }

    #[test]
    fn a_gathered_good_has_no_steps_and_says_so() {
        let s = republic(Nation::Fire);
        let wood = item(&s, "wood");
        let p = plan(&s, wood, 1.0);
        assert!(p.steps.is_empty());
        let text: Vec<String> = lines(&s, &PlanEdit::new(wood), 100, &Marks::new(true))
            .iter()
            .map(Row::plain)
            .collect();
        assert!(text.iter().any(|l| l.contains("gathered, not made")), "{text:?}");
    }

    #[test]
    fn the_screen_draws_the_chain_and_the_step_under_the_pointer() {
        let s = republic(Nation::Fire);
        let mut edit = PlanEdit::new(item(&s, "steel"));
        let text = |e: &PlanEdit, w| -> String {
            lines(&s, e, w, &Marks::new(true)).iter().map(Row::plain).collect::<Vec<_>>().join("\n")
        };
        let wide = text(&edit, 110);
        assert!(wide.starts_with("Steel, 1.0 a minute"), "{wide}");
        assert!(wide.contains("STEELWORKS"), "the goal's step is under a fresh pointer: {wide}");
        step(&s, &mut edit, -1);
        assert!(!text(&edit, 110).contains("STEELWORKS\n"), "the pointer moved");
        adjust(&mut edit, 2);
        assert_eq!(edit.per_min, 2.0);
        adjust(&mut edit, -100);
        assert_eq!(edit.per_min, RATE_STEP);
        // Narrow: a list instead of columns, still with every machine.
        let narrow = text(&edit, 40);
        assert!(narrow.contains("Steelworks"), "{narrow}");
    }

    /// G, Tab, a pointer down the Goods tab and Enter open the planner on that good; the
    /// arrows step and change the rate, and Esc goes back to the Goods tab.
    #[test]
    fn enter_on_a_good_opens_the_planner_and_esc_goes_back() {
        use crate::app::App;
        use crate::keys::Action;
        use crate::state::Screen;
        let mut app = App::new(republic(Nation::Fire), (120, 40));
        app.act(Action::ShowEndeavor);
        app.act(Action::NextUnit);
        let ledger = crate::endeavor::goods_ledger(&app.session.world, &app.session.defs);
        app.act(Action::Move { dx: 0, dy: 1, tiles: 1 });
        app.act(Action::Confirm);
        assert_eq!(app.ui.screen, Some(Screen::Planner));
        let edit = app.ui.planner.expect("open");
        assert_eq!(edit.goal, ledger[1].item, "the good under the pointer");
        app.act(Action::Move { dx: 0, dy: -1, tiles: 1 });
        assert_eq!(app.ui.planner.unwrap().per_min, 1.0 + RATE_STEP);
        app.act(Action::Cancel);
        assert_eq!(app.ui.screen, Some(Screen::Endeavor));
        assert!(app.ui.planner.is_none());
    }
}
