// Reference governor (#564): the campaign champion of the water-3h track with the waits it was
// scattered with removed (a wait was thought to reset the operation count, and did not), as one
// loop with a single wait per pass. A realistic size and shape for what a model writes: about
// 45,000 operations and 40 acts in its heaviest pass. Used only by
// crates/agent/tests/reference_governor.rs; it is never shown to an agent.
let camp = memory.camp ?? observe().cursor;
memory.camp = camp;
memory.stone_at = memory.stone_at ?? 0;

// Research: take the next Available tech in the plan when none is set.
fn research_duty(queue) {
    if !observe().research.label.contains("none set") {
        return;
    }
    let open = [];
    for col in screen("research").columns {
        for t in col {
            if t.state == "Available" {
                open.push(t.name);
            }
        }
    }
    for tech in queue {
        if open.contains(tech) {
            act([#{step: "research", tech: tech}], "research");
            return;
        }
    }
}

// Stockpile: grow the pile in blocks east and south of camp while tiles
// are few. Each block is tried once.
fn stockpile_duty(camp, want) {
    if observe().stockpile.tiles >= want {
        return;
    }
    for i in 0..6 {
        let step = #{
            step: "designate",
            tool: "stockpile",
            from: #{x: camp.x + 4 + i * 4, y: camp.y - 3},
            to: #{x: camp.x + 7 + i * 4, y: camp.y - 1},
        };
        act([step], "stockpile");

    }
    for i in 0..6 {
        let step = #{
            step: "designate",
            tool: "stockpile",
            from: #{x: camp.x - 6 + i * 4, y: camp.y + 3},
            to: #{x: camp.x - 3 + i * 4, y: camp.y + 5},
        };
        act([step], "stockpile");

    }
}

// Food: standing harvest marks on cabbage, fruit and fishing spots.
fn food_duty(camp) {
    designate_near("harvest", "Cabbage", camp, 19, true);

    designate_near("harvest", "Fruit Tree", camp, 19, true);

    designate_near("harvest", "Fishing Spot", camp, 19, true);
}

// Ore: mark gold veins for mining until the store holds `want`.
fn ore_duty(camp, want) {
    if store("Gold Ore") < want {
        designate_near("harvest", "Gold Vein", camp, 19, false);
    }
}

// Tunnels: try the Bore Tunnel bend on gold veins, the likely third
// form for masters. Logs the first refusal so the reason is visible.
fn tunnel_duty(camp) {
    let veins = find("Gold Vein", camp, 19);

    let i = 0;
    while i < 3 && i < veins.len() {
        let v = veins[i];
        let step = #{
            step: "designate",
            tool: "bend",
            from: #{x: v.x, y: v.y},
            to: #{x: v.x, y: v.y},
            ability: "Bore Tunnel",
        };
        let r = act([step], "tunnel");
        if i == 0 {
            log(`tunnel: ${r.error} ${r.notices}`);
        }

        i += 1;
    }
}

// Is this cell open ground (plains or desert, nothing on it)?
fn is_open(c) {
    c.thing == "-" && (c.ground == "Plains" || c.ground == "Desert")
}

// Open ground tiles with water next to them, nearest to camp first.
// Channeling water onto such ground is what the bend tool needs.
fn shore_tiles(camp, limit) {
    let grid = #{};
    for c in cells(camp.x - 10, camp.y - 6, 21, 13) {
        grid[`${c.x},${c.y}`] = c;
    }

    let found = [];
    for c in grid.values() {
        if !is_open(c) {
            continue;
        }
        for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let n = grid[`${c.x + d[0]},${c.y + d[1]}`];
            if n != () && (n.ground == "lake" || n.ground == "coast") {
                found.push(c);
                break;
            }
        }
    }

    found.sort(|a, b| {
        let da = (a.x - camp.x).abs() + (a.y - camp.y).abs();
        let db = (b.x - camp.x).abs() + (b.y - camp.y).abs();
        da - db
    });
    if found.len() > limit {
        found.truncate(limit);
    }
    found
}

// Bending: channel the nation's ability onto shore tiles near camp, so
// benders earn credit toward the masters sign.
fn bend_duty(camp, ability) {
    let tiles = shore_tiles(camp, 6);

    for t in tiles {
        let step = #{
            step: "designate",
            tool: "bend",
            from: #{x: t.x, y: t.y},
            to: #{x: t.x, y: t.y},
        };
        if ability != "" {
            step.ability = ability;
        }
        act([step], "bend");

    }
}

// Mark at most `n` trees at a time, so wood does not flood the piles.
fn chop_some(camp, n) {
    let trees = find("Oak Tree", camp, 19);

    let i = 0;
    while i < n && i < trees.len() {
        let t = trees[i];
        let step = #{
            step: "designate",
            tool: "harvest",
            from: #{x: t.x, y: t.y},
            to: #{x: t.x, y: t.y},
        };
        act([step], "chop");

        i += 1;
    }
}

// Wood and stone: keep a few trees and outcrops marked for building.
// Stone needs Stoneworking, so a refusal delays the retry 20 turns.
// Returns the turn stone may be tried again.
fn material_duty(camp, targets, stone_at) {
    if store("Wood") < targets.wood_cap {
        chop_some(camp, 10);
    }
    let now = observe().turn;
    if store("Stone") < targets.stone_cap && now >= stone_at {
        let r = designate_near("harvest", "Stone Outcrop", camp, 19, false);

        let b = designate_near("harvest", "Boulder", camp, 19, false);
        if !r.ok && !b.ok {
            return now + 20;
        }
    }
    stone_at
}

// Place a building on the nearest tile can_build accepts. A locked
// building is refused up front. Returns true if placed.
fn place_near(building, camp) {
    let probe = can_build(building, camp.x, camp.y);
    if !probe.ok && probe.reason.starts_with("Requires") {
        return false;
    }
    let best = ();
    let best_d = 9999;
    for y in (camp.y - 5)..(camp.y + 7) {
        for x in (camp.x - 9)..(camp.x + 8) {
            let d = (x - camp.x).abs() + (y - camp.y).abs();
            if d < best_d && can_build(building, x, y).ok {
                best = #{x: x, y: y};
                best_d = d;
            }
        }

    }
    if best == () {
        return false;
    }
    let step = #{
        step: "build",
        building: building,
        from: best,
        to: best,
    };
    act([step], "build " + building).error == ()
}

// Beds: build one near camp while beds trail people + targets.beds_ahead.
fn beds_duty(targets, camp) {
    let beds = find("Bed", camp, 15).len();

    let wanted = observe().people.total + targets.beds_ahead;
    if beds < wanted && store("Wood") >= 5 {
        place_near("Bed", camp);
    }
}

// Farms: once Farming is known, keep targets.farms plots and sow them.
fn farm_duty(targets, camp) {
    let plots = find("Farm Plot", camp, 15).len();

    if plots < (targets.farms ?? 0) {
        place_near("Farm Plot", camp);
    }
    if plots > 0 {
        designate_near("sow", "Farm Plot", camp, 15, false);
    }
}

// Build queue: place the first plan().build entry not yet standing.
// A locked or unplaceable entry is skipped so it cannot block the rest.
fn build_duty(queue, camp) {
    for entry in queue {
        let key = entry.key ?? entry.building;
        let have = find(key, camp, 15).len();

        if have == 0 {
            if place_near(entry.building, camp) {
                return;
            }
        }
    }
}

// One pass of every duty, then a wait, for the whole game in one run. A
// pass is the most work this governor does between two waits.
loop {
    let p = plan();
    research_duty(p.research);
    stockpile_duty(camp, p.targets.stockpile_tiles ?? 20);
    food_duty(camp);
    ore_duty(camp, p.targets.ore ?? 40);
    tunnel_duty(camp);
    memory.stone_at = material_duty(camp, p.targets, memory.stone_at);
    bend_duty(camp, (p.bends ?? [""])[0]);
    beds_duty(p.targets, camp);
    farm_duty(p.targets, camp);
    build_duty(p.build ?? [], camp);
    wait(3);
}
