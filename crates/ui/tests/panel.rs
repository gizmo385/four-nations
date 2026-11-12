//! #873: the side panel's model, built from a fixed-seed session with no width.

use fn_sim::Nation;
use fn_ui::panel::{SidePanel, ToolLine};
use fn_ui::rows::{Marks, Tone};
use fn_ui::state::UiState;
use fn_ui::{Session, Tool};

fn panel(tool: Tool) -> SidePanel {
    let session = Session::new(7, Nation::Earth, 64, 64);
    let mut ui = UiState::new(&session, (120, 40));
    ui.set_tool(tool);
    SidePanel::build(&session, &ui, &Marks::new(false), 10)
}

#[test]
fn the_settlement_glance_says_who_lives_there() {
    let p = panel(Tool::Select);
    assert_eq!(p.title, "◆ EARTH KINGDOM ◆");
    assert_eq!(p.footer, "seed 7");
    assert_eq!((p.settlement.title.as_str(), p.settlement.right.as_str()), ("SETTLEMENT", "[N]"));
    let people = &p.settlement.people;
    assert_eq!(people.plain(), "People 6 Idle 6");
    assert_eq!(people.left[0], ("People 6".to_owned(), Tone::Text));
    assert_eq!(people.right, Some(("Idle 6".to_owned(), Tone::Text)));
    assert_eq!(p.header.len(), 2);
}

#[test]
fn the_tool_section_exists_only_with_a_tool_and_keeps_its_text_whole() {
    assert!(panel(Tool::Select).tool.is_none());
    let tool = panel(Tool::Harvest).tool.expect("the Harvest tool has a section");
    assert_eq!((tool.title.as_str(), tool.right.as_str()), ("Harvest", "[H]"));
    assert!(!tool.list && !tool.repeating);
    let Some(ToolLine::Text(text, Tone::Text)) = tool.blocks.first().and_then(|b| b.first()) else {
        panic!("the first line is what the tool does: {:?}", tool.blocks);
    };
    assert!(text.len() > 28, "no wrapping in the model: {text}");
    assert_eq!(tool.keep, 1);
}

#[test]
fn the_build_tool_puts_its_building_under_the_description() {
    let tool = panel(Tool::Build).tool.expect("the Build tool has a section");
    assert_eq!(tool.keep, 2);
    assert!(matches!(tool.blocks[1].as_slice(), [ToolLine::Building(_)]), "{:?}", tool.blocks[1]);
}
