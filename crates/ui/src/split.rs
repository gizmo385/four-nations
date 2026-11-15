//! The Split screen (`V` on a machine in Select mode, docs/design/factory-endgame.md, "How
//! machines connect"): where a machine's automated output goes. One row per destination, each
//! touching machine that takes the output and then stores, with its share; Left and Right
//! change the share under the pointer (`Command::SetShare`). Units are dealt out in proportion
//! to the shares. With no share set, batches go to the first touching machine with room, as
//! before the split existed.

use fn_sim::world::{Command, SplitTo};
use fn_sim::{Defs, Pos, World};

use crate::rows::{Row, Tone};
use crate::session::Session;

/// The highest share one destination can have.
pub const MAX_SHARE: u8 = 9;

/// The open Split screen: the machine it is for and the row under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitEdit {
    pub machine: Pos,
    pub cursor: usize,
}

/// One destination row.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitRow {
    pub to: SplitTo,
    /// "Steelworks (touching)" or "Stores".
    pub name: String,
    pub share: u8,
    /// Its part of the output, 0 to 100, while any share is set.
    pub percent: u32,
}

/// Opens the screen on the machine under `at`, or says why not.
pub fn open(session: &Session, at: Pos) -> Result<SplitEdit, String> {
    let (world, defs) = (&session.world, &session.defs);
    let Some(foot) = world.footprint_at(at) else {
        return Err("Can't split: no machine here".to_owned());
    };
    let Some(recipe) = world.recipe_at(foot.anchor, defs) else {
        return Err("Can't split: this building makes nothing".to_owned());
    };
    if world.split_destinations(foot.anchor, defs).len() < 2
        && world.split_of(foot.anchor).is_empty()
    {
        let made = &defs.item(recipe.output.item).name;
        return Err(format!("Can't split: no touching machine takes its {}", made.to_lowercase()));
    }
    Ok(SplitEdit { machine: foot.anchor, cursor: 0 })
}

/// The machine's name.
pub fn machine_name(session: &Session, machine: Pos) -> String {
    let (world, defs) = (&session.world, &session.defs);
    world
        .building_at(machine)
        .map(|b| defs.building(b).display_name(world.nation).to_owned())
        .unwrap_or_default()
}

/// The destination rows: every destination the machine can send to now, then any on its split
/// that no longer touches it (kept, so the share can be taken off).
pub fn rows(session: &Session, edit: &SplitEdit) -> Vec<SplitRow> {
    let world = &session.world;
    let split = world.split_of(edit.machine);
    let mut tos = world.split_destinations(edit.machine, &session.defs);
    for s in split {
        if !tos.contains(&s.to) {
            tos.push(s.to);
        }
    }
    let total: u32 = split.iter().map(|s| u32::from(s.weight)).sum();
    tos.into_iter()
        .map(|to| {
            let share = split.iter().find(|s| s.to == to).map_or(0, |s| s.weight);
            let name = match to {
                SplitTo::Stores => "Stores".to_owned(),
                SplitTo::Machine(p) => format!("{} (touching)", machine_name(session, p)),
            };
            let percent = (u32::from(share) * 100).checked_div(total).unwrap_or(0);
            SplitRow { to, name, share, percent }
        })
        .collect()
}

/// Moves the pointer by `dy` rows, clamped.
pub fn step(session: &Session, edit: &mut SplitEdit, dy: i32) {
    let n = rows(session, edit).len();
    if n == 0 {
        return;
    }
    edit.cursor = (edit.cursor as i32 + dy).clamp(0, n as i32 - 1) as usize;
}

/// Changes the share under the pointer by `delta`, between 0 and [`MAX_SHARE`].
pub fn adjust(session: &mut Session, edit: &SplitEdit, delta: i32) -> Result<(), String> {
    let Some(row) = rows(session, edit).into_iter().nth(edit.cursor) else { return Ok(()) };
    let weight = (i32::from(row.share) + delta).clamp(0, i32::from(MAX_SHARE)) as u8;
    if weight == row.share {
        return Ok(());
    }
    session
        .apply(Command::SetShare { pos: edit.machine, to: row.to, weight })
        .map_err(|e| format!("Can't split: {e}"))
}

/// The screen's text: the machine and what it makes, a line on how the split works now, then
/// one row per destination with a pointer, its share as pips and its part of the output.
pub fn lines(session: &Session, edit: &SplitEdit) -> Vec<Row> {
    let (world, defs) = (&session.world, &session.defs);
    let made = world
        .recipe_at(edit.machine, defs)
        .map(|r| defs.item(r.output.item).name.to_lowercase())
        .unwrap_or_default();
    let name = machine_name(session, edit.machine);
    let rows_ = rows(session, edit);
    let split = !world.split_of(edit.machine).is_empty();
    let mut out = vec![
        Row::text(format!("Where the {name}'s {made} goes"), Tone::Title),
        Row::text(
            if split {
                "Each batch is dealt out by these shares. A full machine passes its turn on."
            } else {
                "Not split: each batch goes to the first touching machine with room."
            },
            Tone::Dim,
        ),
        Row::default(),
    ];
    let width = rows_.iter().map(|r| r.name.chars().count()).max().unwrap_or(0);
    for (i, r) in rows_.iter().enumerate() {
        let pointer = if i == edit.cursor { "> " } else { "  " };
        let pips: String = (0..MAX_SHARE).map(|k| if k < r.share { '#' } else { '.' }).collect();
        let part = if split { format!("{:>3}%", r.percent) } else { String::new() };
        out.push(Row::spans(vec![
            (pointer.to_owned(), Tone::Accent),
            (
                format!("{:<width$}  ", r.name),
                if i == edit.cursor { Tone::Strong } else { Tone::Text },
            ),
            (format!("{pips} {}  ", r.share), Tone::Accent),
            (part, Tone::Dim),
        ]));
    }
    out
}

/// The Here panel's line for a machine that touches another taking its output, or has a split:
/// `Sends: Steelworks 2, stores 1 (V to split)` or `Sends: to the Workshop first (V to split)`.
pub fn sends_line(world: &World, defs: &Defs, machine: Pos) -> Option<String> {
    let name = |p: Pos| {
        world.building_at(p).map(|b| defs.building(b).display_name(world.nation).to_owned())
    };
    let split = world.split_of(machine);
    if !split.is_empty() {
        let parts: Vec<String> = split
            .iter()
            .map(|s| match s.to {
                SplitTo::Stores => format!("stores {}", s.weight),
                SplitTo::Machine(p) => format!("{} {}", name(p).unwrap_or_default(), s.weight),
            })
            .collect();
        return Some(format!("Sends: {} (V to split)", parts.join(", ")));
    }
    let item = world.recipe_at(machine, defs)?.output.item;
    let first = *world.touching_takers(machine, item, defs).first()?;
    Some(format!("Sends: to the {} first (V to split)", name(first)?))
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::geom::Footprint;

    use super::*;

    /// A session with a Smelter whose right edge touches a Steelworks, returning both anchors.
    fn row() -> (Session, Pos, Pos) {
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        let home = s.world.units[0].pos;
        let (smelter, works) = (Pos::new(home.x + 6, home.y + 6), Pos::new(home.x + 8, home.y + 6));
        let id = |s: &Session, b: &str| s.defs.building_id(b).unwrap();
        let (a, b) = (id(&s, "smelter"), id(&s, "steelworks"));
        let defs = s.defs.clone();
        s.world.put_building(Footprint::new(smelter, 2, 2), a, &defs);
        s.world.put_building(Footprint::new(works, 3, 2), b, &defs);
        (s, smelter, works)
    }

    #[test]
    fn the_screen_lists_the_touching_machine_and_stores_and_sets_their_shares() {
        let (mut s, smelter, _) = row();
        assert_eq!(
            sends_line(&s.world, &s.defs, smelter).as_deref(),
            Some("Sends: to the Steelworks first (V to split)")
        );
        let mut edit = open(&s, Pos::new(smelter.x + 1, smelter.y + 1)).expect("opens");
        let names: Vec<String> = rows(&s, &edit).into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["Steelworks (touching)", "Stores"]);
        // Steelworks 2, stores 1.
        adjust(&mut s, &edit, 2).unwrap();
        step(&s, &mut edit, 1);
        adjust(&mut s, &edit, 1).unwrap();
        let shares: Vec<(u8, u32)> = rows(&s, &edit).iter().map(|r| (r.share, r.percent)).collect();
        assert_eq!(shares, [(2, 66), (1, 33)]);
        assert_eq!(
            sends_line(&s.world, &s.defs, smelter).as_deref(),
            Some("Sends: Steelworks 2, stores 1 (V to split)")
        );
        let text: Vec<String> = lines(&s, &edit).iter().map(Row::plain).collect();
        assert!(text[0].starts_with("Where the Smelter's metal goes"), "{text:?}");
        assert!(text.iter().any(|t| t.starts_with("> Stores")), "{text:?}");
        // Shares stop at 0 and MAX_SHARE.
        adjust(&mut s, &edit, -5).unwrap();
        assert_eq!(rows(&s, &edit)[1].share, 0);
    }

    #[test]
    fn a_machine_nothing_touches_has_nothing_to_split() {
        let (s, _, works) = row();
        // The Steelworks' steel goes into nothing touching it.
        let why = open(&s, works).unwrap_err();
        assert_eq!(why, "Can't split: no touching machine takes its steel");
        assert_eq!(sends_line(&s.world, &s.defs, works), None);
    }
}
