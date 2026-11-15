// Governor: one loop of duties, numbers from plan().
let p = plan();
let t = p.targets;

// Camp centre: mean position of settlers at start.
fn camp_centre() {
    let o = observe();
    let c = #{x: o.cursor.x, y: o.cursor.y};
    let s = find("settler", c, 19);
    if s.len() == 0 {
        return c;
    }
    let sx = 0;
    let sy = 0;
    for a in s {
        sx += a.x;
        sy += a.y;
    }
    #{x: sx / s.len(), y: sy / s.len()}
}

// One scan of the camp area, bucketed by thing name (lower case).
fn scan(c) {
    let b = #{};
    for k in cells(c.x - 19, c.y - 19, 39, 39) {
        let key = k.thing.to_lower();
        if key == "-" {
            continue;
        }
        if b.contains(key) {
            b[key].push(k);
        } else {
            b[key] = [k];
        }
    }
    b
}

// Cells whose thing name contains desc (case-insensitive).
fn pick(b, desc) {
    let d = desc.to_lower();
    let out = [];
    for key in b.keys() {
        if key.contains(d) {
            out += b[key];
        }
    }
    out
}

// The same cells sorted nearest camp first.
fn near_first(a, c) {
    let cx = c.x;
    let cy = c.y;
    a.sort(|p, q| (p.x - cx).abs() + (p.y - cy).abs()
        - (q.x - cx).abs() - (q.y - cy).abs());
    a
}

// Whether a cell already carries a work mark (wet/cursor don't count).
fn busy(x) {
    for m in x.marks {
        if m != "wet" && m != "cursor" {
            return true;
        }
    }
    false
}

// Whether a ground name is water (lake, river, sea, ...).
fn watery(g) {
    let g = g.to_lower();
    g.contains("lake") || g.contains("river") || g.contains("sea")
        || g.contains("ocean") || g.contains("water")
        || g.contains("mountain")
}

// Whether no settler can reach a cell (its inspect lines say so).
fn unreachable(x) {
    for line in inspect(x.x, x.y).lines {
        if line.contains("nreachable") || line.contains("can reach") {
            return true;
        }
    }
    false
}

// Whether a tech is known (research screen card state).
fn known(tech) {
    for col in screen("research").columns {
        for card in col {
            if card.name == tech {
                return card.state == "Known";
            }
        }
    }
    false
}

// Research: first queued tech that is Available and actually takes
// (some Available ones refuse, e.g. Northern City), when none is set.
fn do_research(queue) {
    let o = observe();
    if !o.research.label.contains("none") {
        return;
    }
    let r = screen("research");
    for name in queue {
        for col in r.columns {
            for card in col {
                if card.name == name && card.state == "Available" {
                    act([#{step: "research", tech: name}], "research");
                    if !observe().research.label.contains("none") {
                        log("research " + name);
                        return;
                    }
                }
            }
        }
    }
}

// Bend targets: [dry cells within 2 of fresh water, shore water cells
// (water with dry ground beside it, which Cut Ice needs)].
fn wet_spots(c) {
    let cs = cells(c.x - 19, c.y - 19, 39, 39);
    let water = #{};
    let wcells = [];
    for k in cs {
        if k.ground.contains("river") || k.ground.contains("lake") {
            water[`${k.x},${k.y}`] = true;
            wcells.push(k);
        }
    }
    let shore = [];
    for k in wcells {
        let dry = 0;
        for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            if !water.contains(`${k.x + d[0]},${k.y + d[1]}`) {
                dry += 1;
            }
        }
        if dry > 0 && !busy(k) {
            shore.push(k);
        }
    }
    let out = [];
    for k in cs {
        if k.thing != "-" || busy(k) {
            continue;
        }
        if water.contains(`${k.x},${k.y}`) {
            continue;
        }
        let near = false;
        for dx in [-2, 0, 2] {
            for dy in [-2, 0, 2] {
                if water.contains(`${k.x + dx},${k.y + dy}`) {
                    near = true;
                }
            }
        }
        if near {
            out.push(k);
        }
    }
    [near_first(out, c), near_first(shore, c)]
}

// Wide scan within 45 of camp: timber trees, outcrops, boulders, gold.
fn wide_scan(c) {
    let w = #{wood: [], stone: [], gold: []};
    for k in cells(c.x - 45, c.y - 45, 64, 40)
        + cells(c.x + 19, c.y - 45, 27, 40)
        + cells(c.x - 45, c.y - 5, 64, 40)
        + cells(c.x + 19, c.y - 5, 27, 40)
        + cells(c.x - 45, c.y + 35, 64, 11)
        + cells(c.x + 19, c.y + 35, 27, 11) {
        let th = k.thing;
        if th.ends_with(" Tree") && !th.starts_with("Fruit") {
            w.wood.push(k);
        } else if th == "Stone Outcrop" || th == "Boulder" {
            w.stone.push(k);
        } else if th == "Gold Vein" {
            w.gold.push(k);
        }
    }
    w.wood = near_first(w.wood, c);
    w.stone = near_first(w.stone, c);
    w.gold = near_first(w.gold, c);
    w
}

// Up to 8 reachable cells of a list, inspecting at most 10 new ones;
// returns [reachable cells, updated set of unreachable "x,y" keys].
fn reachable(list, bad) {
    let ok = [];
    let looked = 0;
    for x in list {
        let key = `${x.x},${x.y}`;
        if ok.len() >= 8 || looked >= 10 {
            break;
        }
        if bad.contains(key) {
            continue;
        }
        looked += 1;
        if unreachable(x) {
            bad[key] = true;
        } else {
            ok.push(x);
        }
    }
    [ok, bad]
}

// Scrolls and charted ruins (Lost/Great Ruins, searched for scrolls)
// in one 64x40 chunk (i of 28) of the map, not yet marked. One chunk
// per loop: a whole-map scan in one loop ran away (seed 7, t375).
fn scroll_scan(i) {
    let scrolls = [];
    let x = (i % 4) * 64;
    let y = (i / 4) * 40;
    for k in cells(x, y, 64, 40) {
        let th = k.thing;
        if (th == "Ancient Scroll" || th.ends_with(" Ruins"))
            && !busy(k) {
            scrolls.push(k);
        }
    }
    scrolls
}

// Scrolls: mark every unmarked scroll and ruin for recovery.
fn do_scrolls(scrolls) {
    let steps = [];
    let names = [];
    for s in scrolls {
        steps.push(#{step: "designate", tool: "harvest",
            from: #{x: s.x, y: s.y}, to: #{x: s.x, y: s.y}});
        names.push(s.thing);
    }
    if steps.len() > 0 {
        let r = act(steps, "scrolls");
        log(`scrolls: ${names} marked, ${r.notices[0].notice}`);
    }
}

// A building spot centre: camp first, then a ring `r` tiles around it.
fn spot_centre(c, i, r) {
    let ring = [[0, 0], [1, 0], [0, 1], [-1, 0], [0, -1],
        [1, 1], [-1, 1], [1, -1], [-1, -1]];
    let d = ring[i % ring.len()];
    #{x: c.x + d[0] * r, y: c.y + d[1] * r}
}

// Free origins of size x size near `at` whose whole square is dry.
fn dry_origins(at, size, most) {
    let wet = #{};
    for k in cells(at.x - 16, at.y - 16, 36, 36) {
        if watery(k.ground) {
            wet[`${k.x},${k.y}`] = true;
        }
    }
    let spots = [];
    for s in free_origins(at, 16, size, size) {
        if spots.len() >= most {
            break;
        }
        let dry = true;
        for dx in 0..size {
            for dy in 0..size {
                if wet.contains(`${s.x + dx},${s.y + dy}`) {
                    dry = false;
                }
            }
        }
        if dry {
            spots.push(s);
        }
    }
    spots
}

// Stockpile: keep free tiles available; try up to 4 spread-out spots,
// skipping any 3x3 that touches water or mountain.
fn do_stockpile(c, t, n) {
    let o = observe();
    let sp = o.stockpile;
    let free = sp.tiles - sp.used;
    if free >= t.stockpile_free {
        return;
    }
    let spots = dry_origins(c, 3, 60);
    let m = spots.len();
    for j in 0..4 {
        if m == 0 {
            log("stockpile: no dry spot");
            return;
        }
        let s = spots[(n * 7 + j * 13) % m];
        let r = act([#{step: "designate", tool: "stockpile",
            from: #{x: s.x, y: s.y},
            to: #{x: s.x + 2, y: s.y + 2}}], "stockpile");
        let note = r.notices[0].notice;
        if !note.contains(": 0 done") {
            log("stockpile " + note);
            return;
        }
        if j == 3 {
            let g = cells(s.x, s.y, 1, 1)[0].ground;
            log(`stockpile refused on ${g}: ${note}`);
        }
    }
}

// Total food in stores.
fn food_total() {
    store("Fish") + store("Cabbage") + store("Fruit")
        + store("Grain") + store("Food") + store("Rice")
        + store("Vegetables") + store("Meat")
}

// Food: harvest every wild food and fish near camp, standing orders.
fn do_food(c, b, t) {
    if food_total() >= t.food_cap {
        return;
    }
    let steps = [];
    for d in ["fishing spot", "wild cabbage", "fruit tree"] {
        for x in near_first(pick(b, d), c) {
            if !busy(x) && steps.len() < 6 {
                steps.push(#{step: "designate", tool: "harvest",
                    from: #{x: x.x, y: x.y}, to: #{x: x.x, y: x.y},
                    standing: true});
            }
        }
    }
    if steps.len() > 0 {
        act(steps, "food");
    }
}

// Farms: build plots near camp up to target, sow unsown ones.
fn do_farms(c, b, t) {
    let plots = pick(b, "farm plot");
    let steps = [];
    for x in plots {
        if !busy(x) && x.thing == "Farm Plot" && steps.len() < 6 {
            steps.push(#{step: "designate", tool: "sow",
                from: #{x: x.x, y: x.y}, to: #{x: x.x, y: x.y}});
        }
    }
    if steps.len() > 0 {
        let r = act(steps, "sow");
        if r.notices[0].notice.contains(": 0 done") {
            log("sow " + r.notices[0].notice);
        }
    }
    if plots.len() < t.farm_plots && store("Wood") >= 6 {
        let r = build_near("Farm Plot", c);
        log("plot " + r.msg);
    }
}

// Mark up to n tiles of a list for harvest, nearest first.
fn harvest_list(found, desc, n) {
    let steps = [];
    let marked = 0;
    for x in found {
        if steps.len() + marked >= n {
            break;
        }
        if busy(x) {
            marked += 1;
        } else {
            steps.push(#{step: "designate", tool: "harvest",
                from: #{x: x.x, y: x.y}, to: #{x: x.x, y: x.y}});
        }
    }
    if steps.len() > 0 {
        let r = act(steps, "harvest " + desc);
        let at = steps[0].from;
        log(`${desc}: ${found.len()} found, at ${at.x},${at.y} `
            + r.notices[0].notice.sub_string(40));
    }
    marked + steps.len()
}

// Wood, stone and ore: keep some tiles marked while under target.
fn do_materials(t, k, w) {
    if store("Wood") < t.wood {
        harvest_list(w.wood, "tree", t.chop);
    }
    if store("Stone") < t.stone && k.stone {
        harvest_list(w.stone, "stone", t.quarry);
    }
    if store("Gold Ore") < t.ore && k.ore {
        harvest_list(w.gold, "gold vein", t.mine);
    }
}

// A 1x1 building on a tile with both coordinates even, nearest camp
// first, so no two such buildings touch and lanes stay open; when
// none is left nearby, build_near's own nearest spot instead.
fn build_spaced(name, c) {
    let checked = 0;
    for s in free_origins(c, 16, 1, 1) {
        if s.x % 2 != 0 || s.y % 2 != 0 {
            continue;
        }
        checked += 1;
        if checked > 30 {
            break;
        }
        if can_build(name, s.x, s.y).ok {
            let r = act([#{step: "build", building: name,
                from: #{x: s.x, y: s.y}, to: #{x: s.x, y: s.y}}], name);
            return r.notices[0].notice;
        }
    }
    build_near(name, c).msg
}

// Beds: keep beds ahead of people.
fn do_beds(c, b, t) {
    let o = observe();
    let beds = pick(b, "bed").len();
    if beds < o.people.total + t.beds_ahead && store("Wood") >= 5 {
        log("bed " + build_spaced("Bed", c));
    }
}

// Study: more Elders' Lodges (2x2, so 4 cells each) for faster research.
fn do_lodges(c, b, t, n) {
    let have = pick(b, "elders' lodge").len() / 4;
    if have >= 1 && have < t.lodges && store("Wood") >= 20 {
        let r = build_near("Elders' Lodge", spot_centre(c, n, 8));
        log("lodge " + r.msg);
    }
}

// Tools: a Workshop that makes none while sites wait on Tools (seed
// 7: "Making: Tools", 0 in stores for 300 turns) gets a second one
// elsewhere; at most two Workshops (2x2, so 8 cells).
fn do_tools(c, b, n) {
    let ws = pick(b, "workshop");
    if store("Tool") > 0 || ws.len() == 0 || ws.len() >= 8 {
        return;
    }
    if store("Wood") < 20 || store("Stone") < 10 {
        return;
    }
    let r = build_near("Workshop", spot_centre(c, n + 1, 10));
    log("2nd workshop " + r.msg);
}

// Stuck sites: a build site no settler can reach, seen for 40 loops
// (~120 turns), or any site seen for 150 loops, is taken away and
// forgotten so do_build places it again elsewhere. Sites merely
// waiting on Tools are left alone: removing them threw away the
// Tools already delivered (seed 7: six sites cycled, 0 Tools).
fn do_stuck_sites(b, ages, built) {
    let seen = #{};
    for key in b.keys() {
        if !key.contains("(site)") {
            continue;
        }
        let x = b[key][0];
        let age = if ages.contains(key) { ages[key] + 1 } else { 1 };
        seen[key] = age;
        if age >= 150 || (age >= 40 && age % 10 == 0 && unreachable(x)) {
            let why = inspect(x.x, x.y).lines;
            log(`stuck ${key}: ${why}`);
            act([#{step: "designate", tool: "remove",
                from: #{x: x.x, y: x.y}, to: #{x: x.x, y: x.y}}],
                "stuck site");
            seen.remove(key);
            for name in built.keys() {
                if key.contains(name.to_lower()) {
                    built.remove(name);
                }
            }
        }
    }
    [seen, built]
}

// Build queue: the first missing, unlocked entry in plan.build gets
// one try per loop; water buildings go by water (alternating a shore
// water cell and a dry cell near water, as maps differ in which one
// works), waystations 24 out (away from stockpiles), the rest near
// camp or a ring 8 around it. `built` holds names placed already (far
// ones fall outside the camp scan); `fails` counts refusals, and a
// name refused 3 times rests until the count is cleared (each try
// scans ~120 spots, which is costly). With plan.targets.max_sites
// sites open, buildings that cost Tools wait (the rest still go up),
// so scarce Tools finish the open sites. Returns [placed or "", fails].
fn do_build(c, b, queue, wet, n, built, fails, max_sites) {
    let open = 0;
    for key in b.keys() {
        if key.contains("(site)") && !key.contains("bed")
            && !key.contains("farm") {
            open += 1;
        }
    }
    let rows = screen("build").rows;
    for name in queue {
        if pick(b, name).len() > 0 || built.contains(name) {
            continue;
        }
        if fails.contains(name) && fails[name] >= 3 {
            continue;
        }
        let ok = false;
        for r in rows {
            if r.name == name && !r.locked {
                ok = !(open >= max_sites && r.cost.contains("Tool"));
            }
        }
        if !ok {
            continue;
        }
        let at = spot_centre(c, n, 8);
        if name == "Waystation" {
            at = spot_centre(c, n % 8 + 1, 24);
        }
        let water = wet[(n + 1) % 2];
        if water.len() == 0 {
            water = wet[n % 2];
        }
        if (name.contains("Dock") || name.contains("Mill"))
            && water.len() > 0 {
            at = water[(n * 3) % water.len().min(12)];
        }
        let r = build_near(name, at);
        if r.ok {
            log("build " + name + " ok");
            return [name, fails];
        }
        let m = r.msg;
        log("build " + name + ": " + m.sub_string(m.len() - 50));
        fails[name] = if fails.contains(name) { fails[name] + 1 } else { 1 };
        return ["", fails];
    }
    ["", fails]
}

// Whether the endeavor's "Bending masters" sign is met. Once the
// gate stands the screen lists no signs at all: that counts as met,
// so bending (by foreign residents too) doesn't start up again.
fn masters_met() {
    for g in screen("endeavor").groups {
        for s in g.signs {
            if s.name.starts_with("Bending masters") {
                return s.met;
            }
        }
    }
    true
}

// Bending: every ability each time, one dry spot by water and one
// shore water cell each, so every bender meets every form. Carve
// Canal is left out: canals dug around camp can cut settlers off,
// and three forms are enough for masters. 1-caster abilities are
// Aang's own: his casts drain the 100 chi each gate pillar needs,
// so they're never ordered, and all bending stops once the masters
// sign is met (live 7633: Fire pillar "waits for a bender with
// 100 chi" while Aang was kept busy bending).
fn do_bend(wet, n, gate) {
    if masters_met() {
        return;
    }
    let abil = screen("bend").abilities;
    if abil.len() == 0 {
        return;
    }
    let steps = [];
    let i = 0;
    for a in abil {
        if a.name.contains("Canal") || a.casters == "1 bender" {
            continue;
        }
        for spots in wet {
            if spots.len() == 0 {
                continue;
            }
            let m = spots.len().min(40);
            let s = spots[(n * 7 + i * 5) % m];
            steps.push(#{step: "designate", tool: "bend",
                ability: a.name,
                from: #{x: s.x, y: s.y}, to: #{x: s.x, y: s.y}});
        }
        i += 1;
    }
    if steps.len() == 0 {
        return;
    }
    let r = act(steps, "bend");
    let done = 0;
    for x in r.notices {
        if x.notice.contains(" 1 done") {
            done += 1;
        }
    }
    if done == 0 {
        log(`bend: 0 of ${steps.len()} took`);
    }
}

// Corner cells of the gate (its thing reads "harmony gate (...)").
fn gate_corners(b) {
    let g = pick(b, "harmony gate");
    if g.len() == 0 {
        return [];
    }
    let x0 = g[0].x;
    let x1 = g[0].x;
    let y0 = g[0].y;
    let y1 = g[0].y;
    for k in g {
        x0 = x0.min(k.x);
        x1 = x1.max(k.x);
        y0 = y0.min(k.y);
        y1 = y1.max(k.y);
    }
    [#{x: x0, y: y0}, #{x: x1, y: y0}, #{x: x0, y: y1},
        #{x: x1, y: y1}]
}

// Gate: once the endeavor asks for a site (all signs met, "Ready"),
// place the 3x3 gate on dry open ground near camp.
fn do_gate(c, n) {
    let e = screen("endeavor");
    if !e.contains("action") || !e.action.contains("site for the gate") {
        return;
    }
    let spots = dry_origins(spot_centre(c, n, 10), 5, 30);
    let m = spots.len();
    for j in 0..4 {
        if m == 0 {
            log("gate: no dry spot");
            return;
        }
        let s = spots[(j * 7) % m];
        let r = act([#{step: "gate", x: s.x + 1, y: s.y + 1}], "gate");
        let note = r.notices[0].notice;
        log(`gate at ${s.x + 1},${s.y + 1}: ${note}`);
        if note.contains("placed") {
            return;
        }
    }
}

// Trade blind: leave any tool (Esc until Select; Esc in Select would
// open the game menu), open Traders (m), move to a row (<up*9> = the
// first deal, usually "30 food for 20 stone"), take it, close (m).
// Keys go one token group per call (mixed scripts failed to parse).
fn do_trade(row) {
    let i = 0;
    while observe().tool != "Select" && i < 4 {
        keys("<esc>", "trade");
        i += 1;
    }
    if observe().tool != "Select" {
        return;
    }
    keys("m", "trade");
    keys(row, "trade");
    keys("<enter>", "trade");
    keys("m", "trade");
}

// Log a long value in lines of at most 85 chars (test logs cut ~90).
fn log_long(s) {
    let i = 0;
    while i < s.len() && i < 425 {
        log(s.sub_string(i, (s.len() - i).min(85)));
        i += 85;
    }
}

// Status, in short lines (test logs cut at ~90 chars): stage and
// research, what the endeavor needs next, the workshop's own lines
// (sites wait on Tools), unfinished sites and plan.build entries
// not placed yet.
fn log_status(b, queue, built) {
    let e = screen("endeavor");
    let o = observe();
    log(`${e.stage} | ${o.research.label}`);
    log(`next: ${o.endeavor.line} ${o.endeavor.next}`);
    if e.contains("tasks") {
        for k in e.tasks {
            log(`task ${k.name}: ${k.materials} ${k.status}`);
        }
    }
    let ws = pick(b, "workshop");
    if ws.len() > 0 {
        log_long(`workshop: ${inspect(ws[0].x, ws[0].y).lines}`);
    }
    let sites = [];
    for key in b.keys() {
        if key.contains("(site)") {
            sites.push(key.sub_string(0, 8));
        }
    }
    log(`tool ${store("Tool")} herb ${store("Herbs")} sites ${sites}`);
    let missing = [];
    for name in queue {
        if pick(b, name).len() == 0 && !built.contains(name) {
            missing.push(name.sub_string(0, name.len().min(9)));
        }
    }
    log(`missing ${missing}`);
}

// The camp is fixed once (kept in memory), so a re-run after the
// cursor moved doesn't found a second camp elsewhere.
let c = if memory.contains("camp") { memory.camp } else { camp_centre() };
memory.camp = c;
log("camp " + c);
let max_sites = if t.contains("max_sites") { t.max_sites } else { 3 };
let floor = if t.contains("stone_floor") { t.stone_floor } else { 240 };
let n = 0;
// Next map chunk scroll_scan looks at (0..27).
let sc = 0;
let wet = [[], []];
let k = #{stone: false, ore: false};
// Tree, stone and gold cells found unreachable, never inspected again.
let bad = #{};
// Buildings placed by do_build, so far ones aren't placed twice.
let built = #{};
// Loops each build site has stood unfinished.
let ages = #{};
// Placement refusals per building name, cleared every 60 loops.
let fails = #{};
// Heavy duties are staggered (n % 4 == 0 wide scan, others offset)
// so no one loop runs them all: v29 ran away at n=0 on seed 7.
loop {
    if n % 20 == 1 {
        wet = wet_spots(c);
        k.stone = known("Stoneworking");
        k.ore = known("Prospecting");
    }
    if n % 60 == 0 {
        fails = #{};
    }
    if n % 4 == 0 {
        let w = wide_scan(c);
        // Trees too, once wood runs low: marked trees out of reach
        // held all the chop marks and wood sat at 0 (seed 42, live
        // 7633 late). Only when low: filtering from the start sent
        // settlers far (seed 42: 4 hurt then starved at t238).
        if store("Wood") < t.wood_low {
            let r = reachable(w.wood, bad);
            w.wood = r[0];
            bad = r[1];
        }
        if k.stone && store("Stone") < t.stone {
            let r = reachable(w.stone, bad);
            w.stone = r[0];
            bad = r[1];
        }
        if k.ore && store("Gold Ore") < t.ore {
            let r = reachable(w.gold, bad);
            w.gold = r[0];
            bad = r[1];
        }
        do_materials(t, k, w);
    } else {
        do_scrolls(scroll_scan(sc));
        sc = (sc + 1) % 28;
    }
    let b = scan(c);
    if n % 2 == 1 {
        do_research(p.research);
    }
    do_stockpile(c, t, n);
    do_food(c, b, t);
    do_farms(c, b, t);
    do_beds(c, b, t);
    if n % 10 == 7 {
        do_lodges(c, b, t, n / 10);
    }
    if n % 40 == 39 && n > 100 {
        do_tools(c, b, n / 40);
    }
    let r = do_stuck_sites(b, ages, built);
    ages = r[0];
    built = r[1];
    let r = do_build(c, b, p.build, wet, n, built, fails, max_sites);
    fails = r[1];
    if r[0] != "" {
        built[r[0]] = true;
    }
    // Bend every other loop: every loop (v46-47, v49) kept all hands
    // bending (seed 42 stone never came; seed 7 lost two to hunger).
    if n % 2 == 0 {
        do_bend(wet, n / 2, []);
    }
    if n % 5 == 3 {
        do_gate(c, n / 5);
    }
    // Stone short (seed 42: few reachable outcrops; the gate stages
    // eat stone too): buy it with food from visiting traders. Below
    // the sign's 200 any time; up to the floor only once masters are
    // made (seed 7: masters t293 -> t440 with trading all along).
    if n > 80 && n % 3 == 2 && food_total() > 100 {
        let s = store("Stone");
        if s < t.stone_sign || (s < floor && masters_met()) {
            do_trade("<up*9>");
        }
    }
    if n % 15 == 6 {
        let o = observe();
        log(`t${o.turn} ppl ${o.people.total} food ${food_total()} `
            + `wood ${store("Wood")} stone ${store("Stone")} `
            + `ore ${store("Gold Ore")} bad ${bad.len()} `
            + `idle ${idle()} needs ${o.people.needs}`);
        log_status(b, p.build, built);
    }
    n += 1;
    wait(3);
}
