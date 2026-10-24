//! Which frame of its character sheet a unit is drawn with (#165): the way it faces and, while it
//! walks, the step of the walk cycle. View state only; nothing here reads more of the sim than
//! the unit positions the client already follows, and nothing writes it.
//!
//! A sheet is 4 columns (down, up, left, right) by 6 rows (`sprites::characters`): row 0 is the
//! idle pose, rows 1-3 finish a 4-frame walk cycle (frame 2 has the legs together), and rows 4
//! and 5 are the bend pose.
//!
//! - **Facing** is the way the unit last stepped on the map (`facing_after`): the larger axis of
//!   the step, a tie (a diagonal) keeps the old facing, and an idle unit keeps its last one.
//! - **Walking** is the sprite visibly travelling toward its tile (`is_walking`), and the frame
//!   follows the distance it has covered (`walk_row`), so the feet keep pace with the ground and
//!   a faster unit steps faster. A unit that isn't travelling (idling, working, sleeping, or in
//!   a #110 pause) shows row 0 of its facing.
//! - **Pose override**: `UnitView::pose_override` beats the derived pose. `cast` (#166) sets it
//!   to the bend rows (4 and 5) while a unit bends, and turns the unit to its target; the picker
//!   doesn't change.
//!
//! The sprite itself is moved by `sync_units`, so the selection ring and the canopy fade, which
//! read its transform, follow it, and every frame is a 16px cell like the one it replaces.

use bevy::prelude::*;
use fn_sim::Pos;

use super::sprites::CELL;
use super::{Sim, UnitSprite, sync_units, unit_translation};

/// How far (world pixels) a unit walks per frame of its cycle; a cycle is 4 frames.
pub const STRIDE_PX: f32 = 6.0;
/// The sprite is walking while it is more than this far (world pixels) from its tile. The glide
/// toward a tile decays toward zero without reaching it, so a little slack tells "still arriving"
/// from "there".
pub const ARRIVED_PX: f32 = 0.75;
/// Rows of a sheet taken by the walk cycle.
pub const WALK_ROWS: u32 = 4;
/// The bend pose (#166): the stance held while channelling, then the release as the bend lands.
/// Each nation's stance is in its characters' sheets (`art/<character>.toml`, rows 4 and 5).
pub const BEND_STANCE_ROW: u32 = WALK_ROWS;
pub const BEND_RELEASE_ROW: u32 = WALK_ROWS + 1;

/// The way a unit faces; the column of its sheet.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Facing {
    #[default]
    Down,
    Up,
    Left,
    Right,
}

impl Facing {
    /// The sheet column (down, up, left, right).
    pub fn column(self) -> u32 {
        match self {
            Facing::Down => 0,
            Facing::Up => 1,
            Facing::Left => 2,
            Facing::Right => 3,
        }
    }
}

/// The facing after a step of `(dx, dy)` tiles (y grows down the map). The larger axis wins; a
/// tie (a diagonal step) and no step at all keep the `previous` facing.
pub fn facing_after(previous: Facing, dx: i32, dy: i32) -> Facing {
    match dx.abs().cmp(&dy.abs()) {
        std::cmp::Ordering::Greater if dx > 0 => Facing::Right,
        std::cmp::Ordering::Greater => Facing::Left,
        std::cmp::Ordering::Less if dy > 0 => Facing::Down,
        std::cmp::Ordering::Less => Facing::Up,
        std::cmp::Ordering::Equal => previous,
    }
}

/// What a unit is doing, as far as the sheet is concerned.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Pose {
    /// Standing: row 0.
    #[default]
    Stand,
    /// Walking, having covered `travelled` world pixels in all (only its fraction of a cycle
    /// matters).
    Walk { travelled: f32 },
    /// An explicit row of the sheet, for poses outside the walk rows: the bend pose of `cast`
    /// (#166).
    Sheet { row: u32 },
}

/// A cell of the sheet: its column and row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Frame {
    pub col: u32,
    pub row: u32,
}

impl Frame {
    /// The region of the sheet image, in pixels.
    pub fn rect(self) -> Rect {
        let (x, y) = (self.col as f32 * CELL, self.row as f32 * CELL);
        Rect::new(x, y, x + CELL, y + CELL)
    }
}

/// Whether the sprite, `remaining` world pixels short of its tile, is walking.
pub fn is_walking(remaining: f32) -> bool {
    remaining > ARRIVED_PX
}

/// The walk row after covering `travelled` pixels: one row per `STRIDE_PX`, 0, 1, 2, 3, 0, ...
pub fn walk_row(travelled: f32) -> u32 {
    (travelled.max(0.0) / STRIDE_PX) as u32 % WALK_ROWS
}

/// The pose a unit is in: the `override_pose` if there is one, else walking if it is travelling
/// and standing if it isn't.
pub fn pose_of(travelled: f32, walking: bool, override_pose: Option<Pose>) -> Pose {
    override_pose.unwrap_or(if walking { Pose::Walk { travelled } } else { Pose::Stand })
}

/// The cell of the sheet for a unit facing `facing` in `pose`.
pub fn pick_frame(facing: Facing, pose: Pose) -> Frame {
    let row = match pose {
        Pose::Stand => 0,
        Pose::Walk { travelled } => walk_row(travelled),
        Pose::Sheet { row } => row,
    };
    Frame { col: facing.column(), row }
}

/// A unit sprite's animation state.
#[derive(Component)]
pub struct UnitView {
    pub facing: Facing,
    /// The tile the sim had the unit on when we last looked, to see which way it stepped.
    seen_at: Pos,
    /// Where the sprite was last frame, to measure how far it moved.
    drawn_at: Vec2,
    /// Distance walked, for the cycle.
    travelled: f32,
    /// Set by whatever animates the unit beyond walking (#166); wins over the derived pose.
    pub pose_override: Option<Pose>,
}

impl UnitView {
    /// A unit standing facing down on tile `at`, drawn at `drawn_at`.
    pub fn new(at: Pos, drawn_at: Vec2) -> Self {
        UnitView {
            facing: Facing::Down,
            seen_at: at,
            drawn_at,
            travelled: 0.0,
            pose_override: None,
        }
    }

    /// The frame to draw now, for a sprite `remaining` pixels short of its tile.
    fn frame(&self, remaining: f32) -> Frame {
        let pose = pose_of(self.travelled, is_walking(remaining), self.pose_override);
        pick_frame(self.facing, pose)
    }
}

/// Points each unit sprite at the frame its facing and walk call for.
pub fn animate_units(
    sim: Res<Sim>,
    mut units: Query<(&UnitSprite, &Transform, &mut UnitView, &mut Sprite)>,
) {
    for (UnitSprite(id), tf, mut view, mut sprite) in &mut units {
        let Some(unit) = sim.world.unit(*id) else { continue };
        if unit.pos != view.seen_at {
            view.facing =
                facing_after(view.facing, unit.pos.x - view.seen_at.x, unit.pos.y - view.seen_at.y);
            view.seen_at = unit.pos;
        }
        let here = tf.translation.truncate();
        let remaining = here.distance(unit_translation(unit.pos).truncate());
        if is_walking(remaining) {
            view.travelled += here.distance(view.drawn_at);
        }
        view.drawn_at = here;
        let rect = view.frame(remaining).rect();
        if sprite.rect != Some(rect) {
            sprite.rect = Some(rect);
        }
    }
}

pub struct UnitAnimPlugin;

impl Plugin for UnitAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, animate_units.after(sync_units));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Facing; 4] = [Facing::Down, Facing::Up, Facing::Left, Facing::Right];

    #[test]
    fn columns_are_down_up_left_right() {
        assert_eq!(ALL.map(Facing::column), [0, 1, 2, 3]);
    }

    #[test]
    fn a_step_faces_its_direction_whatever_the_old_facing() {
        for old in ALL {
            assert_eq!(facing_after(old, 0, 1), Facing::Down, "{old:?}");
            assert_eq!(facing_after(old, 0, -1), Facing::Up, "{old:?}");
            assert_eq!(facing_after(old, -1, 0), Facing::Left, "{old:?}");
            assert_eq!(facing_after(old, 1, 0), Facing::Right, "{old:?}");
        }
    }

    #[test]
    fn a_longer_axis_wins_over_a_shorter_one() {
        assert_eq!(facing_after(Facing::Up, 3, 1), Facing::Right);
        assert_eq!(facing_after(Facing::Up, -3, -1), Facing::Left);
        assert_eq!(facing_after(Facing::Left, 1, 3), Facing::Down);
        assert_eq!(facing_after(Facing::Left, -1, -3), Facing::Up);
    }

    #[test]
    fn a_diagonal_tie_keeps_the_previous_facing() {
        for old in ALL {
            for (dx, dy) in [(1, 1), (1, -1), (-1, 1), (-1, -1), (2, 2)] {
                assert_eq!(facing_after(old, dx, dy), old, "{old:?} {dx},{dy}");
            }
        }
    }

    #[test]
    fn no_step_keeps_the_previous_facing() {
        for old in ALL {
            assert_eq!(facing_after(old, 0, 0), old);
        }
    }

    #[test]
    fn a_standing_unit_shows_row_zero_of_its_facing() {
        for f in ALL {
            assert_eq!(pick_frame(f, Pose::Stand), Frame { col: f.column(), row: 0 });
        }
        assert_eq!(pick_frame(Facing::Down, Pose::Stand), Frame { col: 0, row: 0 });
    }

    #[test]
    fn a_walking_unit_cycles_all_four_rows_in_each_facing() {
        for f in ALL {
            let rows: Vec<u32> = (0..8)
                .map(|n| {
                    let travelled = (n as f32 + 0.5) * STRIDE_PX;
                    let frame = pick_frame(f, Pose::Walk { travelled });
                    assert_eq!(frame.col, f.column());
                    frame.row
                })
                .collect();
            assert_eq!(rows, [0, 1, 2, 3, 0, 1, 2, 3], "{f:?}");
        }
    }

    #[test]
    fn the_walk_is_paced_by_distance() {
        assert_eq!(walk_row(0.0), 0);
        assert_eq!(walk_row(STRIDE_PX - 0.01), 0);
        assert_eq!(walk_row(STRIDE_PX), 1);
        assert_eq!(walk_row(2.0 * STRIDE_PX), 2);
        assert_eq!(walk_row(4.0 * STRIDE_PX), 0);
        assert_eq!(walk_row(-5.0), 0);
    }

    #[test]
    fn a_sprite_walks_until_it_has_arrived() {
        assert!(is_walking(16.0));
        assert!(is_walking(ARRIVED_PX + 0.01));
        assert!(!is_walking(ARRIVED_PX));
        assert!(!is_walking(0.0));
    }

    #[test]
    fn idle_and_moving_pick_different_rows_of_the_same_facing() {
        let walking = pose_of(2.0 * STRIDE_PX, true, None);
        let standing = pose_of(2.0 * STRIDE_PX, false, None);
        assert_eq!(pick_frame(Facing::Left, walking), Frame { col: 2, row: 2 });
        assert_eq!(pick_frame(Facing::Left, standing), Frame { col: 2, row: 0 });
    }

    #[test]
    fn a_pose_override_beats_walking_and_standing_and_keeps_the_facing() {
        let bend = Some(Pose::Sheet { row: 5 });
        for walking in [false, true] {
            let pose = pose_of(3.0 * STRIDE_PX, walking, bend);
            assert_eq!(pick_frame(Facing::Right, pose), Frame { col: 3, row: 5 });
        }
    }

    #[test]
    fn a_frame_is_a_16px_cell_of_the_sheet() {
        let r = Frame { col: 3, row: 2 }.rect();
        assert_eq!((r.min, r.max), (Vec2::new(48.0, 32.0), Vec2::new(64.0, 48.0)));
        // Every walk and bend cell stays inside a 64x96 sheet, and the bend rows are below the
        // walk rows.
        for f in ALL {
            for row in 0..=BEND_RELEASE_ROW {
                let r = Frame { col: f.column(), row }.rect();
                assert!(r.max.x <= 64.0 && r.max.y <= 96.0);
            }
        }
        assert_eq!((BEND_STANCE_ROW, BEND_RELEASE_ROW), (4, 5));
    }

    #[test]
    fn the_view_starts_facing_down_and_stands() {
        let view = UnitView::new(Pos::new(3, 4), Vec2::ZERO);
        assert_eq!(view.facing, Facing::Down);
        assert_eq!(view.frame(0.0), Frame { col: 0, row: 0 });
    }

    /// A test app with `animate_units` and one sprite for the first settler.
    fn sprite_app() -> (App, Entity) {
        let mut app = App::new();
        crate::insert_test_art(&mut app);
        let sim = Sim(fn_ui::Session::new(1, fn_sim::Nation::Earth, 48, 32));
        let unit = &sim.world.units[0];
        let (id, pos) = (unit.id, unit.pos);
        let character = crate::unit_character(&sim, unit).to_owned();
        let at = unit_translation(pos);
        let sprite = app.world().resource::<crate::Art>().unit_sprite(&character);
        app.insert_resource(sim).add_systems(Update, animate_units);
        let entity = app
            .world_mut()
            .spawn((
                sprite,
                Transform::from_translation(at),
                UnitSprite(id),
                UnitView::new(pos, at.truncate()),
            ))
            .id();
        (app, entity)
    }

    fn rect_of(app: &App, e: Entity) -> Rect {
        app.world().get::<Sprite>(e).unwrap().rect.unwrap()
    }

    #[test]
    fn a_unit_faces_its_step_walks_while_arriving_and_stands_when_there() {
        let (mut app, e) = sprite_app();
        app.update();
        assert_eq!(rect_of(&app, e), Frame { col: 0, row: 0 }.rect(), "starts idle, facing down");

        // The sim steps the unit one tile left; the sprite is still a tile behind it.
        let from = app.world().resource::<Sim>().world.units[0].pos;
        let to = Pos::new(from.x - 1, from.y);
        app.world_mut().resource_mut::<Sim>().world.units[0].pos = to;
        app.update();
        let mid = rect_of(&app, e);
        assert_eq!(mid.min.x, 2.0 * CELL, "left column while walking");

        // The sprite crosses the tile in a few frames, and the cycle moves on as it covers ground.
        let mut rows = vec![mid.min.y / CELL];
        for _ in 0..4 {
            let mut tf = app.world_mut().get_mut::<Transform>(e).unwrap();
            tf.translation.x -= 3.0;
            app.update();
            rows.push(rect_of(&app, e).min.y / CELL);
        }
        rows.dedup();
        assert!(rows.len() >= 3, "the walk cycle advances with the ground covered: {rows:?}");

        // Arrived: row 0 of the facing it ended with, which it keeps.
        let at = unit_translation(to);
        app.world_mut().get_mut::<Transform>(e).unwrap().translation = at;
        app.update();
        assert_eq!(rect_of(&app, e), Frame { col: 2, row: 0 }.rect());
        app.update();
        assert_eq!(
            rect_of(&app, e),
            Frame { col: 2, row: 0 }.rect(),
            "an idle unit keeps its facing"
        );
    }

    #[test]
    fn a_diagonal_step_keeps_the_facing() {
        let (mut app, e) = sprite_app();
        let from = app.world().resource::<Sim>().world.units[0].pos;
        app.world_mut().resource_mut::<Sim>().world.units[0].pos = Pos::new(from.x + 1, from.y);
        app.update();
        let (_, pos) = (0, app.world().resource::<Sim>().world.units[0].pos);
        app.world_mut().resource_mut::<Sim>().world.units[0].pos = Pos::new(pos.x + 1, pos.y + 1);
        app.update();
        assert_eq!(rect_of(&app, e).min.x, 3.0 * CELL, "still facing right after a diagonal step");
    }
}
