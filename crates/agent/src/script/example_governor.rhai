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

// Beds: build one near camp while beds trail people + targets.beds_ahead.
fn beds_duty(targets, camp) {
    let beds = find("Bed", camp, 12).len();
    let wanted = observe().people.total + targets.beds_ahead;
    if beds < wanted {
        build_near("Bed", camp, 4);
    }
}

loop {
    let p = plan();
    research_duty(p.research);
    food_duty(camp);
    beds_duty(p.targets, camp);
    wait(3);
}
