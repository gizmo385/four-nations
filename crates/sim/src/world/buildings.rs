//! Finished buildings as records (#604, `docs/design/buildings-as-records.md`).
//!
//! Every finished building is one `Building` record in `World::buildings`, under a
//! `BuildingKey` handed out in the order buildings finish and never reused. Each tile of its
//! footprint points at it (`Tile::building`, a `TileBuilding`), carrying only what pathfinding
//! needs (`passable`, `rail`). The record holds the building's id and footprint, and its state:
//! a production building's batch (`forge.rs`), a Map Room's chart (`chart.rs`), a farm plot
//! (`farm.rs`), an orchard (`orchard.rs`) and the building's own fire (`heat.rs`).
//!
//! `World::put_building` is the one way a finished building gets onto the map; `raise`
//! (`build.rs`) wraps it with what finishing a construction job does on top: moving units and
//! stacks out of the way, and opening the building's seats and craft jobs.
//!
//! Two orders are kept, because the simulation has always used both:
//!
//! - `rows`: by anchor, row-major. `World::buildings()` walks it, which is the order the old
//!   whole-map scan saw buildings in, so `bed_route`, `door_owner` and autoplay's first-match
//!   picks don't change.
//! - `kinds`: per kind, by anchor in `Pos` order (`x`, then `y`). The orchard, farm plot and
//!   power loops walk it, which is the order their old `BTreeMap<Pos, _>` side tables had, so
//!   orchard planting draws and Reap jobs come in the same sequence.
//!
//! Neither index is saved: `Buildings::reindex` rebuilds both from the records on load, from the
//! current defs, in time proportional to the number of buildings rather than the map.

use crate::building::{BuildingKey, TileBuilding, is_one_by_one, one_by_one};
use crate::geom::Footprint;
use crate::jobs::JobKind;
use crate::mapgen::Tile;
use crate::save::v2;

use super::farm::FarmPlot;
use super::fuel::Bunker;
use super::orchard::Orchard;
use super::pickers::Pickers;
use super::*;

/// A finished building: what it is, where it stands, and the state of whatever it does.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Building {
    pub id: BuildingId,
    /// Its top-left tile.
    pub anchor: Pos,
    /// The footprint it was built with, `[w, h]`: baked in, never re-derived from the def, so a
    /// building keeps its size if the data changes later. Not written when 1x1.
    #[serde(default = "one_by_one", skip_serializing_if = "is_one_by_one")]
    pub size: [u8; 2],
    /// A production building's batch under way (`forge.rs`); `None` is a fresh, empty batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) craft: Option<CraftState>,
    /// A Map Room's progress toward its next chart, in ticks (`chart.rs`); `None` is 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) chart: Option<f32>,
    /// A farm plot's stage (`farm.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) plot: Option<FarmPlot>,
    /// An orchard's planting timer and trees (`orchard.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) orchard: Option<Orchard>,
    /// The building's own fire (`BuildingDef::heat`, `heat.rs`): lit for good when it finishes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) heat: Option<HeatSource>,
    /// Its fuel bunker (`BuildingDef::fuel`, `fuel.rs`): empty when it finishes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) fuel: Option<Bunker>,
    /// Its pickers' timer and counts (`BuildingDef::pickers`, `pickers.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) pickers: Option<Pickers>,
    /// Where it points: a chute's foot (`BuildingDef::chute`, `chute.rs`), set by
    /// `Command::SetLink`. `None` until set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Pos>,
    /// What a chute has sent down so far, one stack per item in the order first sent
    /// (`chute.rs`): its cursor panel's `Sent: 140 Stone`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sent: Vec<crate::jobs::Stack>,
}

impl Building {
    /// Whether its effect may run: it has no bunker, or its bunker holds at least one item.
    pub fn fuel_lit(&self) -> bool {
        self.fuel.is_none_or(|f| f.count > 0)
    }

    /// Its bunker, if it has one.
    pub fn bunker(&self) -> Option<Bunker> {
        self.fuel
    }

    /// The tiles it covers.
    pub fn footprint(&self) -> Footprint {
        Footprint::new(self.anchor, self.size[0], self.size[1])
    }
}

/// What a building does that some loop has to find it by. Derived from its def whenever the
/// indexes are built, never saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    Orchard,
    Farm,
    Irrigation,
    Power,
    /// A study hall: seats, and no chart (a Map Room's seats are Chart jobs instead).
    Study,
    /// A fuel bunker (`fuel.rs`).
    Fuel,
    /// Pickers (`pickers.rs`).
    Pickers,
    /// A Hothouse (`hothouse.rs`).
    Hothouse,
    /// A chute (`chute.rs`).
    Chute,
}

impl Kind {
    /// Every kind `def` is, in a fixed order.
    fn of(def: &BuildingDef) -> impl Iterator<Item = Kind> {
        [
            (def.orchard.is_some(), Kind::Orchard),
            (def.farm.is_some(), Kind::Farm),
            (def.irrigation.is_some(), Kind::Irrigation),
            (def.power.is_some(), Kind::Power),
            (def.seats > 0 && def.chart.is_none(), Kind::Study),
            (def.fuel.is_some(), Kind::Fuel),
            (def.pickers.is_some(), Kind::Pickers),
            (def.hothouse.is_some(), Kind::Hothouse),
            (def.chute.is_some(), Kind::Chute),
        ]
        .into_iter()
        .filter_map(|(is, kind)| is.then_some(kind))
    }
}

/// Every finished building, and the two indexes over them (module docs).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Buildings {
    /// The key the next building gets: past every key ever handed out.
    next: u32,
    records: BTreeMap<BuildingKey, Building>,
    /// Anchor `(y, x)` to key: the listing order.
    #[serde(skip)]
    rows: BTreeMap<(i32, i32), BuildingKey>,
    /// `(kind, anchor)` to key: the per-kind views, in `Pos` order.
    #[serde(skip)]
    kinds: BTreeMap<(Kind, Pos), BuildingKey>,
}

impl Buildings {
    /// Adds `record` under a fresh key and indexes it.
    fn insert(&mut self, record: Building, defs: &Defs) -> BuildingKey {
        let key = BuildingKey(self.next);
        self.next += 1;
        self.index(key, &record, defs);
        self.records.insert(key, record);
        key
    }

    /// Drops the record under `key` and its index entries.
    fn take(&mut self, key: BuildingKey, defs: &Defs) -> Option<Building> {
        let record = self.records.remove(&key)?;
        self.rows.remove(&(record.anchor.y, record.anchor.x));
        for kind in Kind::of(defs.building(record.id)) {
            self.kinds.remove(&(kind, record.anchor));
        }
        Some(record)
    }

    fn index(&mut self, key: BuildingKey, record: &Building, defs: &Defs) {
        self.rows.insert((record.anchor.y, record.anchor.x), key);
        for kind in Kind::of(defs.building(record.id)) {
            self.kinds.insert((kind, record.anchor), key);
        }
    }

    /// Rebuilds both indexes from the records, after a load, and gives each record whatever
    /// per-kind state its current def calls for but it lacks: an orchard's timer, an empty farm
    /// plot. State is filled, never dropped: a building whose def no longer has an orchard keeps
    /// its (now dormant) one.
    pub(crate) fn reindex(&mut self, defs: &Defs) {
        self.rows.clear();
        self.kinds.clear();
        let records = std::mem::take(&mut self.records);
        for (key, mut record) in records {
            let def = defs.building(record.id);
            if let (None, Some(orchard)) = (&record.orchard, &def.orchard) {
                record.orchard = Some(Orchard::new(orchard));
            }
            if record.plot.is_none() && def.farm.is_some() {
                record.plot = Some(FarmPlot::empty());
            }
            if record.fuel.is_none() && def.fuel.is_some() {
                record.fuel = Some(Bunker::default());
            }
            if let (None, Some(p)) = (&record.pickers, &def.pickers) {
                record.pickers = Some(Pickers::new(p));
            }
            self.index(key, &record, defs);
            self.records.insert(key, record);
        }
    }

    pub(super) fn get(&self, key: BuildingKey) -> Option<&Building> {
        self.records.get(&key)
    }

    pub(super) fn get_mut(&mut self, key: BuildingKey) -> Option<&mut Building> {
        self.records.get_mut(&key)
    }

    /// Every building, by anchor, row-major.
    fn in_rows(&self) -> impl Iterator<Item = &Building> + '_ {
        self.rows.values().map(|k| &self.records[k])
    }

    /// The buildings of one kind, by anchor in `Pos` order.
    fn of_kind(&self, kind: Kind) -> impl Iterator<Item = &Building> + '_ {
        let from = (kind, Pos::new(i32::MIN, i32::MIN));
        self.kinds
            .range(from..)
            .take_while(move |((k, _), _)| *k == kind)
            .map(|(_, key)| &self.records[key])
    }

    fn has_kind(&self, kind: Kind) -> bool {
        self.of_kind(kind).next().is_some()
    }

    /// Every building's own fire, lit or not, in no particular order. A fire whose fuel bunker
    /// is empty is out, so it isn't listed (`fuel.rs`).
    pub(super) fn fires(&self) -> impl Iterator<Item = &HeatSource> + '_ {
        self.records.values().filter(|b| b.fuel_lit()).filter_map(|b| b.heat.as_ref())
    }

    /// `fires`, to change.
    pub(super) fn fires_mut(&mut self) -> impl Iterator<Item = (BuildingId, &mut HeatSource)> {
        self.records.values_mut().filter_map(|b| Some((b.id, b.heat.as_mut()?)))
    }

    /// Checks the records against the map, for a world read from a save: every tile's key names
    /// a record whose footprint covers that tile, every record's footprint points back at it,
    /// anchors are unique, and `next` is past every key. O(map), once per load.
    fn check(&self, map: &TileMap) -> Result<(), String> {
        let mut anchors = BTreeSet::new();
        for (&key, b) in &self.records {
            if key.0 >= self.next {
                return Err(format!("building {} is past the next key {}", key.0, self.next));
            }
            if !anchors.insert(b.anchor) {
                return Err(format!("two buildings are anchored at {:?}", b.anchor));
            }
            if b.size[0] == 0 || b.size[1] == 0 {
                return Err(format!("building {} has no footprint", key.0));
            }
            for p in b.footprint().tiles() {
                let on_tile =
                    map.get(p).ok_or_else(|| format!("building {} is off the map", key.0))?;
                if on_tile.building.map(|t| t.key()) != Some(key) {
                    return Err(format!(
                        "{p:?} doesn't point at the building {} covering it",
                        key.0
                    ));
                }
            }
        }
        for (p, tile) in map.iter() {
            let Some(t) = tile.building else { continue };
            match self.records.get(&t.key()) {
                Some(b) if b.footprint().contains(p) => {}
                Some(_) => return Err(format!("{p:?} points at a building that doesn't cover it")),
                None => return Err(format!("{p:?} points at a building that isn't there")),
            }
        }
        Ok(())
    }
}

impl World {
    /// Stands the finished building `id` on `foot`: makes its record, with the state its def
    /// calls for (an orchard's planting timer, an empty farm plot, a lit fire), and points every
    /// footprint tile at it. Moves no units and opens no jobs: `raise` does that, for a building
    /// that finished construction. This is the only way a finished building gets onto the map;
    /// tests and tools use it too. `foot` must be on the map and free of buildings.
    pub fn put_building(&mut self, foot: Footprint, id: BuildingId, defs: &Defs) -> BuildingKey {
        let def = defs.building(id);
        let size = [foot.w, foot.h];
        let heat = def.heat.map(|heat| HeatSource {
            pos: foot.anchor,
            radius: heat.radius_for(self.modifiers.heating),
            sleep_bonus: heat.sleep_bonus,
            seek_tiles: heat.seek_tiles,
            until: HeatSource::FOREVER,
            size,
            cooks: true,
            ability: None,
        });
        if heat.is_some() {
            // A building's fire has always taken the place of a flame lit on its anchor.
            self.heat.remove(&foot.anchor);
        }
        let record = Building {
            id,
            anchor: foot.anchor,
            size,
            craft: None,
            chart: None,
            plot: def.farm.map(|_| FarmPlot::empty()),
            orchard: def.orchard.as_ref().map(Orchard::new),
            heat,
            fuel: def.fuel.map(|_| Bunker::default()),
            pickers: def.pickers.as_ref().map(Pickers::new),
            link: None,
            sent: Vec::new(),
        };
        let key = self.buildings.insert(record, defs);
        let tile = TileBuilding::new(key, def.passable, def.track.map_or(0, |t| t.index));
        for p in foot.tiles() {
            self.map[p].building = Some(tile);
        }
        self.map_changed();
        key
    }

    /// Takes the finished building `key` off the map and returns its record (`None` if there is
    /// none). Whoever is using it stops first, in this order: units inside leave by the door
    /// (which reads the footprint, so it runs before the tiles are cleared); units holding one
    /// of its Study, Chart, Craft, Sow or Reap jobs drop it, and those jobs are removed; units
    /// bound for it to sleep or heal (`bed`, `healing`) finish that need. Then the footprint's
    /// tiles are cleared, the record and its index entries removed, and `map_changed` called.
    /// It decides no refunds (the record carries a craft batch's deliveries) and doesn't ask
    /// whether the building may go (the gate). Planted orchard trees stay: they are map
    /// resources. Deconstruction (`deconstruct.rs`, #342) is the caller.
    pub fn remove_building(&mut self, key: BuildingKey, defs: &Defs) -> Option<Building> {
        let (anchor, foot) = {
            let b = self.buildings.get(key)?;
            (b.anchor, b.footprint())
        };
        for i in 0..self.units.len() {
            if self.units[i].inside == Some(anchor) {
                self.leave_building(i);
            }
        }
        let at_anchor = |k: &JobKind| {
            matches!(k, JobKind::Study { pos } | JobKind::Chart { pos } | JobKind::Craft { pos }
                | JobKind::Sow { pos } | JobKind::Reap { pos } | JobKind::Deconstruct { pos }
                | JobKind::Refuel { pos }
                if *pos == anchor)
        };
        let ids: Vec<JobId> =
            self.jobs.iter().filter(|j| at_anchor(&j.kind)).map(|j| j.id).collect();
        for i in 0..self.units.len() {
            let held = match self.units[i].job_task {
                Some(
                    Task::Study(id)
                    | Task::Chart(id)
                    | Task::Craft(id)
                    | Task::Sow(id)
                    | Task::Reap(id)
                    | Task::Deconstruct(id)
                    | Task::Refuel(id),
                ) if ids.contains(&id) => Some(id),
                _ => None,
            };
            if let Some(id) = held {
                self.abandon_job(i, id, defs);
            }
        }
        self.jobs.retain(|j| !ids.contains(&j.id));
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.bed == Some(anchor)
                || u.healing == Some(anchor)
                || u.need_task == Some(Task::Healing(anchor))
            {
                self.finish_need(i);
            }
        }
        for p in foot.tiles() {
            self.map[p].building = None;
        }
        let record = self.buildings.take(key, defs);
        self.map_changed();
        // A pair building's partner is left with no partner (`pair.rs`).
        if let Some(r) = &record
            && let Some(partner) = r.link
            && defs.building(r.id).pair.is_some()
        {
            self.unlink_partner(anchor, partner);
        }
        record
    }

    /// Every finished building once, in anchor order (row-major).
    pub fn buildings(&self) -> impl Iterator<Item = &Building> + '_ {
        self.buildings.in_rows()
    }

    /// The finished building covering `pos` (any tile of its footprint), if any.
    pub fn building_record(&self, pos: Pos) -> Option<&Building> {
        self.buildings.get(self.map.get(pos)?.building?.key())
    }

    /// The finished building anchored on `pos`, if any. Per-building state is asked for by
    /// anchor, as it was when it lived in tables keyed by anchor.
    pub(super) fn anchored(&self, pos: Pos) -> Option<&Building> {
        self.building_record(pos).filter(|b| b.anchor == pos)
    }

    /// `anchored`, to change.
    pub(super) fn anchored_mut(&mut self, pos: Pos) -> Option<&mut Building> {
        let key = self.map.get(pos)?.building?.key();
        self.buildings.get_mut(key).filter(|b| b.anchor == pos)
    }

    /// The finished buildings of one kind, by anchor in `Pos` order.
    pub(super) fn of_kind(&self, kind: Kind) -> impl Iterator<Item = &Building> + '_ {
        self.buildings.of_kind(kind)
    }

    /// Whether any finished building is of `kind`.
    pub(super) fn has_kind(&self, kind: Kind) -> bool {
        self.buildings.has_kind(kind)
    }

    /// The record checks `World::validate` runs on a loaded world (`Buildings::check`), and that
    /// no v1/v2 shim was left holding anything.
    pub(super) fn check_buildings(&self) -> Result<(), String> {
        if self.v2_map.width() > 0
            || !self.v2_crafts.is_empty()
            || !self.v2_charts.is_empty()
            || !self.v2_orchards.is_empty()
            || !self.v2_farm_plots.is_empty()
        {
            return Err("an old save's building tables weren't upgraded".into());
        }
        self.buildings.check(&self.map)
    }

    /// The buildings invariant (#600, `invariants.rs`): the tiles and records agree
    /// (`Buildings::check`), each record holds only state its def has a use for (a batch for a
    /// recipe, a chart for a Map Room, a plot for a farm, an orchard for an orchard), and both
    /// indexes list what a rebuild from the records would. `Err` names the first violation.
    pub(super) fn check_building_records(&self, defs: &Defs) -> Result<(), String> {
        self.buildings.check(&self.map)?;
        for b in self.buildings.records.values() {
            let def = defs.building(b.id);
            let stray = [
                ("craft", b.craft.is_some() && def.recipe.is_none()),
                ("chart", b.chart.is_some() && def.chart.is_none()),
                ("plot", b.plot.is_some() && def.farm.is_none()),
                ("orchard", b.orchard.is_some() && def.orchard.is_none()),
            ];
            if let Some((what, _)) = stray.iter().find(|(_, bad)| *bad) {
                return Err(format!(
                    "{} at {:?} holds {what} state it has no use for",
                    def.id, b.anchor
                ));
            }
        }
        let mut rebuilt = Buildings {
            next: self.buildings.next,
            records: self.buildings.records.clone(),
            ..Buildings::default()
        };
        rebuilt.reindex(defs);
        if rebuilt.rows != self.buildings.rows {
            return Err("the building listing doesn't match the records".into());
        }
        if rebuilt.kinds != self.buildings.kinds {
            return Err("the per-kind building index doesn't match the records".into());
        }
        Ok(())
    }

    /// Rebuilds the building indexes after a load (`Buildings::reindex`).
    /// Also re-stamps every building tile's track index and the cart rules' track steps from
    /// today's defs, since neither is saved (`TileBuilding::track`, `MoveRules::with_tracks`).
    pub(crate) fn reindex_buildings(&mut self, defs: &Defs) {
        self.buildings.reindex(defs);
        let stamps: Vec<(Footprint, u8)> = self
            .buildings
            .records
            .values()
            .map(|b| (b.footprint(), defs.building(b.id).track.map_or(0, |t| t.index)))
            .collect();
        for (foot, track) in stamps {
            for p in foot.tiles() {
                if let Some(tb) = self.map[p].building.as_mut() {
                    tb.set_track(track);
                }
            }
        }
        self.cart_rules = self.cart_rules.clone().with_tracks(defs.track_steps());
    }

    /// Turns a v1 or v2 body (read into the `v2_*` shims, `save/v2.rs`) into records: one per
    /// anchor tile, keyed in row-major order from 0, with every tile pointed at its anchor's
    /// record, and the old side tables' state (and each building fire) moved into the record
    /// anchored where it was. Entries anchored where no building stands are dropped, as loading
    /// always dropped them (a 2x2 orchard saved with an entry on every tile is one orchard,
    /// #624). Empties every shim. The indexes are rebuilt afterwards (`reindex_buildings`).
    pub(crate) fn upgrade_from_v2(&mut self) -> Result<(), String> {
        let old = std::mem::take(&mut self.v2_map);
        if !old.is_well_formed() {
            return Err("the map's size doesn't match its tiles".into());
        }
        let mut records = BTreeMap::new();
        let mut keys = BTreeMap::new();
        for (p, t) in old.iter() {
            let Some(b) = t.building.filter(v2::Building::is_anchor) else { continue };
            let key = BuildingKey(records.len() as u32);
            keys.insert(p, key);
            records.insert(
                key,
                Building {
                    id: b.id,
                    anchor: p,
                    size: b.size,
                    craft: None,
                    chart: None,
                    plot: None,
                    orchard: None,
                    heat: None,
                    fuel: None,
                    pickers: None,
                    link: None,
                    sent: Vec::new(),
                },
            );
        }
        let mut bad = None;
        let map = old.map_with(|p, t| {
            let building = t.building.and_then(|b| {
                let anchor = Pos::new(p.x - i32::from(b.part[0]), p.y - i32::from(b.part[1]));
                let key = keys.get(&anchor).copied();
                match key.map(|k| &records[&k]) {
                    Some(r) if r.size == b.size && r.footprint().contains(p) => {}
                    _ => {
                        bad.get_or_insert(p);
                        return None;
                    }
                }
                Some(TileBuilding::new(key?, b.passable, 0))
            });
            Tile {
                elevation: t.elevation,
                temperature: t.temperature,
                moisture: t.moisture,
                terrain: t.terrain,
                biome: t.biome,
                resource: t.resource,
                building,
            }
        });
        if let Some(p) = bad {
            return Err(format!("{p:?} is part of a building with no matching anchor"));
        }
        fn at<'r>(
            keys: &BTreeMap<Pos, BuildingKey>,
            records: &'r mut BTreeMap<BuildingKey, Building>,
            pos: Pos,
        ) -> Option<&'r mut Building> {
            records.get_mut(keys.get(&pos)?)
        }
        for (pos, craft) in std::mem::take(&mut self.v2_crafts) {
            if let Some(r) = at(&keys, &mut records, pos) {
                r.craft = Some(craft);
            }
        }
        for (pos, chart) in std::mem::take(&mut self.v2_charts) {
            if let Some(r) = at(&keys, &mut records, pos) {
                r.chart = Some(chart);
            }
        }
        for (pos, orchard) in std::mem::take(&mut self.v2_orchards) {
            if let Some(r) = at(&keys, &mut records, pos) {
                r.orchard = Some(orchard);
            }
        }
        for (pos, plot) in std::mem::take(&mut self.v2_farm_plots) {
            if let Some(r) = at(&keys, &mut records, pos) {
                r.plot = Some(plot);
            }
        }
        let fires: Vec<Pos> =
            self.heat.iter().filter(|(_, s)| s.is_permanent()).map(|(&p, _)| p).collect();
        for pos in fires {
            let fire = self.heat.remove(&pos).expect("just listed");
            if let Some(r) = at(&keys, &mut records, pos) {
                r.heat = Some(fire);
            }
        }
        self.buildings = Buildings { next: records.len() as u32, records, ..Buildings::default() };
        self.map = map;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
