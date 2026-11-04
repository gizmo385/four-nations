//! What is under the cursor: the lines of the hover tooltip (the Bevy client) or the Cursor
//! section (the terminal UI).

use fn_sim::{Pos, World};

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

/// "Lit: 12m 03s left" for a flame on `tile`, or "Wet: sprouts in 3m 20s" for a watered one.
/// `None` for anything else.
pub fn bending_status_line(world: &World, tile: Pos) -> Option<String> {
    if let Some(flame) = world.flames().into_iter().find(|f| f.pos == tile) {
        return Some(format!("Lit: {} left", clock(flame.seconds_left)));
    }
    let (_, left) = world.wet_tiles().into_iter().find(|(p, _)| *p == tile)?;
    Some(format!("Wet: sprouts in {}", clock(left)))
}

/// The tooltip for what is under the cursor at `tile`: up to 4 lines (name, context, a yield or
/// reason, then an action hint). `None` when `tile` is off the map.
pub fn tooltip_lines(session: &Session, tool: Tool, tile: Pos) -> Option<Lines> {
    let (world, defs) = (&session.world, &session.defs);
    let t = world.map.get(tile)?;
    let mut lines = Lines::new();
    // `units_at` skips a unit that is inside a building (#226): it isn't standing there as far
    // as the map is concerned, so hovering its door falls through to the building below instead.
    if let Some(unit) = world.units_at(tile).next() {
        lines.push(Line::Title(unit.name.clone()));
        let task = describe_task(world, defs, unit);
        // Inner Fire (#123) rides on the unit: its time left joins the line, since a tooltip
        // tops out at four lines.
        let warm = world
            .glows()
            .into_iter()
            .find(|g| g.unit == unit.id)
            .map_or(String::new(), |g| format!(" - Warm {}", clock(g.seconds_left)));
        lines.push(Line::Detail(format!(
            "{} - {task}{warm}",
            defs.nation(world.nation_of(unit)).name
        )));
        // Finished bends (#233) share the carrying line, for the same four-line reason.
        lines.push(Line::Detail(format!(
            "Carrying: {} - Bends: {}",
            describe_stack(defs, unit.carrying),
            unit.bends()
        )));
        if tool == Tool::Select {
            lines.push(Line::Hint(Hint::SelectUnit));
        }
        return Some(lines);
    }
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
        // An orchard says how much of its grove is planted (#142).
        let max =
            world.building_at(tile).and_then(|b| defs.building(b).orchard).map(|o| o.max_trees);
        if let (Some(trees), Some(max)) = (world.orchard_trees(tile), max) {
            lines.push(Line::Detail(format!("Tends {trees} of {max} trees")));
        }
        return Some(lines);
    }
    let biome = &defs.biome(t.biome).name;
    let at = format!("{biome} - {:?} ({}, {})", t.terrain, tile.x, tile.y);
    let Some(res) = t.resource else {
        lines.push(Line::Title(format!("{:?}", t.terrain)));
        lines.push(Line::Detail(format!("{biome} ({}, {})", tile.x, tile.y)));
        lines.push(Line::Detail(format!("Elevation {}", t.elevation)));
        // A lit flame or a wet patch (#72): the result of a bend, so hovering explains it
        // (pillar 4, "readable simulation").
        if let Some(status) = bending_status_line(world, tile) {
            lines.push(Line::Detail(status));
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
            let text = if repeating { "Growing back - repeating order" } else { "Growing back" };
            lines.push(Line::Detail(text.into()));
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
    }
    Some(lines)
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use fn_sim::world::Command;

    use super::*;
    use crate::tools::ability_for_nation;

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

    #[test]
    fn a_unit_only_offers_to_be_selected_with_the_select_tool() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let at = s.world.units[0].pos;
        let lines = tooltip_lines(&s, Tool::Harvest, at).unwrap();
        assert_eq!(lines.len(), 3);
        assert!(!lines.contains(&Line::Hint(Hint::SelectUnit)));
    }

    #[test]
    fn a_unit_s_tooltip_counts_its_finished_bends() {
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
        assert_eq!(caster.bends(), 1);
        let lines = tooltip_lines(&s, Tool::Select, caster.pos).unwrap();
        let carrying = format!("Carrying: {} - Bends: 1", describe_stack(&s.defs, caster.carrying));
        assert_eq!(lines[2], Line::Detail(carrying), "{lines:?}");
        assert_eq!(lines.len(), 4, "still four lines");
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
        assert!(at(&s).contains(&Line::Detail(format!("Tends 0 of {} trees", o.max_trees))));

        s.tick_n(1200);
        assert!(at(&s).contains(&Line::Detail(format!("Tends 1 of {} trees", o.max_trees))));
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
        let line = bending_status_line(&s.world, pos).expect("a lit flame");
        assert!(line.starts_with("Lit:"), "{line}");
        assert_eq!(bending_status_line(&s.world, Pos::new(-1, -1)), None);
    }
}
