p='crates/client/src/cast_demo.rs'
s=open(p).read()
def rep(a,b):
    global s
    assert a in s, a
    s=s.replace(a,b,1)
rep('''    let target = match session.defs.ability(ability).id.as_str() {
        // Next to the river, so the patch is half on the bank.
        "water_the_land" => at(7, 0),
        "wind_carry" => stage_a_stack(session, origin)?,
        _ => at(if session.defs.ability(ability).id == "raise_earth" { 9 } else { 7 }, 0),
    };''','''    let target = match session.defs.ability(ability).id.as_str() {
        "raise_earth" => at(9, 0),
        "wind_carry" => stage_a_stack(session, origin)?,
        // Water the Land and Hearthflame: a tile by the river, so the patch is on the bank.
        _ => at(7, 0),
    };''')
rep("    run_to(session, ability, t)?;","    run_to(session, t)?;")
rep("fn run_to(session: &mut Session, ability: AbilityId, t: f32) -> Option<()> {","fn run_to(session: &mut Session, t: f32) -> Option<()> {")
rep("    let _ = ability;\n","")
rep("use fn_sim::bending::AbilityId;\n","")
open(p,'w').write(s)

p='crates/client/src/main.rs'
s=open(p).read()
rep("mod cast;\n","mod cast;\nmod cast_demo;\n")
rep('''    app.insert_resource(Time::<Fixed>::from_hz''','''    let cast_demo = std::env::var("FN_CAST_DEMO")
        .ok()
        .and_then(|s| s.parse().ok())
        .and_then(|t| cast_demo::prepare(&mut sim, t));
    app.insert_resource(Time::<Fixed>::from_hz''')
rep('''    app.run();
}

/// Present when running as a headless''','''    if let Some(demo) = cast_demo {
        app.insert_resource(demo)
            .add_systems(Update, cast_demo::frame_camera.after(controls));
    }
    app.run();
}

/// Present when running as a headless''')
rep('''//! beside the first one, facing every way, and takes the shot after STEPS steps (`walk_demo`, #165).''','''//! beside the first one, facing every way, and takes the shot after STEPS steps (`walk_demo`, #165).
//! `FN_CAST_DEMO=T` stages the nation's ability on a levelled meadow and pauses the game `T`
//! seconds from the moment the bend completes (negative: mid-channel, e.g. `-0.4`; positive: it
//! has landed, e.g. `0.3`), so the shot catches the bender's pose and the cast effect (`cast_demo`,
//! #166).''')
open(p,'w').write(s)
