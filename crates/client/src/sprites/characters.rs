//! Where each character's art lives: one row per character id. Which characters a nation uses is
//! data (`characters` in `data/nations.toml`); this table is only how a character id becomes
//! files. Replacing a character's art is editing that character's row.

/// The art of one character.
pub struct CharacterRow {
    pub id: &'static str,
    /// The sprite sheet of 16px cells. Original art (`original/<id>.png`, from `art/<id>.toml`) is
    /// 64x64: the columns face down, up, left and right, the rows are 4 walk frames and row 0 is
    /// the idle pose. A pack sheet is 4 columns by 7 rows (64x112).
    pub sheet: &'static str,
    /// The 38x38 portrait.
    pub face: &'static str,
}

macro_rules! pack {
    ($dir:literal) => {
        concat!("third_party/ninja-adventure/Actor/Character/", $dir)
    };
}

pub const CHARACTERS: &[CharacterRow] = &[
    // Air Nomads
    CharacterRow {
        id: "Monk2",
        sheet: pack!("Monk2/SpriteSheet.png"),
        face: pack!("Monk2/Faceset.png"),
    },
    CharacterRow {
        id: "Master",
        sheet: pack!("Master/SpriteSheet.png"),
        face: pack!("Master/Faceset.png"),
    },
    CharacterRow {
        id: "NinjaYellow",
        sheet: pack!("NinjaYellow/SpriteSheet.png"),
        face: pack!("NinjaYellow/Faceset.png"),
    },
    // Water Tribes
    CharacterRow {
        id: "Eskimo",
        sheet: pack!("Eskimo/SpriteSheet.png"),
        face: pack!("Eskimo/Faceset.png"),
    },
    CharacterRow {
        id: "NinjaEskimo",
        sheet: pack!("NinjaEskimo/SpriteSheet.png"),
        face: pack!("NinjaEskimo/Faceset.png"),
    },
    CharacterRow {
        id: "NinjaWater",
        sheet: pack!("NinjaWater/SpriteSheet.png"),
        face: pack!("NinjaWater/Faceset.png"),
    },
    // Earth Kingdom (art/earth_*.toml, #151). Portraits stay on the pack until #153.
    CharacterRow {
        id: "earth_farmer",
        sheet: "original/earth_farmer.png",
        face: pack!("Villager3/Faceset.png"),
    },
    CharacterRow {
        id: "earth_guard",
        sheet: "original/earth_guard.png",
        face: pack!("Samurai/Faceset.png"),
    },
    CharacterRow {
        id: "earth_villager",
        sheet: "original/earth_villager.png",
        face: pack!("NinjaGreen/Faceset.png"),
    },
    // Fire Nation (art/fire_*.toml, #151)
    CharacterRow {
        id: "fire_soldier",
        sheet: "original/fire_soldier.png",
        face: pack!("NinjaFire/Faceset.png"),
    },
    CharacterRow {
        id: "fire_noble",
        sheet: "original/fire_noble.png",
        face: pack!("SamuraiRed/Faceset.png"),
    },
    CharacterRow {
        id: "fire_smith",
        sheet: "original/fire_smith.png",
        face: pack!("NinjaRed/Faceset.png"),
    },
];

/// The art row of a character id, if it has one.
pub fn character_row(character: &str) -> Option<&'static CharacterRow> {
    CHARACTERS.iter().find(|c| c.id == character)
}

/// The sprite sheet of a character (idle-down is the cell at column 0, row 0).
pub fn character_sheet_path(character: &str) -> &'static str {
    character_row(character).unwrap_or_else(|| panic!("no art for character {character}")).sheet
}

/// The 38x38 portrait of a character.
pub fn character_face_path(character: &str) -> &'static str {
    character_row(character).unwrap_or_else(|| panic!("no art for character {character}")).face
}

#[cfg(test)]
mod tests {
    use super::super::test_support::png_size;
    use super::*;
    use fn_sim::{Defs, Nation};

    #[test]
    fn every_nation_has_three_characters_with_art() {
        let defs = Defs::bundled();
        for nation in Nation::ALL {
            let characters = &defs.nation(nation).characters;
            assert_eq!(characters.len(), 3, "{nation:?}");
            for c in characters {
                let size = png_size(character_sheet_path(c));
                assert!(size == (64, 64) || size == (64, 112), "{c} sheet is {size:?}");
                assert_eq!(png_size(character_face_path(c)), (38, 38), "{c} face");
            }
        }
    }

    #[test]
    fn no_character_is_shared_between_nations() {
        let defs = Defs::bundled();
        let all: Vec<&String> =
            Nation::ALL.iter().flat_map(|n| defs.nation(*n).characters.iter()).collect();
        for (i, c) in all.iter().enumerate() {
            assert!(!all[..i].contains(c), "{c} is used twice");
        }
    }

    #[test]
    fn the_table_has_one_row_per_character() {
        for (i, row) in CHARACTERS.iter().enumerate() {
            assert!(CHARACTERS[..i].iter().all(|r| r.id != row.id), "{} has two rows", row.id);
        }
    }

    /// The sprite names of `art/<id>.toml` in file order.
    fn art_sprites(id: &str) -> Vec<String> {
        let path = format!("{}/../../art/{id}.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: toml::Table = text.parse().unwrap();
        doc["sprite"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn original_sheets_are_four_facings_by_four_walk_frames() {
        let mut seen = 0;
        for row in CHARACTERS.iter().filter(|r| r.sheet.starts_with("original/")) {
            assert_eq!(row.sheet, format!("original/{}.png", row.id));
            assert_eq!(png_size(row.sheet), (64, 64), "{}", row.id);
            let names = art_sprites(row.id);
            // Sprite `n` sits at column `n % 4`, row `n / 4`: idle-down is (0, 0).
            assert_eq!(names.len(), 16, "{}", row.id);
            for (n, name) in names.iter().enumerate() {
                let facing = ["down", "up", "left", "right"][n % 4];
                assert_eq!(*name, format!("{facing}_{}", n / 4), "{} sprite {n}", row.id);
            }
            seen += 1;
        }
        assert!(seen >= 6, "only {seen} original character sheets");
    }
}
