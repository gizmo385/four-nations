//! Where each character's art lives: one row per character id. Which characters a nation uses is
//! data (`characters` in `data/nations.toml`); this table is only how a character id becomes
//! files. Replacing a character's art is editing that character's row.

/// The art of one character.
pub struct CharacterRow {
    pub id: &'static str,
    /// The sprite sheet: 4 columns of walk frames, 7 rows of 16px cells (64x112).
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
    // Earth Kingdom
    CharacterRow {
        id: "Villager3",
        sheet: pack!("Villager3/SpriteSheet.png"),
        face: pack!("Villager3/Faceset.png"),
    },
    CharacterRow {
        id: "Samurai",
        sheet: pack!("Samurai/SpriteSheet.png"),
        face: pack!("Samurai/Faceset.png"),
    },
    CharacterRow {
        id: "NinjaGreen",
        sheet: pack!("NinjaGreen/SpriteSheet.png"),
        face: pack!("NinjaGreen/Faceset.png"),
    },
    // Fire Nation
    CharacterRow {
        id: "NinjaFire",
        sheet: pack!("NinjaFire/SpriteSheet.png"),
        face: pack!("NinjaFire/Faceset.png"),
    },
    // The upstream sheet of this one isn't called `SpriteSheet.png`.
    CharacterRow {
        id: "SamuraiRed",
        sheet: pack!("SamuraiRed/redsamurai.png"),
        face: pack!("SamuraiRed/Faceset.png"),
    },
    CharacterRow {
        id: "NinjaRed",
        sheet: pack!("NinjaRed/SpriteSheet.png"),
        face: pack!("NinjaRed/Faceset.png"),
    },
];

/// The art row of a character id, if it has one.
pub fn character_row(character: &str) -> Option<&'static CharacterRow> {
    CHARACTERS.iter().find(|c| c.id == character)
}

/// The sprite sheet of a character: 4 columns of walk frames, 7 rows of 16px cells.
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
                assert_eq!(png_size(character_sheet_path(c)), (64, 112), "{c} sheet");
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

    #[test]
    fn samurai_red_uses_its_odd_sheet_name() {
        assert!(character_sheet_path("SamuraiRed").ends_with("SamuraiRed/redsamurai.png"));
        assert!(character_sheet_path("Monk2").ends_with("Monk2/SpriteSheet.png"));
    }
}
