//! Designation tools: which command each tool issues for a tile, which key picks it, and the loop
//! that applies a tool to every tile of a rectangle. The frontends turn a drag (a mouse's, or two
//! `Enter`s) into a rectangle; this turns the rectangle into `Command`s and a notice.
//!
//! Keys: `H` harvest (chop, quarry, gather, mine, whatever each tile's resource gives), `X` cancel
//! harvest (also withdraws a build site), `P` stockpile, `B` the nation's bending ability, `U` the
//! build tool (pressed again it cycles through the buildings the nation can place) and `Esc` back
//! to selecting.

use fn_sim::bending::AbilityId;
use fn_sim::building::BuildingId;
use fn_sim::world::Command;
use fn_sim::{Defs, Nation, Pos};

use crate::build;
use crate::session::Session;

/// What the pointer (or the cursor's `Enter`) does on the map.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tool {
    /// Pick a unit.
    #[default]
    Select,
    Harvest,
    CancelHarvest,
    Stockpile,
    /// Channel the nation's signature ability (docs/design/bending.md).
    Bend,
    /// Place the building picked in the picker (#111).
    Build,
}

/// What a tool needs besides the tile to decide on a command.
#[derive(Clone, Copy, Default, Debug)]
pub struct Pick {
    /// The nation's bending ability, resolved once per drag (`ability_for_nation`); `Bend`
    /// issues nothing without one.
    pub ability: Option<AbilityId>,
    /// Held Shift (#81): a harvest designation becomes a standing order that re-queues itself
    /// every time the resource ripens again. No effect on any other tool.
    pub repeating: bool,
    /// The building to place; `Build` issues nothing without one.
    pub building: Option<BuildingId>,
}

impl Tool {
    /// Every tool, in the order the side panel lists them.
    pub const ALL: [Tool; 6] = [
        Self::Select,
        Self::Harvest,
        Self::CancelHarvest,
        Self::Stockpile,
        Self::Bend,
        Self::Build,
    ];

    /// The short name for a button.
    pub fn name(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Harvest => "Harvest",
            Self::CancelHarvest => "Cancel",
            Self::Stockpile => "Stockpile",
            Self::Bend => "Bend",
            Self::Build => "Build",
        }
    }

    /// The key that picks the tool, as shown to the player: "H", or "Esc" for `Select`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Select => "Esc",
            Self::Harvest => "H",
            Self::CancelHarvest => "X",
            Self::Stockpile => "P",
            Self::Bend => "B",
            Self::Build => "U",
        }
    }

    /// The letter key that picks the tool; `None` for `Select`, which is `Esc`. Always upper case.
    pub fn letter(self) -> Option<char> {
        match self {
            Self::Select => None,
            other => other.key().chars().next(),
        }
    }

    /// The tool a letter key picks, in either case.
    pub fn from_letter(c: char) -> Option<Self> {
        let c = c.to_ascii_uppercase();
        Self::ALL.into_iter().find(|t| t.letter() == Some(c))
    }

    /// What the tool does, in a sentence.
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "select units",
            Self::Harvest => "designate harvest (chop, mine, gather); hold Shift to repeat",
            Self::CancelHarvest => "cancel harvest or build site",
            Self::Stockpile => "designate stockpile",
            Self::Bend => "channel the nation's ability",
            Self::Build => "build: pick a building, then click a tile (U: next building)",
        }
    }

    /// `label`, except that `Bend` names the nation's one ability, since "channel the nation's
    /// ability" alone doesn't say which (docs/design/bending.md).
    pub fn hint(self, session: &Session) -> String {
        if self == Self::Bend
            && let Some(id) = ability_for_nation(&session.defs, session.world.nation)
        {
            let def = session.defs.ability(id);
            return format!("{}: {}", def.verb, def.description);
        }
        self.label().to_owned()
    }

    /// The command this tool issues for one tile, if it issues any. `site_here` says a build
    /// site is pending on the tile, which the cancel tool withdraws instead of a harvest.
    pub fn command(self, pos: Pos, site_here: bool, pick: Pick) -> Option<Command> {
        match self {
            Self::Select => None,
            Self::Harvest if pick.repeating => Some(Command::DesignateRepeatingHarvest { pos }),
            Self::Harvest => Some(Command::DesignateHarvest { pos }),
            Self::CancelHarvest if site_here => Some(Command::CancelBuild { pos }),
            Self::CancelHarvest => Some(Command::CancelHarvest { pos }),
            Self::Stockpile => Some(Command::DesignateStockpile { pos }),
            Self::Bend => Some(Command::Bend { ability: pick.ability?, pos }),
            Self::Build => Some(Command::DesignateBuild { pos, building: pick.building? }),
        }
    }
}

/// The nation's one bending ability right now (docs/design/bending.md: "There's one ability
/// now; a small picker appears once research unlocks more"). `None` only if a nation has no
/// ability defined at all, which the data tests guard against.
pub fn ability_for_nation(defs: &Defs, nation: Nation) -> Option<AbilityId> {
    defs.abilities.iter().position(|a| a.nation == nation).map(|i| AbilityId(i as u16))
}

/// Every tile in the rectangle spanned by two corners, row by row.
pub fn tiles_between(a: Pos, b: Pos) -> impl Iterator<Item = Pos> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| Pos::new(x, y)))
}

/// The tool and building choice after the player asks for `requested`. `Build` picks the first
/// building the nation can place when none is chosen (or the chosen one is locked); asking for
/// it again while it is active moves on to the next one. Every other tool leaves the choice
/// alone, so `Build` returns to the building last picked.
pub fn switch_tool(
    session: &Session,
    current: Tool,
    choice: Option<BuildingId>,
    requested: Tool,
) -> (Tool, Option<BuildingId>) {
    if requested != Tool::Build {
        return (requested, choice);
    }
    let rows = build::picker_rows(session);
    let choice = if current == Tool::Build {
        build::next_choice(&rows, choice)
    } else {
        build::valid_choice(&rows, choice)
    };
    (Tool::Build, choice)
}

/// Applies `tool` to every tile in the rectangle `from`..=`to` through `Session::apply` and
/// returns the notice to show, such as "designate harvest: 12 done, 3 skipped (nothing to
/// harvest)". `repeating` is the Shift state (#81); `choice` is the Build tool's building.
///
/// The sim decides what each tile accepts; a refusal is counted and its reason (the last one's)
/// shown, even for a one-tile click, so a rejected bend or harvest says why.
pub fn apply_rect(
    session: &mut Session,
    tool: Tool,
    from: Pos,
    to: Pos,
    repeating: bool,
    choice: Option<BuildingId>,
) -> String {
    if tool == Tool::Select {
        return String::new();
    }
    let rows = build::picker_rows(session);
    let building = build::valid_choice(&rows, choice);
    let pick = Pick {
        ability: ability_for_nation(&session.defs, session.world.nation),
        repeating,
        building,
    };
    let what = match (tool, building) {
        (Tool::Build, Some(id)) => {
            let name = rows.iter().find(|r| r.id == id).map_or("", |r| r.name.as_str());
            format!("build {name}")
        }
        _ => tool.label().to_owned(),
    };
    let (mut done, mut refused) = (0, 0);
    let mut last_error = None;
    for pos in tiles_between(from, to) {
        let site_here = session.world.build_site(pos).is_some();
        if let Some(command) = tool.command(pos, site_here, pick) {
            match session.apply(command) {
                Ok(()) => done += 1,
                Err(e) => {
                    refused += 1;
                    last_error = Some(e);
                }
            }
        }
    }
    match last_error {
        Some(e) => {
            let why = build::describe_error(session, e);
            format!("{what}: {done} done, {refused} skipped ({why})")
        }
        None => format!("{what}: {done} done, {refused} skipped"),
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::world::Command;

    use super::*;

    #[test]
    fn rectangle_covers_both_corners_whatever_the_drag_direction() {
        let a = Pos::new(5, 7);
        let b = Pos::new(3, 8);
        let tiles: Vec<Pos> = tiles_between(a, b).collect();
        assert_eq!(tiles.len(), 6);
        assert_eq!(tiles.first(), Some(&Pos::new(3, 7)));
        assert_eq!(tiles.last(), Some(&Pos::new(5, 8)));
        assert_eq!(tiles, tiles_between(b, a).collect::<Vec<_>>());
    }

    #[test]
    fn a_click_is_a_one_tile_rectangle() {
        let p = Pos::new(2, 2);
        assert_eq!(tiles_between(p, p).collect::<Vec<_>>(), [p]);
    }

    #[test]
    fn each_tool_issues_its_own_command() {
        let pos = Pos::new(1, 2);
        let none = Pick::default();
        assert_eq!(Tool::Select.command(pos, false, none), None);
        assert_eq!(
            Tool::Harvest.command(pos, false, none),
            Some(Command::DesignateHarvest { pos })
        );
        assert_eq!(
            Tool::CancelHarvest.command(pos, false, none),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(
            Tool::Stockpile.command(pos, false, none),
            Some(Command::DesignateStockpile { pos })
        );
        let ability = AbilityId(0);
        let bend = Pick { ability: Some(ability), ..none };
        assert_eq!(Tool::Bend.command(pos, false, bend), Some(Command::Bend { ability, pos }));
        let building = BuildingId(2);
        let build = Pick { building: Some(building), ..none };
        assert_eq!(
            Tool::Build.command(pos, false, build),
            Some(Command::DesignateBuild { pos, building })
        );
    }

    #[test]
    fn build_needs_a_chosen_building_to_issue_a_command() {
        assert_eq!(Tool::Build.command(Pos::new(0, 0), false, Pick::default()), None);
    }

    #[test]
    fn cancel_withdraws_a_build_site_where_there_is_one() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        assert_eq!(
            Tool::CancelHarvest.command(pos, true, none),
            Some(Command::CancelBuild { pos })
        );
        // Only the cancel tool does.
        assert_eq!(Tool::Harvest.command(pos, true, none), Some(Command::DesignateHarvest { pos }));
    }

    #[test]
    fn holding_shift_makes_a_harvest_designation_repeating() {
        let pos = Pos::new(1, 2);
        let shift = Pick { repeating: true, ..Pick::default() };
        assert_eq!(
            Tool::Harvest.command(pos, false, shift),
            Some(Command::DesignateRepeatingHarvest { pos })
        );
        // Shift only changes what the harvest tool does.
        assert_eq!(
            Tool::CancelHarvest.command(pos, false, shift),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(Tool::Select.command(pos, false, shift), None);
    }

    #[test]
    fn bend_needs_an_ability_to_issue_a_command() {
        assert_eq!(Tool::Bend.command(Pos::new(0, 0), false, Pick::default()), None);
    }

    #[test]
    fn ability_for_nation_finds_each_nation_s_one_signature_ability() {
        let defs = Defs::bundled();
        for nation in Nation::ALL {
            let id = ability_for_nation(&defs, nation).unwrap_or_else(|| panic!("{nation:?}"));
            assert_eq!(defs.ability(id).nation, nation);
        }
    }

    #[test]
    fn every_tool_has_a_key_and_the_letters_are_distinct() {
        assert_eq!(Tool::Select.key(), "Esc");
        assert_eq!(Tool::Select.letter(), None);
        let letters: Vec<char> = Tool::ALL.iter().filter_map(|t| t.letter()).collect();
        assert_eq!(letters, ['H', 'X', 'P', 'B', 'U']);
        for tool in Tool::ALL {
            if let Some(c) = tool.letter() {
                assert_eq!(Tool::from_letter(c), Some(tool));
                assert_eq!(Tool::from_letter(c.to_ascii_lowercase()), Some(tool));
                assert_eq!(tool.key(), c.to_string());
            }
        }
        assert_eq!(Tool::from_letter('Z'), None);
    }

    #[test]
    fn the_hint_names_the_nation_s_own_ability_for_bend() {
        let s = Session::new(1, Nation::Fire, 48, 32);
        let hint = Tool::Bend.hint(&s);
        let id = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        assert!(hint.starts_with(&s.defs.ability(id).verb), "{hint}");
        assert_eq!(Tool::Harvest.hint(&s), Tool::Harvest.label());
    }

    /// A free land tile near the first unit, away from every unit.
    fn free_tile(s: &Session) -> Pos {
        s.world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == fn_sim::Terrain::Land
                    && t.resource.is_none()
                    && !s.world.units.iter().any(|u| u.pos == *p)
            })
            .map(|(p, _)| p)
            .expect("a land tile")
    }

    #[test]
    fn the_build_key_picks_a_placeable_building_and_pressed_again_cycles() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let rows = build::picker_rows(&s);
        let open: Vec<BuildingId> = rows.iter().filter(|r| !r.locked()).map(|r| r.id).collect();
        assert!(open.len() >= 2);
        let (tool, choice) = switch_tool(&s, Tool::Select, None, Tool::Build);
        assert_eq!((tool, choice), (Tool::Build, Some(open[0])));
        let (tool, choice) = switch_tool(&s, tool, choice, Tool::Build);
        assert_eq!((tool, choice), (Tool::Build, Some(open[1])));
        // Another tool keeps the choice, so coming back to Build returns to it.
        let (tool, kept) = switch_tool(&s, tool, choice, Tool::Harvest);
        assert_eq!((tool, kept), (Tool::Harvest, choice));
        assert_eq!(switch_tool(&s, tool, kept, Tool::Build), (Tool::Build, choice));
    }

    #[test]
    fn applying_a_rectangle_counts_what_the_sim_accepted_and_says_why_for_the_rest() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let pos = free_tile(&s);
        let to = Pos::new(pos.x + 1, pos.y);
        let notice = apply_rect(&mut s, Tool::Stockpile, pos, to, false, None);
        assert!(notice.starts_with("designate stockpile: "), "{notice}");
        let done = s.world.stockpile_zones().count();
        assert!(done >= 1);
        assert!(notice.contains(&format!("{done} done, {} skipped", 2 - done)), "{notice}");

        // Off the map: all refused, with the sim's reason.
        let notice =
            apply_rect(&mut s, Tool::Harvest, Pos::new(-2, -2), Pos::new(-2, -2), false, None);
        assert!(
            notice.starts_with(
                "designate harvest (chop, mine, gather); hold Shift to repeat: 0 done, 1 skipped ("
            ),
            "{notice}"
        );
        assert!(notice.ends_with(')'), "{notice}");

        assert_eq!(apply_rect(&mut s, Tool::Select, pos, to, false, None), "");
    }

    #[test]
    fn the_build_tool_names_the_building_and_cancel_withdraws_the_site() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let bed = s.defs.building_id("bed").unwrap();
        let pos = free_tile(&s);
        let notice = apply_rect(&mut s, Tool::Build, pos, pos, false, Some(bed));
        assert_eq!(notice, "build Bed: 1 done, 0 skipped");
        assert!(s.world.build_site(pos).is_some());
        let notice = apply_rect(&mut s, Tool::CancelHarvest, pos, pos, false, None);
        assert!(notice.ends_with("1 done, 0 skipped"), "{notice}");
        assert!(s.world.build_site(pos).is_none());
    }

    #[test]
    fn a_locked_building_is_replaced_by_the_first_open_one() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let campfire = s.defs.building_id("campfire").unwrap();
        let pos = free_tile(&s);
        // Earth hasn't learned fire making: the locked choice falls back, so the tool still
        // builds something rather than failing on the tech gate.
        let notice = apply_rect(&mut s, Tool::Build, pos, pos, false, Some(campfire));
        assert!(notice.ends_with("1 done, 0 skipped"), "{notice}");
        assert_ne!(s.world.build_site(pos).map(|(b, _, _)| b), Some(campfire));
    }
}
