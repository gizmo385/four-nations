// settle.rhai: a stock script library for `fn_agent play` (#483, docs/design/
// agent-scripting.md, "Headless play"). Plays a settlement's own spawn camp through routine
// chores with no model: a standing harvest order on every wild, renewable food resource that
// needs no research (Wild Cabbage, Fruit Tree -- the same food `fn_sim::autoplay`'s own
// standing-harvest step keeps up; Fishing Spots are left alone, since harvesting one needs the
// Fishing tech this script never researches), and a bed built one ahead of the living
// population once there's wood for one. Nothing here reaches past the porcelain layer
// (`designate_near`, `build_near`, `find`, `store`), the whole point of a "proven" script
// library runs with no model at all.
//
// Proven against seed 7, Earth, Normal: `crates/agent/tests/settle_script.rs`'s `ci --fast`
// test runs a 30-turn slice of this file twice through `fn_agent play` and checks the two run
// logs come out byte-for-byte the same; its soak test (outside `ci --fast`, like
// `crates/sim/tests/balance.rs`'s own CI soak) plays the whole first game year and checks
// nobody starves.

let camp = memory.camp ?? observe().cursor;
memory.camp = camp;

loop {
    // Renewable food, kept standing so it's re-designated the moment each tile ripens again
    // (`designate_near`'s own `standing` argument) -- re-running this every loop is harmless,
    // the same way `fn_sim::autoplay`'s own standing-harvest step is (`AlreadyDesignated` is
    // ignored).
    designate_near("harvest", "Wild Cabbage", camp, 15, true);
    designate_near("harvest", "Fruit Tree", camp, 15, true);

    // A bed one ahead of the living population (`fn_sim::autoplay::Economy`'s own target),
    // once there's wood to build one: `find` only sees what's within reach of `camp`, close
    // enough for "how many beds does this camp already have" to be a fair count.
    let people = observe().people.total;
    let beds = find("Bed", camp, 15).len();
    if beds < people + 1 && store("Wood") >= 5 {
        let r = build_near("Bed", camp);
        if !r.ok {
            log(`bed: ${r.msg}`);
        }
    }

    wait(5);
}
