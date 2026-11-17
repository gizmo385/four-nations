//! The key help's guide tabs (playtest, 2026-11-17: "put other how to info there, such as how
//! to play the initial part of the game, how units work"): Getting started and Settlers, as
//! [`Para`]s the How to rows lay out ([`crate::screens::how_to_lines`]). Machines is
//! [`crate::flow::walkthrough`].
//!
//! Each line names a real key and says one thing. The facts come from the data: the campfire's
//! tech, the newcomer's food (`data/needs.toml`'s `food_per_newcomer`), the Harmony Gate's sign
//! count.

use crate::flow::Para;
use crate::session::Session;

/// The opening: a stockpile, food, wood, beds, research, a campfire, and what brings newcomers
/// and where the goal is.
pub fn getting_started(session: &Session) -> Vec<Para> {
    let (world, defs) = (&session.world, &session.defs);
    let food = defs.needs.growth.food_per_newcomer;
    let campfire = defs.building_id("campfire").map(|id| {
        let def = defs.building(id);
        let name = def.display_name(world.nation).to_owned();
        match def.unlocked_by.filter(|&t| !world.research().is_known(t)) {
            Some(t) => format!("Research {}, then build a {name}", defs.tech(t).name),
            None => format!("Build a {name}"),
        }
    });
    let mut out = vec![
        Para::Heading("Getting started".to_owned()),
        Para::Text(
            "Your settlers work on their own. You mark what needs doing, and they do it between \
             eating, drinking and sleeping."
                .to_owned(),
        ),
        Para::Blank,
        Para::Text(
            "1. Make a stockpile near your people: press P, then Enter on two corners. \
             Gathered goods go there."
                .to_owned(),
        ),
        Para::Text(
            "2. Gather food: press H and mark fruit trees or wild cabbage. Press O while \
             marking to keep picking them as they grow back."
                .to_owned(),
        ),
        Para::Text("3. Chop trees for wood the same way, with H.".to_owned()),
        Para::Text("4. Build a bed for each settler: press U and pick Bed.".to_owned()),
        Para::Text("5. Pick something to research: press T.".to_owned()),
    ];
    if let Some(campfire) = campfire {
        out.push(Para::Text(format!(
            "6. {campfire} near the beds. Warm beds bring newcomers sooner."
        )));
    }
    out.push(Para::Blank);
    out.push(Para::Heading("Growing".to_owned()));
    out.push(Para::Text(format!(
        "A newcomer arrives when there's a free bed and at least {food} food in your \
         stockpiles. Spare beds, warm beds and more than one kind of food bring them sooner."
    )));
    out.push(Para::Blank);
    out.push(Para::Heading("The goal".to_owned()));
    out.push(Para::Text(
        "Press G for the Harmony Gate's signs: what your city needs before it can build the \
         gate. The side panel shows how many you've met."
            .to_owned(),
    ));
    out
}

/// How settlers work: indirect control, selecting and following, needs, idle, bending, crafting.
pub fn settlers(_session: &Session) -> Vec<Para> {
    vec![
        Para::Heading("Settlers".to_owned()),
        Para::Text(
            "You don't give settlers orders. You mark work with the tools, and a free settler \
             takes the nearest job they can do."
                .to_owned(),
        ),
        Para::Blank,
        Para::Text(
            "Select one with Enter, or Tab through them. F follows the selected settler. The \
             side panel shows their task and needs."
                .to_owned(),
        ),
        Para::Text(
            "Settlers get hungry, thirsty and tired. When one runs low, they stop to eat from a \
             stockpile, drink at a river or lake, or sleep in a bed. A need left empty hurts \
             them, and they can die."
                .to_owned(),
        ),
        Para::Text(
            "Idle in the side panel counts settlers with nothing to do. Select one to see why, \
             then mark more work."
                .to_owned(),
        ),
        Para::Text(
            "Every settler bends their nation's element. Press B to pick an ability, then mark \
             where to use it. Bending uses chi, which comes back over time."
                .to_owned(),
        ),
        Para::Text(
            "Buildings with recipes, like the Workshop, need a settler working inside. One \
             takes the job once the inputs are in a stockpile. Press R on one to pick what it \
             makes."
                .to_owned(),
        ),
    ]
}
