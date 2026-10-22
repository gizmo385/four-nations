//! Test-only helpers for loading defs with extra abilities.

use crate::defs::{Defs, DefsError};

const BIOMES: &str = include_str!("../../../../data/biomes.toml");
const RESOURCES: &str = include_str!("../../../../data/resources.toml");
const NATIONS: &str = include_str!("../../../../data/nations.toml");
const RESEARCH: &str = include_str!("../../../../data/research.toml");
const NEEDS: &str = include_str!("../../../../data/needs.toml");
const JOBS: &str = include_str!("../../../../data/jobs.toml");
pub const BENDING: &str = include_str!("../../../../data/bending.toml");

/// The bundled data, with `bending` appended to `data/bending.toml` and `research` appended to
/// `data/research.toml`.
pub fn load(bending: &str, research: &str) -> Result<Defs, DefsError> {
    Defs::from_toml(
        BIOMES,
        RESOURCES,
        NATIONS,
        &format!("{RESEARCH}\n{research}"),
        NEEDS,
        JOBS,
        &format!("{BENDING}\n{bending}"),
    )
}

/// The bundled data with `bending.toml` replaced by `bending`.
pub fn load_only(bending: &str) -> Result<Defs, DefsError> {
    Defs::from_toml(BIOMES, RESOURCES, NATIONS, RESEARCH, NEEDS, JOBS, bending)
}

/// An earth tech that unlocks `probe_locked`.
pub const PROBE_TECH: &str = "
    [[tech]]
    id = 'probe_lore'
    name = 'Seismic Sense'
    description = ''
    nation = 'earth'
    cost = 60
    [tech.unlocks]
    abilities = ['probe_locked']
";

/// The test-only effect, as three abilities: `probe` (Earth, 40 chi), `probe_locked` (Earth,
/// unlocked by `probe_lore`) and `probe_water` (Water). `probe`'s channel time is `work`
/// seconds and `extra` is added to its fields (a `speed_modifier`, say).
pub fn probe_defs(work: f32, extra: &str) -> Defs {
    let abilities = format!(
        "
        [[ability]]
        id = 'probe'
        nation = 'earth'
        name = 'Probe'
        verb = 'Probe'
        chi = 40.0
        work = {work}
        {extra}
        [ability.effect]
        kind = 'probe'
        [[ability]]
        id = 'probe_locked'
        nation = 'earth'
        name = 'Locked probe'
        verb = 'Probe'
        chi = 10.0
        work = 0.0
        [ability.effect]
        kind = 'probe'
        [[ability]]
        id = 'probe_water'
        nation = 'water'
        name = 'Water probe'
        verb = 'Probe'
        chi = 10.0
        work = 0.0
        [ability.effect]
        kind = 'probe'
        "
    );
    load(&abilities, PROBE_TECH).expect("probe defs are valid")
}
