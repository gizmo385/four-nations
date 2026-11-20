//! A building's own panel (#1068, Chris's playtest of 2026-11-16: "a modal for individual
//! buildings if you open them up"): `R` on a finished building that makes something, in Select
//! mode, or `Enter` on one with nobody inside. It lists every recipe the building has, with what
//! each takes, what it makes and how long a batch is, the one it's on marked and the locked ones
//! naming their tech; `Enter` sets the one under the pointer (`Command::SetRecipe`). Under the
//! list: who works it, the batch under way, the inputs and output in stores, and for a machine
//! where its output goes (`V` sets the split). `O` opens the good under the pointer's own screen
//! (`crate::good`), which comes back here. Available from the start, before the factory views.

use fn_sim::Pos;
use fn_sim::building::RecipeDef;
use fn_sim::jobs::{ItemId, JobKind};
use fn_sim::world::{Command, SplitTo};

use crate::inspect::plural;
use crate::rows::{Marks, Row, Tone, wrap};
use crate::session::Session;

/// The open panel: the building's anchor and the recipe under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingEdit {
    pub at: Pos,
    pub cursor: usize,
}

/// One recipe row.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeRow {
    /// `Command::SetRecipe`'s numbering: `None` the first recipe, `Some(i)` the `i`-th alt.
    pub choice: Option<u8>,
    pub output: ItemId,
    /// "1 tool".
    pub makes: String,
    /// "1 metal, 2 wood", or "nothing" for a source recipe.
    pub takes: String,
    /// "40s".
    pub time: String,
    /// The tech it still needs, if any.
    pub locked: Option<String>,
    /// The one the building is on.
    pub current: bool,
}

/// The whole panel.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingView {
    pub name: String,
    /// "2 of 2 crafters at work", "Automated", or "No crafter yet".
    pub workers: String,
    pub recipes: Vec<RecipeRow>,
    /// "Metal 1/1, Wood 0/2, work 40%", or "Not started".
    pub batch: String,
    /// "Metal 12, Wood 40".
    pub inputs_stored: String,
    /// "Tools 8, stops at 30".
    pub output_stored: String,
    /// Where its output goes: "Stores", or "Steelworks 2, Stores 1" on a split. `None` for a
    /// building whose output has nowhere to go but stores.
    pub sends: Option<String>,
    /// Extra state a crafter's or the inspector's lines would say (automation, idle).
    pub notes: Vec<String>,
}

/// "2 wood", "1 tool": a count and the good's name, as the good's screen words costs.
fn amount(session: &Session, item: ItemId, count: u32) -> String {
    format!("{count} {}", session.defs.item(item).name.to_lowercase())
}

/// A batch's time in this nation (its modifier counts), "40s" or "1m 20s".
fn batch_time(session: &Session, recipe: &RecipeDef) -> String {
    let speed =
        recipe.modifier.as_deref().and_then(|m| session.world.modifiers().get(m)).unwrap_or(1.0);
    let secs = (recipe.work / speed.max(0.01)).round() as u32;
    if secs >= 60 {
        let s = secs % 60;
        if s == 0 { format!("{}m", secs / 60) } else { format!("{}m {s}s", secs / 60) }
    } else {
        format!("{secs}s")
    }
}

/// Opens the panel on the finished building under `at`, or says why not: only a building with
/// a recipe has one.
pub fn open(session: &Session, at: Pos) -> Result<BuildingEdit, String> {
    let (world, defs) = (&session.world, &session.defs);
    let (Some(foot), Some(b)) = (world.footprint_at(at), world.building_at(at)) else {
        return Err("Nothing to open here".to_owned());
    };
    if defs.building(b).recipe.is_none() {
        let name = defs.building(b).display_name(world.nation);
        return Err(format!("The {name} makes nothing to set"));
    }
    let edit = BuildingEdit { at: foot.anchor, cursor: 0 };
    // The pointer starts on the recipe it's on.
    let cursor = view(session, &edit).recipes.iter().position(|r| r.current).unwrap_or(0);
    Ok(BuildingEdit { cursor, ..edit })
}

/// The panel for `edit`'s building. Empty lists if it has gone.
pub fn view(session: &Session, edit: &BuildingEdit) -> BuildingView {
    let (world, defs) = (&session.world, &session.defs);
    let at = edit.at;
    let Some(def) = world.building_at(at).map(|b| defs.building(b)) else {
        return BuildingView {
            name: String::new(),
            workers: String::new(),
            recipes: Vec::new(),
            batch: String::new(),
            inputs_stored: String::new(),
            output_stored: String::new(),
            sends: None,
            notes: Vec::new(),
        };
    };
    let current = world.recipe_choice_at(at, defs);
    let row = |choice: Option<u8>, recipe: &RecipeDef, locked: Option<String>| {
        let takes = if recipe.inputs.is_empty() {
            "nothing".to_owned()
        } else {
            let parts: Vec<String> =
                recipe.inputs.iter().map(|c| amount(session, c.item, c.count)).collect();
            parts.join(", ")
        };
        RecipeRow {
            choice,
            output: recipe.output.item,
            makes: amount(session, recipe.output.item, recipe.output.count),
            takes,
            time: batch_time(session, recipe),
            locked,
            current: current == choice,
        }
    };
    let mut recipes: Vec<RecipeRow> = def.recipe.iter().map(|r| row(None, r, None)).collect();
    for (i, alt) in def.alt_recipes.iter().enumerate() {
        let locked = alt
            .unlocked_by
            .filter(|&t| !world.research().is_known(t))
            .map(|t| defs.tech(t).name.clone());
        recipes.push(row(Some(i as u8), &alt.recipe, locked));
    }
    // Its crafters: the craft jobs it put up, and how many someone holds.
    let jobs: Vec<_> = world
        .jobs()
        .iter()
        .filter(|j| matches!(j.kind, JobKind::Craft { pos } if pos == at))
        .collect();
    let held = jobs.iter().filter(|j| j.claimed_by.is_some()).count();
    let workers = if world.automated(at, defs).is_some() {
        "Automated".to_owned()
    } else if jobs.is_empty() {
        "No crafter yet".to_owned()
    } else {
        let noun = if jobs.len() == 1 { "crafter" } else { "crafters" };
        format!("{held} of {} {noun} at work", jobs.len())
    };
    let recipe = world.recipe_at(at, defs);
    let batch = match world.craft_batch(at, defs) {
        Some((inputs, done)) => {
            let mut parts: Vec<String> = inputs
                .iter()
                .map(|&(item, got, need)| format!("{} {got}/{need}", defs.item(item).name))
                .collect();
            let started = done > 0.0 || inputs.iter().any(|&(_, got, _)| got > 0);
            if started {
                parts.push(format!("work {}%", (done * 100.0).round() as u32));
                parts.join(", ")
            } else {
                "Not started".to_owned()
            }
        }
        None => "Not started".to_owned(),
    };
    let inputs_stored = recipe.map_or_else(String::new, |r| {
        if r.inputs.is_empty() {
            return "Takes nothing".to_owned();
        }
        let parts: Vec<String> = r
            .inputs
            .iter()
            .map(|c| format!("{} {}", defs.item(c.item).name, world.stored_total(c.item)))
            .collect();
        parts.join(", ")
    });
    let output_stored = recipe.map_or_else(String::new, |r| {
        let n = world.stored_total(r.output.item);
        let name = plural(&defs.item(r.output.item).name);
        if r.stock_limit > 0 {
            format!("{name} {n}, stops at {}", r.stock_limit)
        } else {
            format!("{name} {n}")
        }
    });
    let split = world.split_of(at);
    let sends = if !split.is_empty() {
        let parts: Vec<String> = split
            .iter()
            .filter(|s| s.weight > 0)
            .map(|s| {
                let name = match s.to {
                    SplitTo::Stores => "Stores".to_owned(),
                    SplitTo::Machine(p) => crate::split::machine_name(session, p),
                };
                format!("{name} {}", s.weight)
            })
            .collect();
        Some(parts.join(", "))
    } else if world.split_destinations(at, defs).len() > 1 {
        Some("The first touching machine with room".to_owned())
    } else {
        None
    };
    let mut notes = Vec::new();
    notes.extend(crate::inspect::door_line(world, defs, at));
    notes.extend(crate::inspect::automation_line(world, defs, at));
    if world.craft_idle(at, defs)
        && let Some(r) = recipe
    {
        notes.push(format!(
            "Idle: {} {} in stores",
            r.stock_limit,
            plural(&defs.item(r.output.item).name)
        ));
    }
    BuildingView {
        name: def.display_name(world.nation).to_owned(),
        workers,
        recipes,
        batch,
        inputs_stored,
        output_stored,
        sends,
        notes,
    }
}

/// Moves the pointer by `dy` recipes, clamped.
pub fn step(session: &Session, edit: &mut BuildingEdit, dy: i32) {
    let n = view(session, edit).recipes.len();
    if n == 0 {
        return;
    }
    edit.cursor = (edit.cursor as i32 + dy).clamp(0, n as i32 - 1) as usize;
}

/// The recipe under the pointer, if any.
pub fn chosen(session: &Session, edit: &BuildingEdit) -> Option<RecipeRow> {
    view(session, edit).recipes.into_iter().nth(edit.cursor)
}

/// `Enter`: sets the recipe under the pointer. The outcome as a notice either way.
pub fn pick(session: &mut Session, edit: &BuildingEdit) -> Result<String, String> {
    let Some(row) = chosen(session, edit) else {
        return Err("Nothing to pick".to_owned());
    };
    let name = view(session, edit).name;
    let made = plural(&session.defs.item(row.output).name);
    if row.current {
        return Ok(format!("The {name} already makes {made}"));
    }
    session
        .apply(Command::SetRecipe { pos: edit.at, recipe: row.choice })
        .map(|()| format!("{name} now makes {made}"))
        .map_err(|e| format!("Can't switch recipe: {e}"))
}

/// The panel's text, `width` columns wide.
pub fn lines(session: &Session, edit: &BuildingEdit, width: usize, marks: &Marks) -> Vec<Row> {
    let v = view(session, edit);
    let mut out = vec![Row::pair(v.name.clone(), Tone::Title, v.workers.clone(), Tone::Dim)];
    let about = if v.recipes.len() > 1 {
        "It makes one recipe at a time. Switching drops the batch under way beside it."
    } else {
        "It has one recipe."
    };
    for l in wrap(about, width, 2, marks.ellipsis) {
        out.push(Row::text(l, Tone::Dim));
    }
    out.push(Row::default());
    let makes_w = v.recipes.iter().map(|r| r.makes.chars().count()).max().unwrap_or(0).max(8) + 12;
    let takes_w = v.recipes.iter().map(|r| r.takes.chars().count()).max().unwrap_or(0).max(6) + 3;
    out.push(Row::spans(vec![
        (format!("  {:<makes_w$}", "MAKES"), Tone::Heading),
        (format!("{:<takes_w$}", "TAKES"), Tone::Dim),
        ("A BATCH".to_owned(), Tone::Dim),
    ]));
    let cursor = edit.cursor.min(v.recipes.len().saturating_sub(1));
    for (i, r) in v.recipes.iter().enumerate() {
        let here = i == cursor;
        let label = if r.current { format!("{}  (making)", r.makes) } else { r.makes.clone() };
        let tone = match (&r.locked, here) {
            (Some(_), _) => Tone::Dim,
            (None, true) => Tone::Strong,
            (None, false) => Tone::Text,
        };
        let mut spans = vec![
            (if here { "> " } else { "  " }.to_owned(), Tone::Accent),
            (format!("{label:<makes_w$}"), if r.current { Tone::Ok } else { tone }),
            (format!("{:<takes_w$}", r.takes), tone),
            (r.time.clone(), Tone::Dim),
        ];
        if let Some(tech) = &r.locked {
            spans.push((format!("   needs {tech}"), Tone::Dim));
        }
        out.push(Row::spans(spans));
    }
    out.push(Row::default());
    let label = |s: &str| (format!("{s:<16}"), Tone::Heading);
    out.push(Row::spans(vec![label("THIS BATCH"), (v.batch.clone(), Tone::Text)]));
    out.push(Row::spans(vec![label("INPUTS STORED"), (v.inputs_stored.clone(), Tone::Text)]));
    out.push(Row::spans(vec![label("OUTPUT STORED"), (v.output_stored.clone(), Tone::Text)]));
    if let Some(sends) = &v.sends {
        out.push(Row::spans(vec![label("SENDS TO"), (sends.clone(), Tone::Text)]));
    }
    for n in &v.notes {
        out.push(Row::text(n.clone(), Tone::Dim));
    }
    out.push(Row::default());
    if let Some(r) = v.recipes.get(cursor) {
        let enter = match (&r.locked, r.current) {
            (Some(tech), _) => format!("Research {tech} to make it."),
            (None, true) => "Making this now.".to_owned(),
            (None, false) => format!("Enter: make {}.", plural(&session.defs.item(r.output).name)),
        };
        let good = format!("O: all about {}.", plural(&session.defs.item(r.output).name));
        let split = if v.sends.is_some() { "  V: where it goes." } else { "" };
        out.push(Row::text(format!("{enter}  {good}{split}"), Tone::Dim));
    }
    out
}

#[cfg(test)]
mod tests {
    use fn_sim::geom::Footprint;
    use fn_sim::{Nation, Pos};

    use super::*;
    use crate::app::App;
    use crate::keys::Action;
    use crate::state::Screen;

    /// Puts a building down on the free 2x2 spot nearest the first settler.
    fn put(s: &mut Session, id: &str) -> Pos {
        let id = s.defs.building_id(id).unwrap();
        let home = s.world.units[0].pos;
        let bed = s.defs.building_id("bed").unwrap();
        let at = s
            .world
            .map
            .iter()
            .map(|(p, _)| p)
            .filter(|&p| {
                [(0, 0), (1, 0), (0, 1), (1, 1)].iter().all(|&(dx, dy)| {
                    let q = Pos::new(p.x + dx, p.y + dy);
                    q != home && s.world.check_build(q, bed, &s.defs).is_ok()
                })
            })
            .min_by_key(|p| ((p.x - home.x).pow(2) + (p.y - home.y).pow(2), p.x, p.y))
            .expect("room near home");
        s.world.put_building(Footprint::new(at, 2, 2), id, &s.defs);
        at
    }

    #[test]
    fn a_workshop_lists_its_recipes_with_the_locked_one_naming_its_tech() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let at = put(&mut s, "workshop");
        let edit = open(&s, Pos::new(at.x + 1, at.y + 1)).expect("a workshop opens");
        assert_eq!(edit.at, at);
        let v = view(&s, &edit);
        assert_eq!(v.name, "Workshop");
        assert!(v.recipes.len() >= 2, "{:?}", v.recipes);
        assert!(v.recipes[0].current, "{:?}", v.recipes[0]);
        assert_eq!(v.recipes[0].makes, "1 tool");
        assert_eq!(v.recipes[1].locked.as_deref(), Some("Trade Wares"));
        let text: Vec<String> =
            lines(&s, &edit, 80, &Marks::new(true)).iter().map(Row::plain).collect();
        assert!(text.iter().any(|l| l.contains("(making)")), "{text:#?}");
        assert!(text.iter().any(|l| l.contains("needs Trade Wares")), "{text:#?}");
    }

    #[test]
    fn enter_on_an_unlocked_recipe_switches_and_a_locked_one_is_refused() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let at = put(&mut s, "workshop");
        let mut edit = open(&s, at).unwrap();
        step(&s, &mut edit, 1);
        assert_eq!(pick(&mut s, &edit), Err("Can't switch recipe: needs Trade Wares".to_owned()));
        let id = s.defs.building_id("workshop").unwrap();
        s.defs.buildings[id.0 as usize].alt_recipes[0].unlocked_by = None;
        assert_eq!(pick(&mut s, &edit), Ok("Workshop now makes Wares".to_owned()));
        assert_eq!(s.world.recipe_choice_at(at, &s.defs), Some(0));
        assert!(view(&s, &edit).recipes[1].current);
    }

    #[test]
    fn a_building_with_no_recipe_has_no_panel() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let at = put(&mut s, "bed");
        assert!(open(&s, at).is_err());
        assert!(open(&s, Pos::new(0, 0)).is_err());
    }

    #[test]
    fn r_opens_the_panel_enter_picks_and_o_goes_to_the_good_and_back() {
        let mut app = App::new(Session::new(7, Nation::Earth, 64, 64), (120, 40));
        let at = put(&mut app.session, "workshop");
        let id = app.session.defs.building_id("workshop").unwrap();
        app.session.defs.buildings[id.0 as usize].alt_recipes[0].unlocked_by = None;
        app.ui.cursor = at;
        app.act(Action::NextRecipe);
        assert_eq!(app.ui.screen, Some(Screen::Building));
        app.act(Action::Move { dx: 0, dy: 1, tiles: 1 });
        app.act(Action::Confirm);
        assert_eq!(app.session.world.recipe_choice_at(at, &app.session.defs), Some(0));
        assert_eq!(app.ui.notice, "Workshop now makes Wares");
        app.act(Action::ToggleRepeat);
        assert_eq!(app.ui.screen, Some(Screen::Good));
        app.act(Action::Cancel);
        assert_eq!(app.ui.screen, Some(Screen::Building));
        app.act(Action::Cancel);
        assert_eq!(app.ui.screen, None);
        assert_eq!(app.ui.building, None);
    }
}
