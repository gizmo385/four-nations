//! A good's own screen (docs/design/factory-endgame.md, "The pipeline planner", point 3, #1029):
//! what makes it here now, what uses it, the gap, and every way to get more, each with how much
//! it adds a minute and what it costs: build another machine, buy from a caravan, or send
//! settlers to the deposits. Opened with `O` from the planner (the step under the pointer) or the
//! Goods tab (the good under the pointer). `Enter` on a machine to build arms the Build tool with
//! it.

use std::collections::BTreeMap;

use fn_sim::building::BuildingId;
use fn_sim::jobs::ItemId;

use crate::planner::{maker, per_machine, standing_rule, yard_rate};
use crate::rows::{Marks, Row, Tone, wrap};
use crate::session::Session;
use crate::state::Screen;

/// The open screen: the good, the way under the pointer, and the screen to go back to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GoodEdit {
    pub item: ItemId,
    pub cursor: usize,
    pub back: Screen,
}

/// One line of MAKING NOW or USING.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowLine {
    /// "2 Deep Mines".
    pub label: String,
    /// What they make or take a minute, all running.
    pub per_min: f32,
    /// "0.4 each, at the rock face".
    pub note: String,
}

/// What a way to get more is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WayKind {
    /// Build another of this machine; `Enter` arms the Build tool with it.
    Build(BuildingId),
    /// Buy it from a caravan.
    Caravan,
    /// Send settlers to gather it.
    Gather,
}

/// One way to get more.
#[derive(Clone, Debug, PartialEq)]
pub struct Way {
    pub kind: WayKind,
    /// "Build a Deep Mine".
    pub label: String,
    /// "0.4 a minute", "up to 0.4 a minute".
    pub adds: String,
    /// "20 stone, 4 metal, 2 tools, at the rock face".
    pub costs: String,
    /// The tech it still needs, if any: such a way can't be taken yet.
    pub locked: Option<String>,
}

/// The whole screen.
#[derive(Clone, Debug, PartialEq)]
pub struct GoodView {
    pub name: String,
    pub stores: u32,
    /// "Gathered from gold veins and gold seams, and made by Deep Mines."
    pub source: String,
    /// The rates measured over the last few minutes (`World::goods_rate`).
    pub made: f32,
    pub used: f32,
    pub making: Vec<FlowLine>,
    pub using: Vec<FlowLine>,
    pub ways: Vec<Way>,
    /// "3 Deep Mines close the gap." while it's short and a machine can be built for it.
    pub close: Option<String>,
}

fn rate(v: f32) -> String {
    format!("{v:.1}")
}

/// `n` of `name`, with a plain `s` past one ("2 Deep Mines").
fn counted(n: usize, name: &str) -> String {
    if n == 1 { format!("1 {name}") } else { format!("{n} {name}s") }
}

/// What one of `id`'s machines makes of its recipe a minute in this city, and its yard rule.
fn machine_each(session: &Session, id: BuildingId, recipe: &fn_sim::building::RecipeDef) -> f32 {
    let def = session.defs.building(id);
    let rate = yard_rate(session, def).map_or(1.0, |y| y.0);
    per_machine(session, recipe, rate)
}

/// The screen for `item`.
pub fn view(session: &Session, item: ItemId) -> GoodView {
    let (world, defs) = (&session.world, &session.defs);
    let nation = world.nation;
    let name = defs.item(item).name.clone();
    // Standing machines, grouped by building: those making it, those taking it.
    let mut makers: BTreeMap<BuildingId, (usize, f32)> = BTreeMap::new();
    let mut takers: BTreeMap<BuildingId, (usize, f32)> = BTreeMap::new();
    for b in world.buildings() {
        let Some(recipe) = world.recipe_at(b.anchor, defs) else { continue };
        let each = machine_each(session, b.id, recipe);
        let batches = each / recipe.output.count.max(1) as f32;
        if recipe.output.item == item {
            let e = makers.entry(b.id).or_default();
            e.0 += 1;
            e.1 = each;
        }
        if let Some(c) = recipe.inputs.iter().find(|c| c.item == item) {
            let e = takers.entry(b.id).or_default();
            e.0 += 1;
            e.1 = batches * c.count as f32;
        }
    }
    let line = |(id, (n, each)): (&BuildingId, &(usize, f32))| {
        let def = defs.building(*id);
        let mut note = format!("{} each", rate(*each));
        if let Some(rule) = standing_rule(def) {
            note.push_str(&format!(", {rule}"));
        }
        FlowLine { label: counted(*n, def.display_name(nation)), per_min: *n as f32 * each, note }
    };
    let making: Vec<FlowLine> = makers.iter().map(line).collect();
    let using: Vec<FlowLine> = takers.iter().map(line).collect();
    let (made, used) = world.goods_rate(item);

    let mut ways = Vec::new();
    // Build another machine: every building the nation has or may unlock that makes it.
    for (i, def) in defs.buildings.iter().enumerate() {
        if !crate::build::available_to_nation(session, def) {
            continue;
        }
        let Some(recipe) = def
            .recipe
            .iter()
            .chain(def.alt_recipes.iter().map(|a| &a.recipe))
            .find(|r| r.output.item == item)
        else {
            continue;
        };
        let id = BuildingId(i as u16);
        let mut costs: Vec<String> = def
            .cost
            .iter()
            .map(|c| format!("{} {}", c.count, defs.item(c.item).name.to_lowercase()))
            .collect();
        if let Some(rule) = standing_rule(def) {
            costs.push(rule.to_owned());
        }
        let locked = (!world.is_unlocked(def.unlocked_by))
            .then(|| def.unlocked_by.map(|t| defs.tech(t).name.clone()))
            .flatten();
        ways.push(Way {
            kind: WayKind::Build(id),
            label: format!("Build a {}", def.display_name(nation)),
            adds: format!("{} a minute", rate(machine_each(session, id, recipe))),
            costs: costs.join(", "),
            locked,
        });
    }
    // Buy it: each caravan that comes here with a deal for it, at most as often as it visits.
    let united = world.united();
    if let Some(schedule) = &defs.trade.schedule {
        let visitors: Vec<_> = schedule.visitors(nation, united).collect();
        let cycle = schedule.interval_minutes * visitors.len().max(1) as f32;
        for caravan in defs.trade.caravans_for(united) {
            if !visitors.contains(&caravan.nation) {
                continue;
            }
            for deal in &caravan.deals {
                let Some(&(_, count)) = deal.get.iter().find(|(i, _)| *i == item) else {
                    continue;
                };
                let give: Vec<String> = deal
                    .give
                    .iter()
                    .map(|g| {
                        let what = if g.any_food {
                            "food".to_owned()
                        } else {
                            g.items
                                .first()
                                .map(|&i| defs.item(i).name.to_lowercase())
                                .unwrap_or_default()
                        };
                        format!("{} {what}", g.count)
                    })
                    .collect();
                ways.push(Way {
                    kind: WayKind::Caravan,
                    label: format!("Buy from {} caravans", caravan.nation.title()),
                    adds: format!(
                        "up to {} a minute",
                        rate((count * deal.times) as f32 / cycle.max(1.0))
                    ),
                    costs: format!("{} a deal, {} a visit at most", give.join(", "), deal.times),
                    locked: None,
                });
            }
        }
    }
    // Gather it: each resource a harvest yields it from, with how many are on the map.
    let mut gathered = Vec::new();
    for h in defs.harvests.iter().filter(|h| h.item == item) {
        let res = defs.resource(h.resource);
        let on_map = world.map.iter().filter(|(_, t)| t.resource == Some(h.resource)).count();
        gathered.push(format!("{}s", res.name.to_lowercase()));
        ways.push(Way {
            kind: WayKind::Gather,
            label: format!("Send settlers to {}s", res.name.to_lowercase()),
            // A trip's yield, not a rate: most of a gatherer's time is the walk.
            adds: format!("{} a trip", h.count),
            costs: format!("a settler's time, {on_map} on the map"),
            locked: None,
        });
    }

    let made_by: Vec<String> = ways
        .iter()
        .filter_map(|w| match w.kind {
            WayKind::Build(id) => Some(format!("{}s", defs.building(id).display_name(nation))),
            _ => None,
        })
        .collect();
    let source = match (gathered.is_empty(), made_by.is_empty()) {
        (true, true) => "Nothing here makes it: it only comes by trade.".to_owned(),
        (false, true) => format!("Gathered from {}.", gathered.join(" and ")),
        (true, false) => format!("Made by {}.", made_by.join(" and ")),
        (false, false) => {
            format!(
                "Gathered from {}, and made by {}.",
                gathered.join(" and "),
                made_by.join(" and ")
            )
        }
    };

    // How many of the first buildable machine close the gap.
    let gap = used - made;
    let close = (gap > 0.05)
        .then(|| {
            ways.iter().find_map(|w| match (w.kind, &w.locked) {
                (WayKind::Build(id), None) => {
                    let recipe = maker(session, item).map(|m| m.2)?;
                    let each = machine_each(session, id, recipe);
                    let n = (gap / each.max(0.01)).ceil().max(1.0) as usize;
                    Some(format!(
                        "{} close the gap.",
                        counted(n, defs.building(id).display_name(nation))
                    ))
                }
                _ => None,
            })
        })
        .flatten();

    GoodView {
        name,
        stores: world.stored_total(item),
        source,
        made,
        used,
        making,
        using,
        ways,
        close,
    }
}

/// Moves the pointer by `dy` ways, clamped.
pub fn step(session: &Session, edit: &mut GoodEdit, dy: i32) {
    let n = view(session, edit.item).ways.len();
    if n == 0 {
        return;
    }
    edit.cursor = (edit.cursor as i32 + dy).clamp(0, n as i32 - 1) as usize;
}

/// The way under the pointer, if any.
pub fn chosen(session: &Session, edit: &GoodEdit) -> Option<Way> {
    view(session, edit.item).ways.into_iter().nth(edit.cursor)
}

/// The screen's text, `width` columns wide.
pub fn lines(session: &Session, edit: &GoodEdit, width: usize, marks: &Marks) -> Vec<Row> {
    let v = view(session, edit.item);
    let mut out =
        vec![Row::pair(v.name.clone(), Tone::Title, format!("{} in stores", v.stores), Tone::Dim)];
    for l in wrap(&v.source, width, 2, marks.ellipsis) {
        out.push(Row::text(l, Tone::Dim));
    }
    let label_w = v
        .making
        .iter()
        .chain(&v.using)
        .map(|l| l.label.chars().count())
        .chain(v.ways.iter().map(|w| w.label.chars().count()))
        .max()
        .unwrap_or(0)
        .max(14)
        + 4;
    let section = |out: &mut Vec<Row>, title: &str, total: f32, lines: &[FlowLine], none: &str| {
        out.push(Row::default());
        out.push(Row::spans(vec![
            (format!("{title:<label_w$}"), Tone::Heading),
            (format!("{} a minute, measured", rate(total)), Tone::Strong),
        ]));
        if lines.is_empty() {
            out.push(Row::text(format!("  {none}"), Tone::Dim));
        }
        for l in lines {
            out.push(Row::spans(vec![
                (format!("  {:<w$}", l.label, w = label_w - 2), Tone::Text),
                (format!("{:<20}", format!("{} a minute", rate(l.per_min))), Tone::Text),
                (l.note.clone(), Tone::Dim),
            ]));
        }
    };
    section(&mut out, "MAKING NOW", v.made, &v.making, "No machine here makes it.");
    section(&mut out, "USING", v.used, &v.using, "No machine here takes it.");
    out.push(Row::default());
    let gap = v.made - v.used;
    if gap < -0.05 {
        out.push(Row::text(format!("Short by {} a minute", rate(-gap)), Tone::Warn));
    } else if gap > 0.05 {
        out.push(Row::text(format!("{} a minute to spare", rate(gap)), Tone::Ok));
    } else {
        out.push(Row::text("Making about what is used", Tone::Dim));
    }
    out.push(Row::default());
    out.push(Row::spans(vec![
        (format!("{:<label_w$}", format!("MORE {}", v.name.to_uppercase())), Tone::Heading),
        (format!("{:<20}", "EACH ADDS"), Tone::Dim),
        ("COSTS".to_owned(), Tone::Dim),
    ]));
    if v.ways.is_empty() {
        out.push(Row::text("  No way to get more here yet.", Tone::Dim));
    }
    let cursor = edit.cursor.min(v.ways.len().saturating_sub(1));
    for (i, w) in v.ways.iter().enumerate() {
        let here = i == cursor;
        let costs = match &w.locked {
            Some(tech) => format!("after {tech}: {}", w.costs),
            None => w.costs.clone(),
        };
        out.push(Row::spans(vec![
            (if here { "> " } else { "  " }.to_owned(), Tone::Accent),
            (
                format!("{:<w$}", w.label, w = label_w - 2),
                if here { Tone::Strong } else { Tone::Text },
            ),
            (format!("{:<20}", w.adds), if w.locked.is_some() { Tone::Dim } else { Tone::Ok }),
            (costs, Tone::Dim),
        ]));
    }
    if let Some(close) = &v.close {
        out.push(Row::default());
        out.push(Row::text(format!("  {close}"), Tone::Text));
    }
    if let Some(w) = v.ways.get(cursor) {
        out.push(Row::default());
        let hint = match (w.kind, &w.locked) {
            (WayKind::Build(_), None) => "Enter: place one with the Build tool.",
            (WayKind::Build(_), Some(_)) => "Research it first, then Enter places one.",
            (WayKind::Caravan, _) => "Trade for it when that caravan camps (M).",
            (WayKind::Gather, _) => "Mark them with the Harvest tool (H).",
        };
        out.push(Row::text(hint, Tone::Dim));
    }
    out
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    fn republic(nation: Nation) -> Session {
        let mut s = Session::new(7, nation, 64, 64);
        let defs = s.defs.clone();
        s.world.start_republic(&defs);
        s
    }

    #[test]
    fn gold_ore_can_be_mined_bought_or_made_by_a_deep_mine() {
        let s = republic(Nation::Earth);
        let ore = s.defs.item_id("gold_ore").unwrap();
        let v = view(&s, ore);
        let kinds: Vec<WayKind> = v.ways.iter().map(|w| w.kind).collect();
        assert!(kinds.iter().any(|k| matches!(k, WayKind::Build(_))), "{:?}", v.ways);
        assert!(kinds.contains(&WayKind::Caravan), "{:?}", v.ways);
        assert!(kinds.contains(&WayKind::Gather), "{:?}", v.ways);
        let mine = v.ways.iter().find(|w| w.label == "Build a Deep Mine").expect("a Deep Mine");
        assert!(mine.costs.contains("at the rock face"), "{}", mine.costs);
        assert!(v.source.starts_with("Gathered from"), "{}", v.source);
    }

    #[test]
    fn a_standing_machine_shows_under_making_and_its_taker_under_using() {
        let mut s = republic(Nation::Fire);
        let home = s.world.units[0].pos;
        let defs = s.defs.clone();
        let foot = |dx, w, h| {
            fn_sim::geom::Footprint::new(fn_sim::Pos::new(home.x + dx, home.y + 6), w, h)
        };
        s.world.put_building(foot(6, 2, 2), defs.building_id("smelter").unwrap(), &defs);
        s.world.put_building(foot(9, 3, 2), defs.building_id("steelworks").unwrap(), &defs);
        let v = view(&s, defs.item_id("metal").unwrap());
        assert_eq!(v.making.len(), 1, "{:?}", v.making);
        assert_eq!(v.making[0].label, "1 Smelter");
        assert_eq!(v.using.len(), 1, "{:?}", v.using);
        assert_eq!(v.using[0].label, "1 Steelworks");
        let text: Vec<String> = lines(
            &s,
            &GoodEdit { item: defs.item_id("metal").unwrap(), cursor: 0, back: Screen::Planner },
            110,
            &Marks::new(true),
        )
        .iter()
        .map(Row::plain)
        .collect();
        assert!(text.iter().any(|t| t.starts_with("MAKING NOW")), "{text:?}");
        assert!(text.iter().any(|t| t.starts_with("MORE METAL")), "{text:?}");
        assert!(text.iter().any(|t| t.starts_with("> ")), "a pointer on the first way: {text:?}");
    }

    /// O on a step in the planner opens that good; Enter on a machine to build arms the Build
    /// tool with it; Esc goes back to the planner.
    #[test]
    fn o_in_the_planner_opens_the_good_and_enter_arms_the_build_tool() {
        use crate::app::App;
        use crate::keys::Action;
        use crate::tools::Tool;
        let s = republic(Nation::Earth);
        let ore = s.defs.item_id("gold_ore").unwrap();
        let mut app = App::new(s, (120, 40));
        app.ui.screen = Some(Screen::Planner);
        app.ui.planner = Some(crate::planner::PlanEdit::new(ore));
        app.act(Action::ToggleRepeat);
        assert_eq!(app.ui.screen, Some(Screen::Good));
        assert_eq!(app.ui.good.unwrap().item, ore);
        app.act(Action::Cancel);
        assert_eq!(app.ui.screen, Some(Screen::Planner), "back to the plan");
        app.act(Action::ToggleRepeat);
        let v = view(&app.session, ore);
        let at = v.ways.iter().position(|w| w.label == "Build a Deep Mine").unwrap();
        for _ in 0..at {
            app.act(Action::Move { dx: 0, dy: 1, tiles: 1 });
        }
        app.act(Action::Confirm);
        if v.ways[at].locked.is_none() {
            assert_eq!(app.ui.screen, None);
            assert_eq!(app.ui.tool, Tool::Build);
            assert_eq!(app.ui.choice, app.session.defs.building_id("deep_mine"));
        } else {
            assert_eq!(app.ui.screen, Some(Screen::Good), "a locked way stays on the screen");
            assert!(app.ui.notice.starts_with("Research"), "{}", app.ui.notice);
        }
    }
}
