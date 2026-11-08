use super::*;

fn economy(styles: &Styles, id: &str) -> (bool, HealingPolicy, BedPolicy, ResearchOrder) {
    match styles.get(id).unwrap_or_else(|| panic!("no style {id}")).play {
        Play::Economy { farms, healing, beds, research } => (farms, healing, beds, research),
        Play::Idle => panic!("{id} is idle"),
    }
}

#[test]
fn the_bundled_styles_load_and_resolve() {
    let styles = Styles::bundled();
    let ids: Vec<&str> = styles.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["steady", "careless", "branch_first", "idle"]);

    assert_eq!(
        economy(&styles, "steady"),
        (
            true,
            HealingPolicy::Planned,
            BedPolicy::ToTarget { margin: 2 },
            ResearchOrder::SharedFirst
        )
    );
    assert_eq!(
        economy(&styles, "careless"),
        (false, HealingPolicy::Off, BedPolicy::Ahead, ResearchOrder::SharedFirst)
    );
    // `branch_first` is `steady` with one field changed.
    assert_eq!(
        economy(&styles, "branch_first"),
        (
            true,
            HealingPolicy::Planned,
            BedPolicy::ToTarget { margin: 2 },
            ResearchOrder::BranchFirst
        )
    );
    assert_eq!(styles.get("idle").unwrap().play, Play::Idle);
}

#[test]
fn a_style_maps_onto_the_economy_builders() {
    let styles = Styles::bundled();
    let Player::Economy(e) = styles.get("steady").unwrap().player(6) else { panic!() };
    assert_eq!(e.people, 6);
    assert!(e.farms);
    assert_eq!(e.healing, HealingPolicy::Planned);
    assert_eq!(e.beds, BedPolicy::ToTarget { margin: 2 });
    assert_eq!(e.research, ResearchOrder::SharedFirst);
    assert!(matches!(styles.get("idle").unwrap().player(6), Player::Idle));
}

#[test]
fn absent_fields_take_the_economy_defaults() {
    let styles = Styles::parse("[[style]]\nid = \"plain\"\nopening = \"economy\"\n").unwrap();
    let Player::Economy(e) = styles.get("plain").unwrap().player(6) else { panic!() };
    let default = Economy::new(6);
    assert_eq!(
        (e.farms, e.healing, e.beds, e.research),
        (default.farms, default.healing, default.beds, default.research)
    );
}

#[test]
fn a_child_that_sets_beds_drops_the_parents_margin() {
    let text = r#"
[[style]]
id = "a"
opening = "economy"
beds = "to_target"
bed_margin = 3

[[style]]
id = "b"
inherits = "a"
beds = "ahead"

[[style]]
id = "c"
inherits = "a"
bed_margin = 5
"#;
    let styles = Styles::parse(text).unwrap();
    assert_eq!(economy(&styles, "b").2, BedPolicy::Ahead);
    assert_eq!(economy(&styles, "c").2, BedPolicy::ToTarget { margin: 5 });
}

fn parse_err(text: &str) -> String {
    Styles::parse(text).expect_err("should not load").to_string()
}

#[test]
fn strict_keys_and_values() {
    assert!(parse_err("[[style]]\nid = \"x\"\nopening = \"none\"\nfarm = true\n").contains("farm"));
    assert!(parse_err("[[style]]\nid = \"x\"\nopening = \"nothing\"\n").contains("nothing"));
    assert!(
        parse_err("[[style]]\nid = \"x\"\nopening = \"economy\"\nbeds = \"lots\"\n")
            .contains("lots")
    );
    assert!(parse_err("[[styles]]\nid = \"x\"\n").contains("styles"));
}

#[test]
fn bad_structure_is_named() {
    let dup =
        "[[style]]\nid = \"x\"\nopening = \"none\"\n[[style]]\nid = \"x\"\nopening = \"none\"\n";
    assert!(parse_err(dup).contains("used twice"));
    assert!(parse_err("[[style]]\nid = \"x\"\ninherits = \"y\"\n").contains("unknown style"));
    let loops =
        "[[style]]\nid = \"x\"\ninherits = \"y\"\n[[style]]\nid = \"y\"\ninherits = \"x\"\n";
    assert!(parse_err(loops).contains("loops"));
    assert!(parse_err("[[style]]\nid = \"x\"\nfarms = true\n").contains("no `opening`"));
    assert!(
        parse_err("[[style]]\nid = \"x\"\nopening = \"none\"\nfarms = true\n")
            .contains("does nothing")
    );
    let margin = "[[style]]\nid = \"x\"\nopening = \"economy\"\nbeds = \"ahead\"\nbed_margin = 1\n";
    assert!(parse_err(margin).contains("bed_margin"));
}
