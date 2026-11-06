//! What is under the cursor: the lines of the Here section (the terminal UI, #418; CURSOR until
//! then).

use fn_sim::building::BuildingId;
use fn_sim::world::{PlotStage, Unit};
use fn_sim::{Defs, Pos, World};

use crate::build;
use crate::session::Session;
use crate::text::{describe_stack, describe_task};
use crate::tools::Tool;

/// An action the player could take on what they're looking at. Each frontend words it for its
/// own input: "Click to select" in one, "Enter to select" in the other.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hint {
    /// Select the unit here.
    SelectUnit,
    /// The tool isn't active; picking it (its key) would let you act on this tile.
    StartTool(Tool),
    /// The tool is active; acting now (a drag, or a rectangle's `Enter`) applies it here.
    ApplyTool(Tool),
}

/// One line of a tooltip, and so its role. The text is final English except for hints.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Line {
    Title(String),
    Detail(String),
    Hint(Hint),
}

pub type Lines = Vec<Line>;

/// Seconds as "12m 03s".
fn clock(secs: f32) -> String {
    let secs = secs.max(0.0) as u64;
    format!("{}m {:02}s", secs / 60, secs % 60)
}

/// "Lit: 12m 03s left" for a flame on `tile`, "Earth Tent: 12m 03s left" for a shelter (#362, by
/// the ability that lit it, same as the TUI's own glyph for it), or "Wet: sprouts in 3m 20s" for
/// a watered one. `None` for anything else.
pub fn bending_status_line(world: &World, defs: &Defs, tile: Pos) -> Option<String> {
    if let Some(flame) = world.flames().into_iter().find(|f| f.pos == tile) {
        // Hearthflame, and a save from before the split (`ability` is `None`), keep today's
        // generic wording; a shelter (Earth Tent, Air Shield) names itself instead.
        let label = match flame.ability.map(|a| defs.ability(a)) {
            Some(def) if def.id != "hearthflame" => def.name.as_str(),
            _ => "Lit",
        };
        return Some(format!("{label}: {} left", clock(flame.seconds_left)));
    }
    let (_, left) = world.wet_tiles().into_iter().find(|(p, _)| *p == tile)?;
    Some(format!("Wet: sprouts in {}", clock(left)))
}

/// "Stockpile: 12/20 Wood" or "Stockpile: empty" for a designated zone tile (#337: "stockpile
/// capacity is visible somewhere"). `None` off a zone.
fn stockpile_line(world: &World, defs: &Defs, tile: Pos) -> Option<String> {
    if !world.stockpile_zones().any(|z| z == tile) {
        return None;
    }
    let cap = defs.jobs.stockpile_stack;
    Some(match world.stored_at(tile) {
        Some(s) => format!("Stockpile: {}/{cap} {}", s.count, defs.item(s.item).name),
        None => "Stockpile: empty".to_owned(),
    })
}

/// "Lying here: 4 Wood (unreachable: every stockpile is full)" for an item stack waiting on the
/// ground for a hauler (#337: a filled stockpile left a 2026-11-05 playtest's chopped wood on
/// the ground with nothing to say why). The reason is only ever a full stockpile: `place_ground`
/// never drops a stack somewhere no one could reach in the first place. `None` with nothing
/// lying on `tile`.
fn ground_line(world: &World, defs: &Defs, tile: Pos) -> Option<String> {
    let (_, stack) = world.ground_items().find(|&(p, _)| p == tile)?;
    let name = defs.item(stack.item).name.clone();
    let cap = defs.jobs.stockpile_stack;
    let has_room = |pos: Pos| match world.stored_at(pos) {
        None => true,
        Some(s) => s.item == stack.item && s.count < cap,
    };
    let stuck = world.stockpile_zones().next().is_some() && !world.stockpile_zones().any(has_room);
    let suffix = if stuck { " (unreachable: every stockpile is full)" } else { "" };
    Some(format!("Lying here: {} {name}{suffix}", stack.count))
}

/// How many finished, `uses_power` buildings currently have `source` as the one that actually
/// powers them: the best boost in reach (`World::power_at`), not merely every building its reach
/// physically covers (two sources never both power one building, #428). For a source's own
/// "Powering" readout (#429). Counted by anchor (`Building::part` zero), so a multi-tile
/// consumer isn't counted once per tile.
fn powering_count(world: &World, defs: &Defs, source: Pos) -> usize {
    world
        .map
        .iter()
        .filter(|&(p, t)| {
            t.building.is_some_and(|b| b.part == [0, 0] && defs.building(b.id).uses_power)
                && world.power_at(p, defs).is_some_and(|at| at.source == source)
        })
        .count()
}

/// What a finished power building says about itself (issue #428, #429,
/// docs/design/late-tree.md "UI"): a source's reach, already scaled for the nation's current
/// `power_generation`, and how many buildings it's powering right now; a consumer's source and
/// boost, or "Unpowered" with none in reach. Empty for a building that is neither.
fn power_lines(world: &World, defs: &Defs, anchor: Pos, building: BuildingId) -> Vec<String> {
    let def = defs.building(building);
    if let Some(power) = def.power {
        let reach = power.radius_for(world.modifiers().power_generation);
        let powering = powering_count(world, defs, anchor);
        vec![
            format!("Reach: {reach} tiles"),
            format!("Powering: {powering} building{}", if powering == 1 { "" } else { "s" }),
        ]
    } else if def.uses_power {
        let line = match world.power_at(anchor, defs) {
            Some(p) => {
                let source = world.building_at(p.source).expect("a registered source stands here");
                format!(
                    "Powered: {}, {:.1}x",
                    defs.building(source).display_name(world.nation),
                    p.boost
                )
            }
            None => "Unpowered".to_owned(),
        };
        vec![line]
    } else {
        Vec::new()
    }
}

/// The tooltip for what is under the cursor at `tile`: up to 4 lines (name, context, a yield or
/// reason, then an action hint). `None` when `tile` is off the map.
///
/// A unit standing there (`units_at` skips one inside a building, #226: it isn't standing there
/// as far as the map is concerned, so hovering its door falls through to the building below
/// instead) takes over the tile entirely (`unit_lines`); otherwise this is [`tile_lines`].
pub fn tooltip_lines(session: &Session, tool: Tool, tile: Pos) -> Option<Lines> {
    let world = &session.world;
    world.map.get(tile)?;
    if let Some(unit) = world.units_at(tile).next() {
        return Some(unit_lines(session, tool, unit));
    }
    tile_lines(session, tool, tile)
}

/// A unit's own lines (docs/design/side-panel-v2.md "Zone 3"): its name, its task and, once
/// selected, `Enter: select`. The own-nation prefix (`Fire Nation - `) only earns its keep once
/// the settlement has more than one nation (#248, "the transformation") -- on a single-nation
/// game it's the same word on every unit, worth nothing. Carrying shows only with something to
/// carry ("nothing" says nothing worth a row); finished bends moved to the selection bar's own
/// line instead (#418, `fn_ui::text::bends_line`), since a unit under the cursor can be selected
/// and show both at once.
fn unit_lines(session: &Session, tool: Tool, unit: &Unit) -> Lines {
    let (world, defs) = (&session.world, &session.defs);
    let mut lines = Lines::new();
    lines.push(Line::Title(unit.name.clone()));
    let task = describe_task(world, defs, unit);
    // Inner Fire (#123) and the cold (#299) share this slot, since a tooltip tops out at
    // four lines and the two never apply at once (being heated is what rules cold out,
    // `World::is_cold`).
    let status = world
        .glows()
        .into_iter()
        .find(|g| g.unit == unit.id)
        .map(|g| format!(" - Warm {}", clock(g.seconds_left)))
        .or_else(|| {
            world
                .is_cold(unit.pos, defs)
                .then(|| " - Cold: tiring faster (no fire near)".to_owned())
        })
        .unwrap_or_default();
    let multi_nation = crate::view::population_by_nation(world, defs).len() > 1;
    let detail = if multi_nation {
        format!("{} - {task}{status}", defs.nation(world.nation_of(unit)).name)
    } else {
        format!("{task}{status}")
    };
    lines.push(Line::Detail(detail));
    if unit.carrying.is_some() {
        lines.push(Line::Detail(format!("Carrying: {}", describe_stack(defs, unit.carrying))));
    }
    if tool == Tool::Select {
        lines.push(Line::Hint(Hint::SelectUnit));
    }
    lines
}

/// What `tile` itself holds, ignoring any unit standing there: the Here section's fallback once
/// the unit under the cursor already has the selection bar open (#418), so it isn't named twice.
/// `None` when `tile` is off the map.
pub fn tile_lines(session: &Session, tool: Tool, tile: Pos) -> Option<Lines> {
    let (world, defs) = (&session.world, &session.defs);
    let t = world.map.get(tile)?;
    let mut lines = Lines::new();
    // A construction site or a finished building (#111): what it is and how far along.
    if let Some(report) = build::site_report(session, tile) {
        lines.push(Line::Title(format!("{} (under construction)", report.name)));
        lines.push(Line::Detail(report.materials));
        lines.push(Line::Detail(report.work));
        lines.push(Line::Hint(if tool == Tool::CancelHarvest {
            Hint::ApplyTool(Tool::CancelHarvest)
        } else {
            Hint::StartTool(Tool::CancelHarvest)
        }));
        return Some(lines);
    }
    if let Some(name) = build::built_name(session, tile) {
        lines.push(Line::Title(name));
        // Its anchor, not the hovered tile (#212): every tile of its footprint says the same.
        let at = world.footprint_at(tile).map_or(tile, |f| f.anchor);
        lines.push(Line::Detail(format!("Built ({}, {})", at.x, at.y)));
        // An orchard says how much of its grove is planted (#142); a Fishing Dock (#374) the
        // same, in terms of fishing spots rather than trees.
        let orchard = world.building_at(tile).and_then(|b| defs.building(b).orchard);
        if let (Some(trees), Some(o)) = (world.orchard_trees(tile), orchard) {
            let plants = defs.resource(o.plants).name.to_lowercase();
            lines.push(Line::Detail(format!("Tends {trees} of {} {plants}s", o.max_trees)));
        }
        // A farm plot says what stage it's in (#198).
        if let Some(stage) = world.plot_stage(tile) {
            lines.push(Line::Detail(match stage {
                PlotStage::Empty => "Empty".to_owned(),
                PlotStage::Growing => {
                    let left = world.plot_growth_left(tile).unwrap_or(0.0);
                    format!("Growing, ripe in {}", clock(left))
                }
                PlotStage::Ripe => "Ripe".to_owned(),
            }));
        }
        // A study hall says how many of its seats are filled right now (#338): the Build
        // modal's own line (`build::building_detail`) only has the total, since nothing is
        // built yet when it shows it.
        if let Some(seats) =
            world.building_at(tile).map(|b| defs.building(b).seats).filter(|&s| s > 0)
        {
            let filled = world.occupants(at).count();
            lines.push(Line::Detail(format!("Study: {filled}/{seats} seats filled")));
        }
        // A power source names its reach and how many buildings it's powering; a consumer names
        // which source powers it, and at what boost, or "Unpowered" (issue #429).
        if let Some(building) = world.building_at(tile) {
            lines.extend(power_lines(world, defs, at, building).into_iter().map(Line::Detail));
        }
        return Some(lines);
    }
    let biome = &defs.biome(t.biome).name;
    let at = format!("{biome} - {:?} ({}, {})", t.terrain, tile.x, tile.y);
    let Some(res) = t.resource else {
        lines.push(Line::Title(format!("{:?}", t.terrain)));
        lines.push(Line::Detail(format!("{biome} ({}, {})", tile.x, tile.y)));
        lines.push(Line::Detail(format!("Elevation {}", t.elevation)));
        // The climate band's winter effects (#299, docs/design/difficulty.md "Readable
        // simulation"): only once a difficulty preset turns seasons on, which is also when
        // `World::tick_needs` and `World::growth_rate` start reading it.
        if world.knobs(defs).seasons
            && let Some(c) = world.climate_at(tile, defs)
        {
            let effects = match (c.cold, c.frost) {
                (true, true) => "cold and frost",
                (true, false) => "cold",
                (false, true) => "frost",
                (false, false) => "mild",
            };
            lines.push(Line::Detail(format!("{} climate: {effects} in winter", c.name)));
        }
        // A lit flame or a wet patch (#72): the result of a bend, so hovering explains it
        // (pillar 4, "readable simulation").
        if let Some(status) = bending_status_line(world, defs, tile) {
            lines.push(Line::Detail(status));
        }
        // The tile's own designation (#337): a stockpile zone names what it holds and its
        // capacity, an item stack waiting for a hauler names itself and, if a full stockpile is
        // why it's still here, says so. The two never both apply (`World::place_ground` never
        // drops a stack on a zone tile).
        if let Some(line) = stockpile_line(world, defs, tile) {
            lines.push(Line::Detail(line));
        } else if let Some(line) = ground_line(world, defs, tile) {
            lines.push(Line::Detail(line));
        }
        return Some(lines);
    };
    lines.push(Line::Title(defs.resource(res).name.clone()));
    lines.push(Line::Detail(at));
    if let Some(h) = defs.harvest(res) {
        // A standing order (#81) outlives the resting period it's shown through, so it's folded
        // into the existing lines rather than a line of its own (tooltips top out at 4 lines).
        let repeating = world.is_repeating_harvest(tile);
        if world.is_sapling(tile) {
            // An orchard's young tree (#142): not just picked, it has never borne fruit.
            let text = if repeating { "Sapling - repeating order" } else { "Sapling" };
            lines.push(Line::Detail(text.into()));
        } else if world.is_resting(tile) {
            // Its own line, not folded into the harvest's (docs/design/difficulty.md "Readable
            // simulation"): "Growing, ripe in 11m 05s - tired (strain 2.1)" doesn't fit the
            // panel's 28 columns together, but each half alone does.
            let left = world.resting_left(tile).unwrap_or(0.0);
            let suffix = if repeating { " - repeating order" } else { "" };
            lines.push(Line::Detail(format!("Growing, ripe in {}{suffix}", clock(left))));
            // Strain (#296): off, or fully rested already, says nothing.
            if let Some(strain) = world.strain_at(tile).filter(|&s| s > 0.0) {
                lines.push(Line::Detail(format!("Tired (strain {strain:.1})")));
            }
            // Hard only: this harvest's own strain, added to what it already carries, would
            // reach the kill threshold next time it's picked.
            let knobs = world.knobs(defs);
            if knobs.strain_kills {
                let strain = world.strain_at(tile).unwrap_or(0.0);
                if strain + h.strain * knobs.strain >= defs.jobs.strain_kill_at {
                    lines.push(Line::Detail("One more picking may kill it".to_owned()));
                }
            }
        } else {
            let item = &defs.item(h.item).name;
            let suffix = if repeating { " (repeating)" } else { "" };
            // A vein beside a tunnel (#132) is mined from inside the rock, for more.
            let count = world.harvest_yield(tile, h);
            let tunnel = if count > h.count { " from the tunnel" } else { "" };
            lines.push(Line::Detail(format!(
                "{}: {count} {item}{tunnel} - {:.0}s{suffix}",
                h.verb, h.work
            )));
            lines.push(Line::Hint(if tool == Tool::Harvest {
                Hint::ApplyTool(Tool::Harvest)
            } else {
                Hint::StartTool(Tool::Harvest)
            }));
        }
        // A harvest designated on this tile (a one-shot job, or a standing order waiting to
        // ripen) that no living settler could ever reach (#337: "inspect shows no designation
        // or 'unreachable' status on a tile, so a job can't be told apart from one that's just
        // waiting" -- a playtest's own scroll stayed marked for a year across a river nobody
        // could cross). `None` from `job_unreachable` (the connectivity cache not warm yet)
        // says nothing rather than guess.
        if world.harvest_designations().any(|p| p == tile)
            && world.job_unreachable(tile) == Some(true)
        {
            lines.push(Line::Detail("Unreachable: no settler can reach it".to_owned()));
        }
    }
    Some(lines)
}

#[cfg(test)]
mod tests {
    use fn_sim::defs::{BiomeId, Terrain};
    use fn_sim::world::Command;
    use fn_sim::{Defs, Grid, Nation, Tile, TileMap, World};

    use super::*;
    use crate::clock::Clock;
    use crate::tools::ability_for_nation;

    /// A flat field, `w` by `h`, every tile at `temperature`, with `resource` on `(1, 1)`.
    fn field(
        w: i32,
        h: i32,
        temperature: u8,
        resource: Option<fn_sim::defs::ResourceId>,
    ) -> TileMap {
        Grid::from_fn(w, h, |p| Tile {
            elevation: 100,
            temperature,
            moisture: 128,
            terrain: Terrain::Land,
            biome: BiomeId(0),
            resource: if p == Pos::new(1, 1) { resource } else { None },
            building: None,
        })
    }

    fn custom_session(defs: Defs, world: World) -> Session {
        Session { world, defs, clock: Clock::default(), bend_choice: None }
    }

    #[test]
    fn tooltips_have_at_most_four_lines() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let mut seen_resource = false;
        let mut seen_unit = false;
        for (p, t) in s.world.map.iter() {
            for tool in [Tool::Select, Tool::Harvest] {
                let lines = tooltip_lines(&s, tool, p).expect("on the map");
                assert!((2..=4).contains(&lines.len()), "{p:?}: {lines:?}");
                assert!(matches!(lines[0], Line::Title(_)));
            }
            seen_resource |= t.resource.is_some();
        }
        for u in &s.world.units {
            seen_unit = true;
            let lines = tooltip_lines(&s, Tool::Select, u.pos).unwrap();
            assert_eq!(lines[0], Line::Title(u.name.clone()));
            assert_eq!(lines.last(), Some(&Line::Hint(Hint::SelectUnit)));
        }
        assert!(seen_resource && seen_unit);
        assert_eq!(tooltip_lines(&s, Tool::Select, Pos::new(-1, -1)), None);
    }

    /// A fresh single-nation game, off the Select tool: just the name and the task, no nation
    /// prefix (#418, "only once there's more than one nation") and no Carrying row (carrying
    /// nothing is omitted), nor a hint (that's Select's own).
    #[test]
    fn a_unit_only_offers_to_be_selected_with_the_select_tool() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let at = s.world.units[0].pos;
        let lines = tooltip_lines(&s, Tool::Harvest, at).unwrap();
        assert_eq!(lines.len(), 2, "name and task only: {lines:?}");
        assert!(!lines.contains(&Line::Hint(Hint::SelectUnit)));
    }

    /// The own-nation prefix only shows once the settlement has more than one nation (#418,
    /// #248 "the transformation"); until then it's the same word on every unit, so the Detail
    /// row is the plain task.
    #[test]
    fn the_nation_prefix_only_shows_once_the_settlement_has_more_than_one_nation() {
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        let at = s.world.units[0].pos;
        let lines = tooltip_lines(&s, Tool::Select, at).unwrap();
        let Line::Detail(detail) = &lines[1] else { panic!("{lines:?}") };
        assert!(!detail.contains("Nation"), "single-nation: {detail}");

        s.world.units[0].nation = Some(Nation::Earth);
        let lines = tooltip_lines(&s, Tool::Select, at).unwrap();
        let Line::Detail(detail) = &lines[1] else { panic!("{lines:?}") };
        assert!(detail.starts_with(&s.defs.nation(Nation::Earth).name), "{detail}");
    }

    /// Carrying something shows a row for it; finished bends moved to the selection bar's own
    /// line instead (#418, `fn_ui::text::bends_line`), so the tooltip never mentions them.
    #[test]
    fn a_unit_s_tooltip_shows_carrying_but_not_its_finished_bends() {
        use fn_sim::world::{BendOutcome, Command};
        let mut s = Session::new(3, Nation::Fire, 64, 64);
        let ability = s.defs.ability_id("hearthflame").expect("fire has a Hearthflame");
        let home = s.world.units[0].pos;
        let target = (1..8)
            .flat_map(|d| [Pos::new(home.x + d, home.y), Pos::new(home.x - d, home.y)])
            .find(|&pos| s.apply(Command::Bend { ability, pos }).is_ok())
            .expect("somewhere to light a Hearthflame");
        for _ in 0..50 {
            if !s.world.bend_events().is_empty() {
                break;
            }
            s.tick_n(20);
        }
        let event = s.world.bend_events()[0];
        assert_eq!(event.outcome, BendOutcome::Done, "at {target:?}");
        let caster = s.world.unit(event.unit).unwrap();
        assert_eq!(caster.bends(), 1, "finished a bend");
        let lines = tooltip_lines(&s, Tool::Select, caster.pos).unwrap();
        assert!(
            lines.iter().all(|l| !format!("{l:?}").contains("Bends")),
            "bends are the selection bar's line now (#418): {lines:?}"
        );

        let wood = s.defs.item_id("wood").expect("the bundled data has wood");
        let unit = s.world.units.iter_mut().find(|u| u.id == event.unit).unwrap();
        unit.carrying = Some(fn_sim::jobs::Stack { item: wood, count: 1 });
        let caster = s.world.unit(event.unit).unwrap();
        let lines = tooltip_lines(&s, Tool::Select, caster.pos).unwrap();
        let carrying =
            lines.iter().find(|l| matches!(l, Line::Detail(d) if d.starts_with("Carrying")));
        assert!(carrying.is_some(), "{lines:?}");
    }

    #[test]
    fn a_resource_hints_at_the_harvest_tool_and_applies_it_when_active() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let tree = s
            .world
            .map
            .iter()
            .find(|(p, t)| {
                t.resource.and_then(|r| s.defs.harvest(r)).is_some_and(|h| h.regrow.is_none())
                    && !s.world.units.iter().any(|u| u.pos == *p)
            })
            .map(|(p, _)| p)
            .unwrap();
        let hint = |tool| tooltip_lines(&s, tool, tree).unwrap().pop().unwrap();
        assert_eq!(hint(Tool::Select), Line::Hint(Hint::StartTool(Tool::Harvest)));
        assert_eq!(hint(Tool::Harvest), Line::Hint(Hint::ApplyTool(Tool::Harvest)));
    }

    #[test]
    fn a_vein_beside_a_tunnel_shows_its_richer_yield() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let gold = s.defs.resource_id("gold_vein").unwrap();
        let h = s.defs.harvest(gold).unwrap().clone();
        let (vein, rock) = s
            .world
            .map
            .iter()
            .filter(|(_, t)| t.resource == Some(gold))
            .find_map(|(p, _)| {
                let rock = Pos::new(p.x + 1, p.y);
                let bare = |t: &fn_sim::Tile| t.terrain == fn_sim::Terrain::Mountain;
                s.world
                    .map
                    .get(rock)
                    .is_some_and(|t| bare(t) && t.resource.is_none())
                    .then_some((p, rock))
            })
            .expect("a gold vein with bare rock east of it");
        let detail = |s: &Session| tooltip_lines(s, Tool::Select, vein).unwrap()[2].clone();
        assert_eq!(detail(&s), Line::Detail(format!("Mine: {} Gold Ore - 30s", h.count)));
        s.world.map[rock].terrain = fn_sim::Terrain::Tunnel;
        let more = h.tunnel_count.unwrap();
        assert_eq!(
            detail(&s),
            Line::Detail(format!("Mine: {more} Gold Ore from the tunnel - 30s"))
        );
    }

    #[test]
    fn a_site_s_tooltip_shows_materials_and_work() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let bed = s.defs.building_id("bed").unwrap();
        let pos = s
            .world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == fn_sim::Terrain::Land
                    && t.resource.is_none()
                    && !s.world.units.iter().any(|u| u.pos == *p)
                    && s.world.check_build(*p, bed, &s.defs).is_ok()
            })
            .map(|(p, _)| p)
            .unwrap();
        s.apply(Command::DesignateBuild { pos, building: bed }).unwrap();
        let lines = tooltip_lines(&s, Tool::Build, pos).unwrap();
        assert_eq!(
            lines,
            [
                Line::Title("Bed (under construction)".into()),
                Line::Detail("Materials: 0/5 Wood".into()),
                Line::Detail("Work: 0/20s".into()),
                Line::Hint(Hint::StartTool(Tool::CancelHarvest)),
            ]
        );
        let lines = tooltip_lines(&s, Tool::CancelHarvest, pos).unwrap();
        assert_eq!(lines.last(), Some(&Line::Hint(Hint::ApplyTool(Tool::CancelHarvest))));
    }

    #[test]
    fn a_finished_building_s_tooltip_names_its_anchor_from_every_tile_of_its_footprint() {
        use fn_sim::building::Building;
        use fn_sim::geom::Footprint;
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let bed = s.defs.building_id("bed").unwrap();
        let at = Pos::new(30, 30);
        let foot = Footprint::new(at, 2, 2);
        s.world.units.retain(|u| !foot.contains(u.pos));
        for p in foot.tiles() {
            let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
            let b = Building { part, size: [2, 2], ..Building::new(bed, true, false) };
            s.world.map.get_mut(p).unwrap().building = Some(b);
        }
        for p in foot.tiles() {
            let lines = tooltip_lines(&s, Tool::Select, p).unwrap();
            assert_eq!(
                lines[1],
                Line::Detail(format!("Built ({}, {})", at.x, at.y)),
                "{p:?} should name the anchor"
            );
        }
    }

    /// A study hall's tooltip says how many of its seats are filled, counting whoever is
    /// currently inside (#338): the Build modal's own line, shown before anything is built, has
    /// only the total (`build::building_detail`).
    #[test]
    fn a_study_hall_says_how_many_seats_are_filled() {
        use fn_sim::building::Building;
        use fn_sim::geom::Footprint;
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let hall = s.defs.building_id("study_hall").unwrap();
        let def = s.defs.building(hall);
        let (seats, passable) = (def.seats, def.passable);
        let at = Pos::new(30, 30);
        let foot = Footprint::new(at, 2, 2);
        s.world.units.retain(|u| !foot.contains(u.pos) && u.pos != foot.entrance());
        for p in foot.tiles() {
            let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
            let b = Building { part, size: [2, 2], ..Building::new(hall, passable, false) };
            s.world.map.get_mut(p).unwrap().building = Some(b);
        }
        let lines = tooltip_lines(&s, Tool::Select, at).unwrap();
        assert!(
            lines.contains(&Line::Detail(format!("Study: 0/{seats} seats filled"))),
            "{lines:?}"
        );

        s.world.units[0].pos = foot.entrance();
        assert!(s.world.enter_building(0, at, &s.defs));
        let lines = tooltip_lines(&s, Tool::Select, at).unwrap();
        assert!(
            lines.contains(&Line::Detail(format!("Study: 1/{seats} seats filled"))),
            "{lines:?}"
        );
    }

    /// A consumer names the source that actually powers it and the boost, or "Unpowered" out of
    /// reach (issue #428, #429); a source names its reach, already scaled for the nation's
    /// current `power_generation`, and how many buildings it's powering right now. No bundled
    /// building is a source yet (#255, #204, #200), so this gives one of its own, mutating a
    /// building's `power` and `uses_power` fields the way `world/power/tests` does for the sim.
    /// Water's `power_generation` is 1.0 (data/nations.toml), so its reach is the bare radius.
    #[test]
    fn a_consumer_names_its_source_and_a_source_names_its_reach_and_how_many_it_powers() {
        use fn_sim::building::{Building, PowerDef};
        let mut s = Session::new(7, Nation::Water, 24, 24);
        let source_id = s.defs.building_id("bed").unwrap();
        s.defs.buildings[source_id.0 as usize].power =
            Some(PowerDef { radius: 3, boost: 1.5, boost_millis: 1500 });
        let consumer_id = s.defs.building_id("earth_wall").unwrap();
        s.defs.buildings[consumer_id.0 as usize].uses_power = true;

        let source_at = Pos::new(10, 10);
        let near = Pos::new(10, 13); // 3 tiles away: in reach
        let far = Pos::new(10, 20); // out of reach
        s.world.units.retain(|u| ![source_at, near, far].contains(&u.pos));
        s.world.map.get_mut(source_at).unwrap().building =
            Some(Building::new(source_id, true, false));
        s.world.adopt_power_sources(&s.defs);
        s.world.map.get_mut(near).unwrap().building = Some(Building::new(consumer_id, true, false));
        s.world.map.get_mut(far).unwrap().building = Some(Building::new(consumer_id, true, false));

        let source_name = s.defs.building(source_id).display_name(s.world.nation).to_owned();
        let near_lines = tile_lines(&s, Tool::Select, near).unwrap();
        assert!(
            near_lines.contains(&Line::Detail(format!("Powered: {source_name}, 1.5x"))),
            "{near_lines:?}"
        );
        let far_lines = tile_lines(&s, Tool::Select, far).unwrap();
        assert!(far_lines.contains(&Line::Detail("Unpowered".to_owned())), "{far_lines:?}");

        let source_lines = tile_lines(&s, Tool::Select, source_at).unwrap();
        assert!(
            source_lines.contains(&Line::Detail("Reach: 3 tiles".to_owned())),
            "{source_lines:?}"
        );
        assert!(
            source_lines.contains(&Line::Detail("Powering: 1 building".to_owned())),
            "{source_lines:?}"
        );
    }

    #[test]
    fn an_orchard_says_how_many_trees_it_tends_and_its_saplings_say_so() {
        let mut s = Session::new(3, Nation::Air, 64, 64);
        let id = s.defs.building_id("orchard").unwrap();
        let o = s.defs.building(id).orchard.unwrap();
        let passable = s.defs.building(id).passable;
        let home = s.world.units[0].pos;
        let mut spots: Vec<Pos> = s.world.map.iter().map(|(p, _)| p).collect();
        spots.sort_by_key(|p| ((p.x - home.x).pow(2) + (p.y - home.y).pow(2), p.x, p.y));
        let open = |p: Pos| {
            let ring =
                (-4..=4).flat_map(move |dy| (-4..=4).map(move |dx| Pos::new(p.x + dx, p.y + dy)));
            ring.filter(|q| {
                s.world
                    .map
                    .get(*q)
                    .is_some_and(|t| t.terrain == fn_sim::Terrain::Land && t.resource.is_none())
            })
            .count()
        };
        spots.truncate(400);
        let pos = spots.into_iter().max_by_key(|&p| open(p)).expect("an open meadow");
        assert!(open(pos) >= 40, "the open meadow has room to plant");
        s.world.map.get_mut(pos).unwrap().building =
            Some(fn_sim::building::Building::new(id, passable, false));
        s.world.adopt_orchards(&s.defs);
        let at = |s: &Session| tooltip_lines(s, Tool::Select, pos).unwrap();
        assert!(at(&s).contains(&Line::Detail(format!("Tends 0 of {} fruit trees", o.max_trees))));

        s.tick_n(1200);
        assert!(at(&s).contains(&Line::Detail(format!("Tends 1 of {} fruit trees", o.max_trees))));
        let sapling = s.world.map.iter().map(|(p, _)| p).find(|&p| s.world.is_sapling(p)).unwrap();
        let lines = tooltip_lines(&s, Tool::Select, sapling).unwrap();
        assert!(lines.contains(&Line::Detail("Sapling - repeating order".into())), "{lines:?}");
        assert!((2..=4).contains(&lines.len()));
    }

    #[test]
    fn bending_status_line_names_a_lit_flame() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let pos = s.world.units[0].pos;
        let ability = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        s.apply(Command::Bend { ability, pos }).expect("a unit's own tile validates");
        for _ in 0..20_000 {
            s.tick();
            if s.world.is_lit(pos) {
                break;
            }
        }
        assert!(s.world.is_lit(pos), "the flame should finish lighting");
        let line = bending_status_line(&s.world, &s.defs, pos).expect("a lit flame");
        assert!(line.starts_with("Lit:"), "{line}");
        assert_eq!(bending_status_line(&s.world, &s.defs, Pos::new(-1, -1)), None);
    }

    /// A shelter (Earth Tent, Air Shield, #362) reads like a flame's line, but names itself
    /// instead of "Lit": "Earth Tent: 12m 00s left", not "Lit: 12m 00s left".
    #[test]
    fn bending_status_line_names_a_lit_shelter_by_its_own_ability() {
        let mut s = Session::new(2, Nation::Earth, 48, 32);
        let tech = s.defs.tech_id("earth_shaping").unwrap();
        s.apply(Command::SetResearch { tech }).unwrap();
        for _ in 0..60_000 {
            if s.world.research().is_known(tech) {
                break;
            }
            s.tick();
        }
        assert!(s.world.research().is_known(tech), "researched");

        let pos = s.world.units[0].pos;
        s.world.units[0].chi = s.defs.chi.max;
        let ability = s.defs.ability_id("earth_tent").unwrap();
        s.apply(Command::Bend { ability, pos }).expect("a unit's own tile validates");
        for _ in 0..20_000 {
            s.tick();
            if s.world.is_lit(pos) {
                break;
            }
        }
        assert!(s.world.is_lit(pos), "the tent should finish raising");
        let line = bending_status_line(&s.world, &s.defs, pos).expect("a lit shelter");
        assert!(line.starts_with("Earth Tent:"), "{line}");
        assert!(!line.starts_with("Lit:"), "{line}");
    }

    /// A resting wild plant names its regrow time and, once a pick has left it tired, its
    /// strain (#296, #299, docs/design/difficulty.md "Readable simulation").
    #[test]
    fn a_resting_wild_plant_shows_its_regrow_time_and_strain() {
        let defs = Defs::bundled();
        let fruit = defs.resource_id("fruit_tree").unwrap();
        let map = field(3, 3, 128, Some(fruit));
        let mut world = World::with_map(1, map, Nation::Earth, &[Pos::new(0, 0)], &defs);
        world.set_difficulty(Some("normal".to_owned()));
        let pos = Pos::new(1, 1);
        world.apply(Command::DesignateHarvest { pos }, &defs).unwrap();
        for _ in 0..2000 {
            world.tick(&defs);
            if world.is_resting(pos) {
                break;
            }
        }
        assert!(world.is_resting(pos), "the pick should leave it resting");
        assert!(world.strain_at(pos).unwrap() > 0.0, "and tired, on Normal");
        let s = custom_session(defs, world);
        let lines = tooltip_lines(&s, Tool::Select, pos).unwrap();
        let growing = lines
            .iter()
            .find_map(|l| match l {
                Line::Detail(t) if t.starts_with("Growing") => Some(t.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("a Growing line in {lines:?}"));
        assert!(growing.contains("ripe in"), "{growing}");
        assert!(
            lines.iter().any(|l| matches!(l, Line::Detail(t) if t.starts_with("Tired (strain"))),
            "{lines:?}"
        );
    }

    /// A bare tile names its climate band's winter effects once a difficulty preset turns
    /// seasons on, and says nothing extra without one (#299).
    #[test]
    fn a_tile_names_its_climate_band_once_seasons_are_on() {
        let defs = Defs::bundled();
        let map = field(3, 3, 0, None); // polar: cold and frost
        let pos = Pos::new(1, 1);
        let plain = World::with_map(1, map.clone(), Nation::Earth, &[Pos::new(0, 0)], &defs);
        let s = custom_session(defs.clone(), plain);
        let lines = tooltip_lines(&s, Tool::Select, pos).unwrap();
        assert!(
            !lines.iter().any(|l| matches!(l, Line::Detail(t) if t.contains("climate"))),
            "seasons off: {lines:?}"
        );

        let mut seasonal = World::with_map(1, map, Nation::Earth, &[Pos::new(0, 0)], &defs);
        seasonal.set_difficulty(Some("normal".to_owned()));
        let s = custom_session(defs, seasonal);
        let lines = tooltip_lines(&s, Tool::Select, pos).unwrap();
        let climate = s.defs.seasons.climate_for(0);
        assert!(
            lines
                .iter()
                .any(|l| matches!(l, Line::Detail(t) if t == &format!("{} climate: cold and frost in winter", climate.name))),
            "{lines:?}"
        );
    }

    /// A unit standing on a cold band in winter, with no fire near, reads "Cold" in the slot
    /// Inner Fire's own warmth uses (#299): the two never show together, since being heated is
    /// what rules cold out.
    #[test]
    fn a_cold_unit_s_status_names_the_cold() {
        let defs = Defs::bundled();
        let map = field(3, 3, 0, None); // polar: cold
        let pos = Pos::new(0, 0);
        let mut world = World::with_map(1, map, Nation::Earth, &[pos], &defs);
        world.set_difficulty(Some("normal".to_owned()));
        let season_ticks = (f64::from(defs.seasons.calendar.season_minutes)
            * 60.0
            * f64::from(fn_sim::world::TICKS_PER_SECOND)) as u64;
        world.tick = season_ticks * 3; // winter
        assert!(world.season(&defs).winter);
        assert!(world.is_cold(pos, &defs));
        let s = custom_session(defs, world);
        let lines = tooltip_lines(&s, Tool::Select, pos).unwrap();
        assert!(
            lines.iter().any(
                |l| matches!(l, Line::Detail(t) if t.ends_with("Cold: tiring faster (no fire near)"))
            ),
            "{lines:?}"
        );
    }

    /// A woods-and-a-stockpile map: a stockpile zone at `(1, 2)`, an oak tree at `(7, 2)`, a
    /// stone outcrop at `(4, 1)` (`Terrain::Mountain`, worked from the land tile beside it), the
    /// settler at `(3, 2)`.
    fn woods_and_stockpile(defs: &Defs) -> (World, Pos, Pos, Pos) {
        let oak = defs.resource_id("oak_tree").unwrap();
        let stone = defs.resource_id("stone").unwrap();
        let map = Grid::from_fn(10, 3, |p| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if p == Pos::new(4, 1) { Terrain::Mountain } else { Terrain::Land },
            biome: BiomeId(0),
            resource: match p {
                q if q == Pos::new(7, 2) => Some(oak),
                q if q == Pos::new(4, 1) => Some(stone),
                _ => None,
            },
            building: None,
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(3, 2)], defs);
        (world, Pos::new(1, 2), Pos::new(7, 2), Pos::new(4, 1))
    }

    /// A stockpile tile's own line (#337, "stockpile capacity is visible somewhere"): "empty"
    /// with nothing in it yet, else "n/cap Item".
    #[test]
    fn a_stockpile_tile_names_what_it_holds_and_its_capacity() {
        let defs = Defs::bundled();
        let (mut world, zone, tree, _) = woods_and_stockpile(&defs);
        world.apply(Command::DesignateStockpile { pos: zone }, &defs).unwrap();
        let s = custom_session(defs.clone(), world);
        let lines = tooltip_lines(&s, Tool::Select, zone).unwrap();
        assert!(
            lines.contains(&Line::Detail("Stockpile: empty".to_owned())),
            "nothing stored yet: {lines:?}"
        );

        let mut s = s;
        s.apply(Command::DesignateHarvest { pos: tree }).unwrap();
        let wood = s.defs.item_id("wood").unwrap();
        for _ in 0..3000 {
            s.tick();
            if s.world.stored_total(wood) > 0 {
                break;
            }
        }
        let count = s.world.stored_total(wood);
        assert!(count > 0, "wood should have reached the stockpile");
        // The hauler stands right on the zone tile to deposit it; step past that so the
        // tooltip reads the tile, not the unit standing on it.
        for _ in 0..200 {
            if s.world.units_at(zone).next().is_none() {
                break;
            }
            s.tick();
        }
        assert!(s.world.units_at(zone).next().is_none(), "the hauler should have moved on");
        let cap = s.defs.jobs.stockpile_stack;
        let lines = tooltip_lines(&s, Tool::Select, zone).unwrap();
        assert!(
            lines.contains(&Line::Detail(format!("Stockpile: {count}/{cap} Wood"))),
            "{lines:?}"
        );
    }

    /// An item stack on the ground names itself, and -- only once every stockpile is full --
    /// says why it's still there (#337, the playtest evidence that made this the issue's
    /// highest-value fix).
    #[test]
    fn a_ground_stack_names_itself_and_a_full_stockpile_blocking_it() {
        let defs = Defs::bundled();
        let (mut world, zone, tree, quarry) = woods_and_stockpile(&defs);
        world.apply(Command::DesignateStockpile { pos: zone }, &defs).unwrap();
        world.apply(Command::DesignateHarvest { pos: quarry }, &defs).unwrap();
        let stone = defs.item_id("stone").unwrap();
        let wood = defs.item_id("wood").unwrap();
        let mut s = custom_session(defs, world);
        for _ in 0..3000 {
            s.tick();
            if s.world.stored_total(stone) > 0 {
                break;
            }
        }
        assert!(s.world.stored_total(stone) > 0, "stone should have reached the stockpile");
        s.apply(Command::DesignateHarvest { pos: tree }).unwrap();
        let mut dropped = None;
        for _ in 0..3000 {
            s.tick();
            if let Some((p, _)) = s.world.ground_items().find(|(_, st)| st.item == wood) {
                dropped = Some(p);
                break;
            }
        }
        let dropped = dropped.expect("wood should have been chopped and dropped");
        let lines = tooltip_lines(&s, Tool::Select, dropped).unwrap();
        let line = lines
            .iter()
            .find_map(|l| match l {
                Line::Detail(t) if t.starts_with("Lying here") => Some(t.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("a ground-stack line in {lines:?}"));
        assert!(line.starts_with("Lying here: 4 Wood"), "{line}");
        assert!(line.ends_with("(unreachable: every stockpile is full)"), "{line}");
    }

    /// A harvest designated across water nobody can cross flags itself unreachable once the
    /// connectivity cache is warm (#337).
    #[test]
    fn an_unreachable_harvest_designation_flags_itself() {
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
        let mut world = World::with_map(1, map, Nation::Earth, &[Pos::new(3, 2)], &defs);
        let island = Pos::new(1, 1);
        world.apply(Command::DesignateHarvest { pos: island }, &defs).unwrap();
        let mut s = custom_session(defs, world);
        assert!(
            tooltip_lines(&s, Tool::Select, island)
                .unwrap()
                .iter()
                .all(|l| l != &Line::Detail("Unreachable: no settler can reach it".to_owned())),
            "the connectivity cache isn't warm yet"
        );
        s.tick();
        let lines = tooltip_lines(&s, Tool::Select, island).unwrap();
        assert!(
            lines.contains(&Line::Detail("Unreachable: no settler can reach it".to_owned())),
            "{lines:?}"
        );
    }
}
