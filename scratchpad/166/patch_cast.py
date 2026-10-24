p='crates/client/src/cast.rs'
s=open(p).read()
def rep(a,b):
    global s
    assert a in s, a
    s=s.replace(a,b,1)
rep('''    let fx_sheet = sprites::fx::SHEET;
    debug_assert!(!fx_sheet.is_empty());
''','')
rep('''        let sprites: Vec<Entity> = casts.sprites.values().flatten().copied().collect();
        for e in sprites {''','''        let old: Vec<Entity> = casts.sprites.values().flatten().copied().collect();
        for e in old {''')
rep('''        let keep = if c.finished.is_none() { live.contains(unit) } else { !c.is_over(now) }
            && stand_of(*unit).is_some();''','''        let running = if c.finished.is_none() { live.contains(unit) } else { !c.is_over(now) };
        let keep = running && stand_of(*unit).is_some();''')
a=s.index("/// Wind Carry's stack is already in the stockpile")
b=s.index("#[cfg(test)]\nmod tests")
s=s[:a]+'''/// Wind Carry's stack is already in the stockpile when the event lands; hides it while its copy
/// flies there, if the tile was empty before (a tile that held something keeps showing that).
fn hide_landing(
    sim: Res<Sim>,
    mut casts: ResMut<Casts>,
    markers: Res<super::jobview::ItemMarkers>,
    mut visibility: Query<&mut Visibility>,
) {
    let wanted: BTreeSet<Pos> = casts
        .casts
        .values()
        .filter_map(|c| {
            let m = c.moved?;
            let t = c.time(sim.world.tick);
            let in_flight = t >= 0.0 && t <= c.scene().flight();
            let was_empty = sim.world.stored_at(m.to).is_some_and(|s| s.count == m.count);
            (in_flight && was_empty).then_some(m.to)
        })
        .collect();
    let mut set = |pos: Pos, v: Visibility| {
        if let Some(mut now) = markers.stored(pos).and_then(|e| visibility.get_mut(e).ok()) {
            if *now != v {
                *now = v;
            }
        }
    };
    // Every frame, since the marker is respawned when the stack changes.
    for pos in &wanted {
        set(*pos, Visibility::Hidden);
    }
    for pos in casts.hidden.difference(&wanted) {
        set(*pos, Visibility::Inherited);
    }
    casts.hidden = wanted;
}

'''+s[b:]
open(p,'w').write(s)

p='crates/client/src/unit_anim.rs'
s=open(p).read()
rep('''    /// An explicit row of the sheet, for poses outside the walk rows (#166's bending). Nothing
    /// sets one yet.
    #[allow(dead_code)]
    Sheet { row: u32 },''','''    /// An explicit row of the sheet, for poses outside the walk rows: the bend pose of `cast`
    /// (#166).
    Sheet { row: u32 },''')
rep('''/// Rows of a sheet taken by the walk cycle.
pub const WALK_ROWS: u32 = 4;''','''/// Rows of a sheet taken by the walk cycle.
pub const WALK_ROWS: u32 = 4;
/// The bend pose (#166): the stance held while channelling, then the release as the bend lands.
/// Each nation's stance is in its characters' sheets (`art/<character>.toml`, rows 4 and 5).
pub const BEND_STANCE_ROW: u32 = WALK_ROWS;
pub const BEND_RELEASE_ROW: u32 = WALK_ROWS + 1;''')
rep('''fn animate_units(''','''pub fn animate_units(''')
rep('''//! - **Pose override**: `UnitView::pose_override` beats the derived pose. Bending animations
//!   (#166) set it to a `Pose::Sheet` row beyond the walk rows; the picker doesn't change.''','''//! - **Pose override**: `UnitView::pose_override` beats the derived pose. `cast` (#166) sets it
//!   to the bend rows (4 and 5) while a unit bends, and turns the unit to its target; the picker
//!   doesn't change.''')
rep('''//! A sheet is 4 columns (down, up, left, right) by 4 rows (`sprites::characters`): row 0 is the
//! idle pose and rows 1-3 finish a 4-frame walk cycle (frame 2 has the legs together).''','''//! A sheet is 4 columns (down, up, left, right) by 6 rows (`sprites::characters`): row 0 is the
//! idle pose, rows 1-3 finish a 4-frame walk cycle (frame 2 has the legs together), and rows 4
//! and 5 are the bend pose.''')
rep('''        // Every walk cell stays inside a 64x64 sheet.
        for f in ALL {
            for row in 0..WALK_ROWS {
                let r = Frame { col: f.column(), row }.rect();
                assert!(r.max.x <= 64.0 && r.max.y <= 64.0);
            }
        }''','''        // Every walk and bend cell stays inside a 64x96 sheet, and the bend rows are below the
        // walk rows.
        for f in ALL {
            for row in 0..=BEND_RELEASE_ROW {
                let r = Frame { col: f.column(), row }.rect();
                assert!(r.max.x <= 64.0 && r.max.y <= 96.0);
            }
        }
        assert_eq!((BEND_STANCE_ROW, BEND_RELEASE_ROW), (4, 5));''')
open(p,'w').write(s)

p='crates/client/src/jobview.rs'
s=open(p).read()
rep("fn item_cell(defs: &Defs","pub fn item_cell(defs: &Defs")
rep('''/// The icon cell of an item''','''impl ItemMarkers {
    /// The marker entity of the stack stored on `pos`, if it has one.
    pub fn stored(&self, pos: Pos) -> Option<Entity> {
        self.0.get(&(pos, Place::Stored)).map(|(_, e)| *e)
    }
}

/// The icon cell of an item''')
open(p,'w').write(s)

p='crates/client/src/main.rs'
s=open(p).read()
rep("mod buildings;\nmod canopy;","mod buildings;\nmod canopy;\nmod cast;")
rep("use canopy::{CanopyFade, CanopyPlugin};","use canopy::{CanopyFade, CanopyPlugin};\nuse cast::CastPlugin;")
rep("            BendingPlugin,\n            ResearchPlugin,","            BendingPlugin,\n            CastPlugin,\n            ResearchPlugin,")
open(p,'w').write(s)
