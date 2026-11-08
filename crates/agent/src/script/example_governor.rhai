// governor: one loop of duties. Intent lives in plan(); this is the how.
let camp = memory.camp ?? observe().cursor;
memory.camp = camp;

// Research: take the next tech in the plan when none is set.
fn research_duty(queue) {
    if !observe().research.label.contains("none set") {
        return;
    }
    for tech in queue {
        let step = #{step: "research", tech: tech};
        if act([step], "research").ok {
            break;
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
