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
    /// The portrait faceset. A pack faceset is one 38x38 image. Original art
    /// (`original/faces_<nation>.png`, from `art/faces_<nation>.toml`) is one row of square cells,
    /// 32x32 each, shared by the nation's characters: the face is the cell at the character's
    /// position among the rows with the same `face` path (see `character_face_cell`).
    pub face: &'static str,
}

macro_rules! pack {
    ($dir:literal) => {
        concat!("third_party/ninja-adventure/Actor/Character/", $dir)
    };
}

pub const CHARACTERS: &[CharacterRow] = &[
    // Air Nomads. The portraits stay on the pack's facesets until the portraits issue (#152).
    CharacterRow {
        id: "air_monk",
        sheet: "original/air_monk.png",
        face: pack!("Monk2/Faceset.png"),
    },
    CharacterRow {
        id: "air_elder",
        sheet: "original/air_elder.png",
        face: pack!("Master/Faceset.png"),
    },
    CharacterRow {
        id: "air_acolyte",
        sheet: "original/air_acolyte.png",
        face: pack!("NinjaYellow/Faceset.png"),
    },
    // Water Tribes
    CharacterRow {
        id: "water_hunter",
        sheet: "original/water_hunter.png",
        face: pack!("Eskimo/Faceset.png"),
    },
    CharacterRow {
        id: "water_healer",
        sheet: "original/water_healer.png",
        face: pack!("NinjaEskimo/Faceset.png"),
    },
    CharacterRow {
        id: "water_warrior",
        sheet: "original/water_warrior.png",
        face: pack!("NinjaWater/Faceset.png"),
    },
    // Earth Kingdom (art/earth_*.toml, #151; faces in art/faces_earth.toml, #153)
    CharacterRow {
        id: "earth_farmer",
        sheet: "original/earth_farmer.png",
        face: "original/faces_earth.png",
    },
    CharacterRow {
        id: "earth_guard",
        sheet: "original/earth_guard.png",
        face: "original/faces_earth.png",
    },
    CharacterRow {
        id: "earth_villager",
        sheet: "original/earth_villager.png",
        face: "original/faces_earth.png",
    },
    // Fire Nation (art/fire_*.toml, #151; faces in art/faces_fire.toml, #153)
    CharacterRow {
        id: "fire_soldier",
        sheet: "original/fire_soldier.png",
        face: "original/faces_fire.png",
    },
    CharacterRow {
        id: "fire_noble",
        sheet: "original/fire_noble.png",
        face: "original/faces_fire.png",
    },
    CharacterRow {
        id: "fire_smith",
        sheet: "original/fire_smith.png",
        face: "original/faces_fire.png",
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

/// The faceset image of a character (see `character_face_cell` for which cell is the face).
pub fn character_face_path(character: &str) -> &'static str {
    character_row(character).unwrap_or_else(|| panic!("no art for character {character}")).face
}

/// Which square cell of the faceset is this character's face: its position among the rows that
/// share the faceset, so a pack faceset (one face per image) is cell 0. The cell's side is the
/// image's height.
pub fn character_face_cell(character: &str) -> u32 {
    let row =
        character_row(character).unwrap_or_else(|| panic!("no art for character {character}"));
    CHARACTERS.iter().filter(|r| r.face == row.face).position(|r| r.id == row.id).unwrap_or(0)
        as u32
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
                assert_eq!(png_size(character_sheet_path(c)), (64, 64), "{c} sheet");
                let (w, h) = png_size(character_face_path(c));
                assert!(h == 38 || h == 32, "{c} face cells are {h}px");
                let cells =
                    CHARACTERS.iter().filter(|r| r.face == character_face_path(c)).count() as u32;
                assert_eq!(w, h * cells, "{c} faceset is {cells} cells of {h}px");
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
        assert!(seen >= 12, "only {seen} original character sheets");
    }

    #[test]
    fn original_facesets_hold_one_32px_face_per_character_in_row_order() {
        let mut seen = 0;
        for face in ["original/faces_earth.png", "original/faces_fire.png"] {
            let ids: Vec<&str> =
                CHARACTERS.iter().filter(|r| r.face == face).map(|r| r.id).collect();
            assert_eq!(png_size(face), (32 * ids.len() as u32, 32), "{face}");
            // `art/faces_<nation>.toml` names its sprites by character id, and sprite `n` is cell `n`.
            let sheet = face.trim_start_matches("original/").trim_end_matches(".png");
            assert_eq!(art_sprites(sheet), ids, "{face}");
            for (n, id) in ids.iter().enumerate() {
                assert_eq!(character_face_cell(id), n as u32, "{id}");
            }
            seen += ids.len();
        }
        assert_eq!(seen, 6);
    }
}
