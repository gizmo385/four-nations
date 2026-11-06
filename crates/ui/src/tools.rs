//! Designation tools: which command each tool issues for a tile, which key picks it, and the loop
//! that applies a tool to every tile of a rectangle. The frontends turn a drag (a mouse's, or two
//! `Enter`s) into a rectangle; this turns the rectangle into `Command`s and a notice.
//!
//! Keys: `H` harvest (chop, quarry, gather, mine, whatever each tile's resource gives), `X` cancel
//! harvest (also withdraws a build site, a stockpile tile, a Sow designation or a Clear mark),
//! `E` clear the ground (docs/design/clearing.md, #411/#426: uproots a plant or works a
//! resource's ordinary harvest, removing it for good), `P` stockpile, `B` the nation's bending
//! ability (pressed again it cycles through the abilities the settlement can use), `U` the build
//! tool (pressed again it cycles through the buildings the nation can place), `C` sow an empty
//! farm plot (issue #198; a ripe one is reaped on its own, with no tool) and `Esc` back to
//! selecting.

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
    /// Remove whatever resource stands on a tile, for good (docs/design/clearing.md, #411/#426,
    /// ground mode only; a buildings mode would be a second press of `E`, #342). `X` also
    /// withdraws a Clear mark, as it does any other order.
    Clear,
    Stockpile,
    /// Channel one of the nation's abilities (docs/design/bending.md): its signature one, or
    /// the next the settlement can use after `B` again (`Session::bend_choice`).
    Bend,
    /// Place the building picked in the picker (#111).
    Build,
    /// Sow an empty farm plot (issue #198). A ripe one is reaped on its own; there's no tool
    /// for that.
    Sow,
}

/// What a tool needs besides the tile to decide on a command.
#[derive(Clone, Copy, Default, Debug)]
pub struct Pick {
    /// The ability the Bend tool channels, resolved once per drag (`bend_ability`); `Bend`
    /// issues nothing without one.
    pub ability: Option<AbilityId>,
    /// Held Shift (#81): a harvest designation becomes a standing order that re-queues itself
    /// every time the resource ripens again. No effect on any other tool.
    pub repeating: bool,
    /// The building to place; `Build` issues nothing without one.
    pub building: Option<BuildingId>,
}

/// What the cancel tool can withdraw on a tile, other than a harvest designation.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Here {
    /// A build site is pending on the tile.
    pub site: bool,
    /// The tile is designated as stockpile.
    pub zone: bool,
    /// The tile has a Sow designation on it (#198).
    pub sowing: bool,
    /// The tile has a Clear mark on it (docs/design/clearing.md, #411/#426).
    pub clearing: bool,
}

impl Here {
    /// What is designated on `pos` right now.
    pub fn at(session: &Session, pos: Pos) -> Self {
        let world = &session.world;
        Self {
            site: world.build_site(pos).is_some(),
            zone: world.stockpile_zones().any(|z| z == pos),
            sowing: world.sow_designations().any(|z| z == pos),
            clearing: world.clear_designations().any(|z| z == pos),
        }
    }
}

impl Tool {
    /// Every tool, in the order the side panel lists them.
    pub const ALL: [Tool; 8] = [
        Self::Select,
        Self::Harvest,
        Self::CancelHarvest,
        Self::Clear,
        Self::Stockpile,
        Self::Bend,
        Self::Build,
        Self::Sow,
    ];

    /// The short name for a button.
    pub fn name(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Harvest => "Harvest",
            Self::CancelHarvest => "Cancel",
            Self::Clear => "Clear",
            Self::Stockpile => "Stockpile",
            Self::Bend => "Bend",
            Self::Build => "Build",
            Self::Sow => "Sow",
        }
    }

    /// The key that picks the tool, as shown to the player: "H", or "Esc" for `Select`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Select => "Esc",
            Self::Harvest => "H",
            Self::CancelHarvest => "X",
            Self::Clear => "E",
            Self::Stockpile => "P",
            Self::Bend => "B",
            Self::Build => "U",
            Self::Sow => "C",
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
            // The client that read held Shift as a standing order is gone (#281); the TUI's
            // own `O` key stands in for it (#277, `fn_tui::state::UiState::tool_hint`).
            Self::Harvest => "designate harvest (chop, mine, gather)",
            Self::CancelHarvest => "cancel harvest, build site, stockpile, sowing or clearing",
            Self::Clear => "clear the ground",
            Self::Stockpile => "designate stockpile",
            Self::Bend => "channel the nation's ability",
            Self::Build => "place the chosen building (U: next building)",
            Self::Sow => "sow an empty farm plot",
        }
    }

    /// A short clause naming what the tool does, for the TUI's active-tool banner (#307),
    /// without the verb its own name badge already says ("HARVEST" already says this designates
    /// a harvest) and without the stale Shift text #305 dropped from `label`. `""` where the
    /// name alone says enough (`Select`, `Clear`, `Stockpile`, `Bend`, `Build`, `Sow`): the
    /// banner's badge already names Clear's mode (`CLEAR ground`, `crate::banner` in `fn_tui`).
    pub fn brief(self) -> &'static str {
        match self {
            Self::Select | Self::Clear | Self::Stockpile | Self::Bend | Self::Build | Self::Sow => {
                ""
            }
            Self::Harvest => "chop, mine, gather",
            Self::CancelHarvest => "harvest, build site, stockpile, sowing or clearing",
        }
    }

    /// `label`, except that `Bend` names the ability it channels, since "channel the nation's
    /// ability" alone doesn't say which (docs/design/bending.md). With more than one usable it
    /// says how to move on to the next.
    pub fn hint(self, session: &Session) -> String {
        if self == Self::Bend
            && let Some(id) = bend_ability(session)
        {
            let def = session.defs.ability(id);
            let more = if usable_abilities(session).len() > 1 { " (B: next ability)" } else { "" };
            return format!("{}: {}{more}", def.verb, def.description);
        }
        self.label().to_owned()
    }

    /// The command this tool issues for one tile, if it issues any. `here` says what else is
    /// designated on the tile: the cancel tool withdraws a build site, else a stockpile tile,
    /// a Sow designation or a Clear mark, instead of a harvest.
    pub fn command(self, pos: Pos, here: Here, pick: Pick) -> Option<Command> {
        match self {
            Self::Select => None,
            Self::Harvest if pick.repeating => Some(Command::DesignateRepeatingHarvest { pos }),
            Self::Harvest => Some(Command::DesignateHarvest { pos }),
            Self::CancelHarvest if here.site => Some(Command::CancelBuild { pos }),
            Self::CancelHarvest if here.zone => Some(Command::CancelStockpile { pos }),
            Self::CancelHarvest if here.sowing => Some(Command::CancelSow { pos }),
            Self::CancelHarvest if here.clearing => Some(Command::CancelClear { pos }),
            Self::CancelHarvest => Some(Command::CancelHarvest { pos }),
            Self::Clear => Some(Command::DesignateClear { pos }),
            Self::Stockpile => Some(Command::DesignateStockpile { pos }),
            Self::Bend => Some(Command::Bend { ability: pick.ability?, pos }),
            Self::Build => Some(Command::DesignateBuild { pos, building: pick.building? }),
            Self::Sow => Some(Command::Sow { pos }),
        }
    }
}

/// The nation's signature ability: its first in the data, which needs no research. `None` only
/// if a nation has no ability defined at all, which the data tests guard against.
pub fn ability_for_nation(defs: &Defs, nation: Nation) -> Option<AbilityId> {
    defs.abilities.iter().position(|a| a.nation == nation).map(|i| AbilityId(i as u16))
}

/// Every ability this settlement can use now (its nation's, with their tech known), in data
/// order: the signature one first, then what research has unlocked (#123).
pub fn usable_abilities(session: &Session) -> Vec<AbilityId> {
    (0..session.defs.abilities.len() as u16)
        .map(AbilityId)
        .filter(|&id| session.world.check_ability(id, &session.defs).is_ok())
        .collect()
}

/// The ability the Bend tool channels: the one picked with `B` while it is still usable, else the
/// first usable (the signature ability).
pub fn bend_ability(session: &Session) -> Option<AbilityId> {
    let usable = usable_abilities(session);
    session
        .bend_choice
        .filter(|c| usable.contains(c))
        .or_else(|| usable.first().copied())
        .or_else(|| ability_for_nation(&session.defs, session.world.nation))
}

/// `B` pressed while the Bend tool is already active moves on to the next usable ability,
/// wrapping round. Any other key leaves the choice alone, so coming back to `B` returns to it.
pub fn cycle_ability(session: &mut Session, current: Tool, requested: Tool) {
    if current != Tool::Bend || requested != Tool::Bend {
        return;
    }
    let usable = usable_abilities(session);
    let now = bend_ability(session);
    let next = now.and_then(|n| usable.iter().position(|&a| a == n)).map_or(0, |i| i + 1);
    session.bend_choice = usable.get(next % usable.len().max(1)).copied();
}

/// Every tile in the rectangle spanned by two corners, row by row.
pub fn tiles_between(a: Pos, b: Pos) -> impl Iterator<Item = Pos> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| Pos::new(x, y)))
}

/// The anchors a Build drag from `from` to `to` stamps for a building of `footprint` size: a
/// lattice from the drag's start, stepping by the footprint away from it, keeping only the stamps
/// that fit inside the dragged rectangle, in row-major order. So a drag along a row lays
/// buildings side by side, and a 1x1 drag stamps every tile, as for any other tool. A drag too
/// small to fit one (a click) places one, anchored for the cursor (`build::anchor_for`).
pub fn build_stamps(from: Pos, to: Pos, footprint: (u8, u8)) -> Vec<Pos> {
    // The anchor's coordinate on one axis for each stamp that fits between `start` and `end`.
    let axis = |start: i32, end: i32, size: u8| -> Vec<i32> {
        let size = i32::from(size);
        let fit = ((end - start).abs() + 1) / size;
        (0..fit)
            .map(|k| if end >= start { start + k * size } else { start - (k + 1) * size + 1 })
            .collect()
    };
    let (xs, ys) = (axis(from.x, to.x, footprint.0), axis(from.y, to.y, footprint.1));
    if xs.is_empty() || ys.is_empty() {
        return vec![build::anchor_for(from, footprint)];
    }
    let mut stamps: Vec<Pos> =
        ys.iter().flat_map(|&y| xs.iter().map(move |&x| Pos::new(x, y))).collect();
    stamps.sort_by_key(|p| (p.y, p.x));
    stamps
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

/// Applies `tool` to every tile in the rectangle `from`..=`to` (for `Build`, to every footprint
/// `build_stamps` fits in it) through `Session::apply` and returns the notice to show, such as "designate harvest: 12 done, 3 skipped (nothing to
/// harvest), 2 unreachable". `repeating` is the Shift state (#81); `choice` is the Build tool's
/// building.
///
/// The sim decides what each tile accepts; a refusal is counted and its reason (the last one's)
/// shown, even for a one-tile click, so a rejected bend or harvest says why. For `Harvest` and
/// `Stockpile`, a designation that went through is also checked against `World::job_unreachable`
/// (#337: "designating a job or stockpile no settler can reach says so in the notice"), so a tile
/// on the wrong side of a river, wall or shore is called out rather than silently "done".
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
    let pick = Pick { ability: bend_ability(session), repeating, building };
    let what = match (tool, building) {
        (Tool::Build, Some(id)) => {
            let name = rows.iter().find(|r| r.id == id).map_or("", |r| r.name.as_str());
            format!("build {name}")
        }
        // Short, not the key-help label ("clear the ground"): docs/design/clearing.md's own
        // example notice is `clear: 6 marked, 2 skipped (can't be cleared)`.
        (Tool::Clear, _) => "clear".to_owned(),
        _ => tool.label().to_owned(),
    };
    let targets: Vec<Pos> = match (tool, building) {
        (Tool::Build, Some(id)) => build_stamps(from, to, session.defs.building(id).footprint),
        _ => tiles_between(from, to).collect(),
    };
    // So `job_unreachable` below has an answer even for the very first designation in a fresh
    // game, before any idle unit's own job search would otherwise have warmed the cache (#337).
    if matches!(tool, Tool::Harvest | Tool::Stockpile) {
        session.world.ensure_regions();
    }
    let (mut done, mut refused, mut unreachable) = (0, 0, 0);
    let mut last_error = None;
    for pos in targets {
        if let Some(command) = tool.command(pos, Here::at(session, pos), pick) {
            match session.apply(command) {
                Ok(()) => {
                    done += 1;
                    if matches!(tool, Tool::Harvest | Tool::Stockpile)
                        && session.world.job_unreachable(pos) == Some(true)
                    {
                        unreachable += 1;
                    }
                }
                Err(e) => {
                    refused += 1;
                    last_error = Some(e);
                }
            }
        }
    }
    let suffix =
        if unreachable > 0 { format!(", {unreachable} unreachable") } else { String::new() };
    // Clear marks tiles rather than finishing work on them (a unit still has to do the
    // clearing), so its own notice says "marked" (docs/design/clearing.md, "One removal
    // vocabulary"); every other tool keeps "done".
    let verb = if tool == Tool::Clear { "marked" } else { "done" };
    match last_error {
        Some(e) => {
            let why = build::describe_error(session, e);
            format!("{what}: {done} {verb}, {refused} skipped ({why}){suffix}")
        }
        None => format!("{what}: {done} {verb}, {refused} skipped{suffix}"),
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
        assert_eq!(Tool::Select.command(pos, Here::default(), none), None);
        assert_eq!(
            Tool::Harvest.command(pos, Here::default(), none),
            Some(Command::DesignateHarvest { pos })
        );
        assert_eq!(
            Tool::CancelHarvest.command(pos, Here::default(), none),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(
            Tool::Stockpile.command(pos, Here::default(), none),
            Some(Command::DesignateStockpile { pos })
        );
        let ability = AbilityId(0);
        let bend = Pick { ability: Some(ability), ..none };
        assert_eq!(
            Tool::Bend.command(pos, Here::default(), bend),
            Some(Command::Bend { ability, pos })
        );
        let building = BuildingId(2);
        let build = Pick { building: Some(building), ..none };
        assert_eq!(
            Tool::Build.command(pos, Here::default(), build),
            Some(Command::DesignateBuild { pos, building })
        );
        assert_eq!(Tool::Sow.command(pos, Here::default(), none), Some(Command::Sow { pos }));
        assert_eq!(
            Tool::Clear.command(pos, Here::default(), none),
            Some(Command::DesignateClear { pos })
        );
    }

    #[test]
    fn cancel_withdraws_a_clear_mark_where_there_is_one() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        let clearing = Here { clearing: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, clearing, none),
            Some(Command::CancelClear { pos })
        );
        // A build site comes first, as for a plain harvest or a Sow mark.
        let site = Here { site: true, clearing: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, site, none),
            Some(Command::CancelBuild { pos })
        );
        // The Clear tool itself is unaffected by what else is on the tile.
        assert_eq!(Tool::Clear.command(pos, clearing, none), Some(Command::DesignateClear { pos }));
    }

    #[test]
    fn cancel_withdraws_a_sow_designation_where_there_is_one() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        let sowing = Here { sowing: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, sowing, none),
            Some(Command::CancelSow { pos })
        );
        // A build site or stockpile tile comes first, as for a plain harvest.
        let site = Here { site: true, sowing: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, site, none),
            Some(Command::CancelBuild { pos })
        );
    }

    #[test]
    fn build_needs_a_chosen_building_to_issue_a_command() {
        assert_eq!(Tool::Build.command(Pos::new(0, 0), Here::default(), Pick::default()), None);
    }

    #[test]
    fn cancel_withdraws_a_build_site_where_there_is_one() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        let site = Here { site: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, site, none),
            Some(Command::CancelBuild { pos })
        );
        // Only the cancel tool does.
        assert_eq!(Tool::Harvest.command(pos, site, none), Some(Command::DesignateHarvest { pos }));
    }

    #[test]
    fn cancel_withdraws_a_stockpile_tile_where_there_is_one() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        let zone = Here { zone: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, zone, none),
            Some(Command::CancelStockpile { pos })
        );
        assert_eq!(
            Tool::Stockpile.command(pos, zone, none),
            Some(Command::DesignateStockpile { pos })
        );
        // A build site comes first.
        let both = Here { site: true, zone: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, both, none),
            Some(Command::CancelBuild { pos })
        );
    }

    #[test]
    fn holding_shift_makes_a_harvest_designation_repeating() {
        let pos = Pos::new(1, 2);
        let shift = Pick { repeating: true, ..Pick::default() };
        assert_eq!(
            Tool::Harvest.command(pos, Here::default(), shift),
            Some(Command::DesignateRepeatingHarvest { pos })
        );
        // Shift only changes what the harvest tool does.
        assert_eq!(
            Tool::CancelHarvest.command(pos, Here::default(), shift),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(Tool::Select.command(pos, Here::default(), shift), None);
    }

    #[test]
    fn bend_needs_an_ability_to_issue_a_command() {
        assert_eq!(Tool::Bend.command(Pos::new(0, 0), Here::default(), Pick::default()), None);
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
    fn a_new_game_can_use_only_the_signature_ability_so_b_has_nothing_to_cycle() {
        for nation in Nation::ALL {
            let mut s = Session::new(1, nation, 48, 32);
            let signature = ability_for_nation(&s.defs, nation).unwrap();
            assert_eq!(usable_abilities(&s), [signature], "{nation:?}");
            assert_eq!(bend_ability(&s), Some(signature));
            cycle_ability(&mut s, Tool::Bend, Tool::Bend);
            assert_eq!(bend_ability(&s), Some(signature), "{nation:?}");
            assert!(!Tool::Bend.hint(&s).contains("next ability"));
        }
    }

    #[test]
    fn b_again_cycles_through_what_research_has_unlocked() {
        // Fire's Inner Fire needs Dancing Dragon; research it the slow way, with the settlers
        // kept fed so the passive trickle has time to pay for it. Forging, a prereq on the way
        // (through Smelting), also unlocks Weld (#363), so it comes along for the ride.
        let mut s = Session::new(3, Nation::Fire, 48, 32);
        let dragon = s.defs.tech_id("dancing_dragon").unwrap();
        let (hearth, inner, weld) = (
            s.defs.ability_id("hearthflame").unwrap(),
            s.defs.ability_id("inner_fire").unwrap(),
            s.defs.ability_id("weld").unwrap(),
        );
        assert_eq!(bend_ability(&s), Some(hearth));
        s.apply(Command::SetResearch { tech: dragon }).unwrap();
        for _ in 0..200_000 {
            if s.world.research().is_known(dragon) {
                break;
            }
            s.tick();
        }
        assert!(s.world.research().is_known(dragon), "researched");
        assert_eq!(usable_abilities(&s), [hearth, inner, weld]);
        assert!(Tool::Bend.hint(&s).contains("B: next ability"));

        // Picking the tool leaves the choice; B again moves on, wrapping round.
        cycle_ability(&mut s, Tool::Select, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(hearth));
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(inner));
        assert!(Tool::Bend.hint(&s).starts_with(&s.defs.ability(inner).verb));
        // Another tool and back keeps it; the tool then issues that ability.
        cycle_ability(&mut s, Tool::Bend, Tool::Harvest);
        cycle_ability(&mut s, Tool::Harvest, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(inner));
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(weld));
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(hearth));
    }

    /// #248: in a mixed-nation settlement (#240) `B` also cycles onto a newcomer's signature
    /// ability, not just the settlement's own nation's.
    #[test]
    fn b_also_cycles_onto_a_newcomers_nation() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let hearth = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let raise_earth = ability_for_nation(&s.defs, Nation::Earth).unwrap();
        assert_eq!(usable_abilities(&s), [hearth], "no Earth newcomer yet");

        s.world.units[1].nation = Some(Nation::Earth);
        let usable = usable_abilities(&s);
        assert_eq!(usable, [raise_earth, hearth], "data order, not which nation founded it");
        assert!(Tool::Bend.hint(&s).contains("B: next ability"));

        assert_eq!(bend_ability(&s), Some(raise_earth), "the first usable, with no choice yet");
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(hearth));
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(raise_earth), "wraps round");
    }

    #[test]
    fn every_tool_has_a_key_and_the_letters_are_distinct() {
        assert_eq!(Tool::Select.key(), "Esc");
        assert_eq!(Tool::Select.letter(), None);
        let letters: Vec<char> = Tool::ALL.iter().filter_map(|t| t.letter()).collect();
        assert_eq!(letters, ['H', 'X', 'E', 'P', 'B', 'U', 'C']);
        for tool in Tool::ALL {
            if let Some(c) = tool.letter() {
                assert_eq!(Tool::from_letter(c), Some(tool));
                assert_eq!(Tool::from_letter(c.to_ascii_lowercase()), Some(tool));
                assert_eq!(tool.key(), c.to_string());
            }
        }
        assert_eq!(Tool::from_letter('Z'), None);
    }

    /// The banner's brief (#307) never repeats #305's dropped "hold Shift to repeat", and is
    /// empty exactly for the tools whose name badge already says enough on its own.
    #[test]
    fn brief_has_no_stale_shift_text_and_is_empty_where_the_name_says_enough() {
        for tool in Tool::ALL {
            assert!(!tool.brief().contains("Shift"), "{tool:?}");
        }
        for tool in [Tool::Select, Tool::Clear, Tool::Stockpile, Tool::Bend, Tool::Build, Tool::Sow]
        {
            assert_eq!(tool.brief(), "", "{tool:?}");
        }
        assert_eq!(Tool::Harvest.brief(), "chop, mine, gather");
        assert!(!Tool::CancelHarvest.brief().is_empty());
    }

    #[test]
    fn the_hint_names_the_nation_s_own_ability_for_bend() {
        let s = Session::new(1, Nation::Fire, 48, 32);
        let hint = Tool::Bend.hint(&s);
        let id = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        assert!(hint.starts_with(&s.defs.ability(id).verb), "{hint}");
        assert_eq!(Tool::Harvest.hint(&s), Tool::Harvest.label());
    }

    /// A free land tile near the first unit, away from every unit, with room for a 1x1
    /// building's door (#226).
    fn free_tile(s: &Session) -> Pos {
        let bed = s.defs.building_id("bed").unwrap();
        s.world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == fn_sim::Terrain::Land
                    && t.resource.is_none()
                    && !s.world.units.iter().any(|u| u.pos == *p)
                    && s.world.check_build(*p, bed, &s.defs).is_ok()
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
    fn the_build_key_never_cycles_through_another_nation_s_buildings() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let orchard = s.defs.building_id("orchard").unwrap();
        let (mut tool, mut choice) = (Tool::Select, None);
        for _ in 0..s.defs.buildings.len() * 2 {
            (tool, choice) = switch_tool(&s, tool, choice, Tool::Build);
            assert_ne!(choice, Some(orchard));
        }
        // A stale choice of a hidden building falls back to a placeable one.
        let (_, fixed) = switch_tool(&s, Tool::Select, Some(orchard), Tool::Build);
        assert_ne!(fixed, Some(orchard));
        assert!(fixed.is_some());
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
            notice.starts_with("designate harvest (chop, mine, gather): 0 done, 1 skipped ("),
            "{notice}"
        );
        assert!(notice.ends_with(')'), "{notice}");

        assert_eq!(apply_rect(&mut s, Tool::Select, pos, to, false, None), "");
    }

    /// A harvest designated on an island nobody can reach says so in the notice (#337:
    /// "designating a job or stockpile no settler can reach says so in the notice"), not just
    /// "done" like one that will actually get worked.
    #[test]
    fn designating_an_unreachable_harvest_says_so_in_the_notice() {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Defs, Grid, Nation, Terrain, Tile, World};

        use crate::clock::Clock;

        let defs = Defs::bundled();
        let rows = ["~~~~~~~~~", "~T~.....~", "~~~.....~", "~~~~~~~~~"];
        let map = Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
            let c = rows[p.y as usize].as_bytes()[p.x as usize];
            Tile {
                elevation: 100,
                temperature: 128,
                moisture: 128,
                terrain: if c == b'~' { Terrain::Water } else { Terrain::Land },
                biome: BiomeId(0),
                resource: (c == b'T').then(|| defs.resource_id("oak_tree").unwrap()),
                building: None,
            }
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(3, 2)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };
        let island = Pos::new(1, 1);
        let notice = apply_rect(&mut s, Tool::Harvest, island, island, false, None);
        assert!(notice.contains("1 done"), "{notice}");
        assert!(notice.contains("1 unreachable"), "{notice}");

        // An ordinary, reachable designation names no unreachable count at all.
        let reachable = Pos::new(5, 1);
        let notice = apply_rect(&mut s, Tool::Stockpile, reachable, reachable, false, None);
        assert!(!notice.contains("unreachable"), "{notice}");
    }

    /// docs/design/clearing.md, #411/#426: a fruit tree marks ("clear: N marked"), but a fish
    /// spot -- a renewable with no `[harvest.clear]` table -- is refused with "can't be
    /// cleared", the reason the notice names.
    #[test]
    fn clear_marks_a_fruit_tree_and_refuses_a_fish_spot_as_cant_be_cleared() {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Defs, Grid, Nation, Terrain, Tile, World};

        use crate::clock::Clock;

        let mut defs = Defs::bundled();
        let fruit = defs.resource_id("fruit_tree").unwrap();
        let fish = defs.resource_id("fish_spot").unwrap();
        // Fishing itself is still behind its own tech in the bundled data; drop the gate here so
        // the refusal below is `CannotClear`, not `NotResearched` (the tech gate isn't this
        // test's own subject, as `with_2x2_hall`'s footprint mutation elsewhere in this module
        // also isn't).
        defs.harvests.iter_mut().find(|h| h.resource == fish).unwrap().unlocked_by = None;
        let (fruit_pos, fish_pos) = (Pos::new(1, 0), Pos::new(2, 0));
        let map = Grid::from_fn(4, 2, |p| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if p == fish_pos { Terrain::Water } else { Terrain::Land },
            biome: BiomeId(0),
            resource: if p == fruit_pos {
                Some(fruit)
            } else if p == fish_pos {
                Some(fish)
            } else {
                None
            },
            building: None,
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(0, 1)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };

        let notice = apply_rect(&mut s, Tool::Clear, fruit_pos, fish_pos, false, None);
        assert_eq!(notice, "clear: 1 marked, 1 skipped (can't be cleared)", "{notice}");
        assert_eq!(s.world.clear_designations().collect::<Vec<_>>(), [fruit_pos]);
    }

    #[test]
    fn a_bend_drag_designates_the_whole_chain_across_a_lake_at_once() {
        // #304: a Raise Earth drag spanning a lake used to be refused past its first tile; the
        // sim now chains a pending designation off its already-pending neighbour, so the whole
        // line the player dragged over designates in one go, with nothing skipped.
        use fn_sim::defs::BiomeId;
        use fn_sim::{Grid, Terrain, Tile, TileMap, World};

        use crate::clock::Clock;

        let defs = Defs::bundled();
        let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
        let map: TileMap = Grid::from_fn(10, 3, |p| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if (3..=6).contains(&p.x) { Terrain::Lake } else { Terrain::Land },
            biome: plains,
            resource: None,
            building: None,
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(1, 1)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };
        s.bend_choice = Some(s.defs.ability_id("raise_earth").unwrap());

        let notice = apply_rect(&mut s, Tool::Bend, Pos::new(3, 1), Pos::new(6, 1), false, None);
        assert_eq!(notice, "channel the nation's ability: 4 done, 0 skipped", "{notice}");
        for x in 3..=6 {
            let pos = Pos::new(x, 1);
            assert!(s.world.bend_designations().any(|(_, p)| p == pos), "x={x}");
        }
    }

    /// The restoke rule (#359): a flame that's still nearly full refuses a second Hearthflame on
    /// the same tile, and the notice names why -- the same "done, skipped (reason)" shape every
    /// other tool already reports through, so no bend-specific plumbing was needed.
    #[test]
    fn restoking_a_still_fresh_flame_is_skipped_with_the_restoke_reason() {
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        s.bend_choice = Some(s.defs.ability_id("hearthflame").unwrap());
        let pos = free_tile(&s);

        let first = apply_rect(&mut s, Tool::Bend, pos, pos, false, None);
        assert_eq!(first, "channel the nation's ability: 1 done, 0 skipped", "{first}");
        s.tick_n(2000);
        assert!(s.world.flames().iter().any(|f| f.pos == pos), "the flame should be lit by now");

        let second = apply_rect(&mut s, Tool::Bend, pos, pos, false, None);
        assert_eq!(
            second,
            "channel the nation's ability: 0 done, 1 skipped \
             (flame still burning, restoke below half)",
            "{second}"
        );
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
    fn cancel_drags_over_a_stockpile_and_clears_it() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let pos = free_tile(&s);
        let to = Pos::new(pos.x + 1, pos.y);
        apply_rect(&mut s, Tool::Stockpile, pos, to, false, None);
        let zones: Vec<Pos> = s.world.stockpile_zones().collect();
        assert!(!zones.is_empty());
        let notice = apply_rect(&mut s, Tool::CancelHarvest, pos, to, false, None);
        assert!(notice.contains(&format!("{} done", zones.len())), "{notice}");
        assert_eq!(s.world.stockpile_zones().count(), 0);
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

    /// A session whose bed is a 2x2 building (no bundled building is bigger yet, #214), with a
    /// clear 6x3 field of land from the returned top-left tile: the footprint's own two rows,
    /// plus the row below for every anchor's door (#226).
    fn with_2x2_hall() -> (Session, BuildingId, Pos) {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let bed = s.defs.building_id("bed").unwrap();
        let def = &mut s.defs.buildings[bed.0 as usize];
        def.footprint = (2, 2);
        def.bed = None;
        let corner = Pos::new(20, 20);
        for p in tiles_between(corner, Pos::new(corner.x + 5, corner.y + 2)) {
            let t = s.world.map.get_mut(p).unwrap();
            t.terrain = fn_sim::Terrain::Land;
            t.resource = None;
        }
        (s, bed, corner)
    }

    #[test]
    fn a_build_drag_stamps_footprints_side_by_side_and_skips_the_blocked_ones() {
        let (mut s, hall, c) = with_2x2_hall();
        let oak = s.defs.resource_id("oak_tree").unwrap();
        s.world.map.get_mut(Pos::new(c.x + 5, c.y + 1)).unwrap().resource = Some(oak);
        let notice =
            apply_rect(&mut s, Tool::Build, c, Pos::new(c.x + 5, c.y + 1), false, Some(hall));
        assert_eq!(notice, "build Bed: 2 done, 1 skipped (an oak tree is in the way)");
        let anchors: Vec<Pos> = s.world.build_sites().map(|(f, _, _)| f.anchor).collect();
        assert_eq!(anchors, [c, Pos::new(c.x + 2, c.y)]);
    }

    #[test]
    fn build_stamps_tile_a_drag_from_its_start_and_fit_inside_it() {
        let p = |x, y| Pos::new(x, y);
        assert_eq!(build_stamps(p(0, 0), p(5, 1), (2, 2)), [p(0, 0), p(2, 0), p(4, 0)]);
        // A 5-wide drag fits two; the leftover column is not stamped.
        assert_eq!(build_stamps(p(0, 0), p(4, 1), (2, 2)), [p(0, 0), p(2, 0)]);
        // Dragged up and to the left, the lattice grows from the start that way.
        assert_eq!(
            build_stamps(p(5, 3), p(0, 0), (2, 2)),
            [p(0, 0), p(2, 0), p(4, 0), p(0, 2), p(2, 2), p(4, 2)]
        );
        // A click, or a drag too small for one, places one at the cursor's anchor.
        assert_eq!(build_stamps(p(7, 7), p(7, 7), (3, 3)), [p(6, 6)]);
        assert_eq!(build_stamps(p(7, 7), p(8, 7), (2, 2)), [p(7, 7)]);
        // 1x1 stamps every tile in the rectangle, in the same order as any other tool.
        let one: Vec<Pos> = tiles_between(p(4, 2), p(1, 3)).collect();
        assert_eq!(build_stamps(p(4, 2), p(1, 3), (1, 1)), one);
    }

    #[test]
    fn a_2x2_site_is_cancelled_from_any_of_its_tiles() {
        let (mut s, hall, c) = with_2x2_hall();
        apply_rect(&mut s, Tool::Build, c, c, false, Some(hall));
        let corner = Pos::new(c.x + 1, c.y + 1);
        assert_eq!(Here::at(&s, corner), Here::at(&s, c));
        assert!(Here::at(&s, corner).site);
        let notice = apply_rect(&mut s, Tool::CancelHarvest, corner, corner, false, None);
        assert!(notice.ends_with("1 done, 0 skipped"), "{notice}");
        assert_eq!(s.world.build_sites().count(), 0);
    }
}
