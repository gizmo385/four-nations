// governor: one loop of duties. Intent lives in plan(); this is the how.
let camp = memory.camp ?? observe().cursor;
memory.camp = camp;

// Techs that can be picked right now. act's ok only says the keys were
// pressed, so a locked or finished tech would look the same.
fn open_techs() {
    let open = [];
    for column in screen("research").columns {
        for card in column {
            if card.state == "Available" {
                open.push(card.name);
            }
        }
    }
    open
}

// Research: take the first open tech in the plan when none is set.
fn research_duty(queue) {
    if !observe().research.label.contains("none set") {
        return;
    }
    let open = open_techs();
    for tech in queue {
        if open.contains(tech) {
            act([#{step: "research", tech: tech}], "research");
            return;
        }
    }
}

// Food: keep harvest marks standing near camp, so nobody goes Hungry.
fn food_duty(camp) {
    designate_near("harvest", "Cabbage", camp, 19, true);
}

// Stockpile: harvested food needs a pile to go to. Mark a 5x5 block near
// camp, trying a few spots until one takes (water refuses); one more pass
// adds another block while the pile is under 12 tiles.
fn stockpile_duty(camp) {
    if observe().stockpile.tiles >= 12 {
        return;
    }
    for dx in [3, -7, 9, -13] {
        let from = #{x: camp.x + dx, y: camp.y - 2};
        let to = #{x: from.x + 4, y: from.y + 4};
        let step = #{step: "designate", tool: "stockpile", from: from, to: to};
        if act([step], "stockpile").ok {
            return;
        }
    }
}

// Wood: beds are built from it, so chop the trees by camp while it is short.
fn wood_duty(camp) {
    if store("Wood") < 10 {
        designate_near("harvest", "Oak Tree", camp, 9, false);
        designate_near("harvest", "Fir Tree", camp, 9, false);
    }
}

// Beds: build sites near camp while finished beds plus sites trail
// people + targets.beds_ahead. A bed with a sleeper on it reads as a settler,
// so this can build a few spare.
fn beds_duty(targets, camp) {
    let area = cells(camp.x - 15, camp.y - 15, 31, 31);
    let sites = area.filter(|c| c.marks.contains("build")).len();
    let beds = find("Bed", camp, 15).len() + sites;
    let wanted = observe().people.total + targets.beds_ahead;
    if beds < wanted {
        build_near("Bed", camp, 4);
    }
}

loop {
    let p = plan();
    research_duty(p.research);
    food_duty(camp);
    stockpile_duty(camp);
    wood_duty(camp);
    beds_duty(p.targets, camp);
    wait(3);
}
