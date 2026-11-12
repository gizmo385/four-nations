//! #858: with the `serde` feature on, the view models serialise to JSON a webview or a script can
//! read. Run with `cargo test -p fn_ui --features serde --test it serde_views::`; without the
//! feature this file is empty.
#![cfg(feature = "serde")]

use fn_sim::{Nation, Pos};
use fn_ui::alerts::Alerts;
use fn_ui::tools::Tool;
use fn_ui::{Session, build, view};
use serde_json::{Value, json};

fn session() -> Session {
    Session::new(7, Nation::Earth, 64, 64)
}

fn to_json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("a view model serialises")
}

#[test]
fn side_view_serialises_to_json_with_its_fields() {
    let s = session();
    let v = view::side_view(&s, Tool::Select, "Marked 3 trees", None, &Alerts::default());
    let j = to_json(&v);
    assert_eq!(j["seed"], 7);
    assert_eq!(j["nation"], v.nation);
    assert_eq!(j["people"], v.people);
    assert_eq!(j["notice"], "Marked 3 trees");
    assert_eq!(j["tool"], "Select");
    assert_eq!(j["paused"], v.paused);
    assert!(j["choice"].is_null(), "no building chosen: {j}");
    assert_eq!(j["stores"].as_array().map(Vec::len), Some(v.stores.len()));
    assert!(j["research"].is_object(), "nested models become objects: {j}");
    // The whole value survives text: what a webview receives is what `to_json` built.
    let text = serde_json::to_string(&v).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), j);
}

#[test]
fn unit_views_carry_their_needs_and_ids() {
    let s = session();
    let unit = s.world.units.iter().find(|u| !u.is_beast()).expect("a settler");
    let v = view::unit_view(&s, unit);
    let j = to_json(&v);
    assert_eq!(j["name"], v.name);
    assert_eq!(j["tile"], json!({ "x": unit.pos.x, "y": unit.pos.y }));
    let needs = j["needs"].as_array().expect("needs is a list");
    let labels: Vec<_> = needs.iter().map(|n| n["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["Health", "Hunger", "Thirst", "Energy"]);
    assert!(needs.iter().all(|n| n["frac"].is_number() && n["status"].is_string()));
}

#[test]
fn the_nation_screen_and_a_build_ghost_serialise() {
    let s = session();
    let nation = to_json(&view::nation_view(&s));
    assert_eq!(nation["seed"], 7);
    assert!(nation["groups"].as_array().is_some_and(|g| !g.is_empty()));
    // `Placement` embeds sim types with no `Serialize` of their own: they come out as plain
    // tuples and text.
    let bed = s.defs.building_id("bed").expect("the bed");
    let ghost = to_json(&build::placement(&s, Pos::new(10, 10), bed));
    assert_eq!(ghost["footprint"].as_array().map(Vec::len), Some(3));
    assert!(ghost["tiles"].as_array().is_some_and(|t| !t.is_empty()), "{ghost}");
}
