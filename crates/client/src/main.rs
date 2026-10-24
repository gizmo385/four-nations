//! Bevy client: draws the `fn_sim` world and forwards player input. Owns no game rules.
//!
//!     cargo run -p fn_client -- [seed] [air|water|earth|fire] [width] [height]
//!
//! Controls: click a unit to select it (Esc deselects, and the camera centres it), WASD/arrows or mouse drag pan, mouse wheel
//! zoom, R new map, 1-4 switch nation, Space pause. H/X/P/B pick a designation tool (harvest,
//! cancel harvest, stockpile, the nation's bending ability): drag a rectangle with the left
//! button, middle/right still pan; Esc returns to selecting. `T` opens the research screen (Esc
//! closes it); map input is suspended while it's open. See `designate`, `bending` and `research`.
//!
//! `FN_SCREENSHOT=out.png` runs headless: no window is created at all (winit is disabled and the
//! camera renders into an offscreen `Image`), and once the art has loaded it saves one frame as a
//! PNG and exits. `FN_SCREENSHOT_SIZE=WIDTHxHEIGHT` sets the image size (default 1280x720),
//! `FN_SELECT=N` starts with unit N selected so its panel shows in the shot, and
//! `FN_RESEARCH_OPEN=1` starts with the research screen (`T`) open, and `FN_BUILD_DEMO=1`
//! starts with the build tool (`U`) active and a construction site designated beside the first
//! unit for every building the nation can place, so the picker and the site sprites show in the
//! shot. `FN_BUILD_DEMO=buildings` also levels a meadow and puts a finished building of every
//! kind in the data on it, to see the art. `FN_STORES_DEMO=1` harvests what is near the settlers
//! into a stockpile and runs the game until the Stores section has rows to show. `FN_FX_DEMO=1`
//! has the settlers cast the nation's ability on tiles near the first unit, so a lit flame or a
//! wet patch is on the map when the shot is taken. Agents use this, via
//! `cargo xtask screenshot SEED NATION OUT.png [WIDTH HEIGHT]`, to check visual changes without a
//! window popping up and stealing the human's focus. `FN_CURSOR=X,Y` puts the cursor on map tile
//! X,Y (tiles may be fractional), since a headless run has no mouse, so the tree fade near the
//! cursor shows in the shot (#77). `FN_CANOPY_DEMO=TICKS` first moves the settlers into
//! the densest forest near them, designates trees there and runs TICKS ticks, and `FN_CANOPY_FADE=off` turns the fade off for a "before" shot.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use bending::{BendingPlugin, TerrainSprite};
use bevy::app::ScheduleRunnerPlugin;
use bevy::camera::RenderTarget;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::sprite::Anchor;
use bevy::window::{ExitCondition, PrimaryWindow};
use bevy::winit::WinitPlugin;
use buildings::BuildingsPlugin;
use canopy::{CanopyFade, CanopyPlugin};
use designate::{ActiveTool, DesignatePlugin};
use fn_sim::world::{TICKS_PER_SECOND, Unit, UnitId};
use fn_sim::{MapGenConfig, Nation, Pos, Tile};
use fn_ui::{Session, Tool};
use hud::HudPlugin;
use jobview::{JobViewPlugin, ResourceSprite};
use research::{ResearchPlugin, ScreenOpen};

mod bending;
mod buildings;
mod canopy;
mod designate;
mod hud;
mod jobview;
mod research;
mod sprites;
mod ui_style;

const TILE: f32 = 16.0;
const ASSET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let nation = args.next().and_then(|s| parse_nation(&s)).unwrap_or(Nation::Earth);
    let defaults = MapGenConfig::default();
    let width = args.next().and_then(|s| s.parse().ok()).unwrap_or(defaults.width);
    let height = args.next().and_then(|s| s.parse().ok()).unwrap_or(defaults.height);
    let headless = std::env::var_os("FN_SCREENSHOT").map(|path| Headless {
        path: path.into(),
        size: std::env::var("FN_SCREENSHOT_SIZE")
            .ok()
            .map(|s| parse_size(&s).unwrap_or_else(|| panic!("bad FN_SCREENSHOT_SIZE {s:?}")))
            .unwrap_or(UVec2::new(1280, 720)),
    });

    let plugins = DefaultPlugins
        .set(AssetPlugin { file_path: ASSET_DIR.into(), ..default() })
        .set(ImagePlugin::default_nearest());
    let mut app = App::new();
    if let Some(headless) = headless {
        // No window, no winit: frames are driven by the schedule runner and the camera renders
        // into an offscreen image (see `setup_camera_and_ui`).
        app.add_plugins((
            plugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<WinitPlugin>(),
            ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 60.0)),
        ))
        .insert_resource(headless)
        .add_systems(Update, headless_screenshot);
    } else {
        app.add_plugins(plugins.set(WindowPlugin {
            primary_window: Some(Window { title: "The Four Nations".into(), ..default() }),
            ..default()
        }));
    }
    let mut sim = Session::new(seed, nation, width, height);
    let build_demo = std::env::var_os("FN_BUILD_DEMO").is_some();
    if build_demo {
        designate_demo_sites(
            &mut sim,
            std::env::var("FN_BUILD_DEMO").is_ok_and(|v| v == "buildings"),
        );
    }
    if std::env::var_os("FN_STORES_DEMO").is_some() {
        fill_demo_stores(&mut sim);
    }
    if let Some(ticks) = std::env::var("FN_CANOPY_DEMO").ok().and_then(|s| s.parse().ok()) {
        canopy::place_demo_units(&mut sim, ticks);
    }
    if std::env::var_os("FN_FX_DEMO").is_some() {
        cast_demo_effects(&mut sim);
    }
    app.insert_resource(Time::<Fixed>::from_hz(f64::from(TICKS_PER_SECOND)))
        .insert_resource(ActiveTool(if build_demo { Tool::Build } else { Tool::Select }))
        .insert_resource(Sim(sim))
        .insert_resource(Selection(
            std::env::var("FN_SELECT").ok().and_then(|s| s.parse().ok()).map(UnitId),
        ))
        .insert_resource(ScreenOpen(std::env::var_os("FN_RESEARCH_OPEN").is_some()))
        .add_message::<MapClicked>()
        .add_plugins((
            DesignatePlugin,
            JobViewPlugin,
            BuildingsPlugin,
            HudPlugin,
            BendingPlugin,
            ResearchPlugin,
            CanopyPlugin,
        ))
        .add_systems(Startup, (load_art, setup_camera_and_ui, spawn_world).chain())
        .add_systems(FixedUpdate, tick_sim)
        .add_systems(
            Update,
            (
                controls,
                camera_controls.run_if(screen_closed),
                sync_units,
                despawn_dead_units,
                select_units.run_if(screen_closed),
                center_on_selection,
                draw_selection,
            )
                .chain(),
        )
        .run();
}

/// Present when running as a headless screenshot (`FN_SCREENSHOT`).
#[derive(Resource)]
struct Headless {
    path: PathBuf,
    size: UVec2,
}

/// Parses `WIDTHxHEIGHT`, e.g. `1280x720`.
fn parse_size(s: &str) -> Option<UVec2> {
    let (w, h) = s.split_once('x')?;
    let size = UVec2::new(w.parse().ok()?, h.parse().ok()?);
    (size.min_element() > 0).then_some(size)
}

fn parse_nation(s: &str) -> Option<Nation> {
    Nation::ALL.into_iter().find(|n| n.as_str() == s)
}

/// A run condition for map input systems: suspended while the research screen (`T`) covers the
/// map, so a click or drag meant for a tech card never also moves the camera or selects a unit.
fn screen_closed(open: Res<ScreenOpen>) -> bool {
    !open.0
}

/// The game being played: `fn_ui`'s `Session` (world, data and clock) as a Bevy resource.
#[derive(Resource, Deref, DerefMut)]
struct Sim(Session);

/// `FN_BUILD_DEMO`: designates each building in the data on a free tile near the first unit (the
/// ones this nation can't place yet are refused, which is fine). With `finished` (the value
/// `FN_BUILD_DEMO=buildings`) it also levels a meadow beside them and puts a finished one of every
/// building in the data on it, in a grid, so one shot shows the sites and all the building art.
fn designate_demo_sites(session: &mut Session, finished: bool) {
    let Some(origin) = session.world.units.first().map(|u| u.pos) else { return };
    if finished {
        level_meadow(session, origin);
    }
    let mut dy = 0;
    for i in 0..session.defs.buildings.len() {
        let building = fn_sim::building::BuildingId(i as u16);
        let pos = Pos::new(origin.x + 2, origin.y + dy);
        if session.apply(fn_sim::world::Command::DesignateBuild { pos, building }).is_ok() {
            dy += 2;
        }
    }
    if !finished {
        return;
    }
    // Four columns, 4 tiles apart so the 2x2 sprites don't overlap, to the right of the sites.
    for i in 0..session.defs.buildings.len() {
        let id = fn_sim::building::BuildingId(i as u16);
        let passable = session.defs.building(id).passable;
        let pos = Pos::new(origin.x + 6 + 4 * (i as i32 % 4), origin.y - 6 + 4 * (i as i32 / 4));
        if let Some(tile) = session.world.map.get_mut(pos) {
            tile.building = Some(fn_sim::building::Building { id, passable });
        }
    }
}

/// `FN_BUILD_DEMO=buildings` only: turns the ground around `origin` into open land with nothing on
/// it (in the biome the first unit stands in), so the building art isn't lost in a forest or a lake in the shot.
fn level_meadow(session: &mut Session, origin: Pos) {
    let Some(biome) = session.world.map.get(origin).map(|t| t.biome) else { return };
    for y in origin.y - 9..=origin.y + 8 {
        for x in origin.x..=origin.x + 20 {
            if let Some(tile) = session.world.map.get_mut(Pos::new(x, y)) {
                tile.terrain = fn_sim::Terrain::Land;
                tile.biome = biome;
                tile.resource = None;
            }
        }
    }
}

/// `FN_STORES_DEMO`: designates every harvest near the first unit and a stockpile beside it, then
/// runs the game (at most 20 in-game minutes) until at least five kinds of item are stored.
fn fill_demo_stores(session: &mut Session) {
    use fn_sim::world::Command;
    let Some(from) = session.world.units.first().map(|u| u.pos) else { return };
    let near = |p: Pos, w: i32, h: i32| (p.x - from.x).abs() < w && (p.y - from.y).abs() < h;
    let harvests: Vec<Pos> = session
        .world
        .map
        .iter()
        .filter(|(p, t)| {
            near(*p, 30, 20) && t.resource.is_some_and(|r| session.defs.harvest(r).is_some())
        })
        .map(|(p, _)| p)
        .collect();
    for pos in harvests {
        let _ = session.apply(Command::DesignateHarvest { pos });
    }
    let mut zone: Vec<Pos> = session
        .world
        .map
        .iter()
        .filter(|(_, t)| t.terrain == fn_sim::Terrain::Land && t.resource.is_none())
        .map(|(p, _)| p)
        .collect();
    zone.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    for pos in zone.into_iter().take(12) {
        let _ = session.apply(Command::DesignateStockpile { pos });
    }
    for _ in 0..20 * 60 {
        session.tick_n(TICKS_PER_SECOND);
        if fn_ui::view::stores(session).len() >= 5 {
            break;
        }
    }
}

/// `FN_FX_DEMO`: designates the nation's bending ability on a few tiles near the first unit it
/// accepts, then runs the game (at most 3 in-game minutes) until each cast has finished, so the
/// effect it leaves is on the map.
fn cast_demo_effects(session: &mut Session) {
    use fn_sim::world::Command;
    let Some(ability) = fn_ui::tools::ability_for_nation(&session.defs, session.world.nation)
    else {
        return;
    };
    let Some(from) = session.world.units.first().map(|u| u.pos) else { return };
    // Clear ground, so no tree canopy hides the effect.
    let world = &session.world;
    let clear = |p: Pos| {
        (-2..=2).all(|dy| {
            (-2..=2).all(|dx| {
                world.map.get(Pos::new(p.x + dx, p.y + dy)).is_none_or(|t| t.resource.is_none())
            })
        })
    };
    let mut targets: Vec<Pos> = world.map.iter().map(|(p, _)| p).filter(|p| clear(*p)).collect();
    targets.sort_by_key(|p| ((p.x - from.x).pow(2) + (p.y - from.y).pow(2), *p));
    let mut cast = 0;
    for pos in targets.into_iter().filter(|p| (p.x + p.y) % 3 == 0) {
        if cast < 4 && session.apply(Command::Bend { ability, pos }).is_ok() {
            cast += 1;
        }
    }
    for _ in 0..3 * 60 {
        session.tick_n(TICKS_PER_SECOND);
        if session.world.bend_events().len() >= cast {
            break;
        }
    }
}

/// The images the game draws with. A sheet is named by its asset path (`sprites/`) and loaded the
/// first time something asks for it, so a new sheet is only a new path constant in a family module.
#[derive(Resource)]
struct Art {
    assets: AssetServer,
    /// The tree trunks alone (`canopy::build_trunks`), in the layout of the tree sheet: blank
    /// until the sheet has loaded.
    trunks: Handle<Image>,
    /// Every sheet asked for so far, by asset path.
    images: Mutex<HashMap<&'static str, Handle<Image>>>,
}

/// The images of one character (`sprites::characters`).
struct CharacterArt {
    sheet: Handle<Image>,
    face: Handle<Image>,
}

impl Art {
    fn new(assets: AssetServer) -> Self {
        Art { assets, trunks: Handle::default(), images: Mutex::default() }
    }

    /// The image at asset `path`, loaded on first use.
    fn image(&self, path: &'static str) -> Handle<Image> {
        let mut images = self.images.lock().expect("art lock");
        images.entry(path).or_insert_with(|| self.assets.load(path)).clone()
    }

    /// Whether every image asked for so far has finished loading.
    fn all_loaded(&self) -> bool {
        let images = self.images.lock().expect("art lock");
        images.values().all(|h| self.assets.is_loaded_with_dependencies(h))
    }

    /// The sheet and portrait of a character id (`characters` in `data/nations.toml`).
    fn character(&self, id: &str) -> CharacterArt {
        CharacterArt {
            sheet: self.image(sprites::character_sheet_path(id)),
            face: self.image(sprites::character_face_path(id)),
        }
    }

    /// A unit's sprite: the character's down-facing idle frame, at native size.
    fn unit_sprite(&self, character: &str) -> Sprite {
        let [x0, y0, x1, y1] = sprites::IDLE_DOWN;
        Sprite {
            image: self.character(character).sheet,
            rect: Some(Rect::new(x0, y0, x1, y1)),
            ..default()
        }
    }

    /// A tree's trunk without its canopy: `cell` (a tree on the tree sheet) of the trunk image.
    fn trunk_sprite(&self, cell: sprites::Cell) -> Sprite {
        let [x0, y0, x1, y1] = cell.pixel_rect();
        Sprite { image: self.trunks.clone(), rect: Some(Rect::new(x0, y0, x1, y1)), ..default() }
    }

    /// A sprite showing `cell` of its sheet, at native size.
    fn sprite(&self, cell: sprites::Cell) -> Sprite {
        let [x0, y0, x1, y1] = cell.pixel_rect();
        Sprite { image: self.image(cell.sheet), rect: Some(Rect::new(x0, y0, x1, y1)), ..default() }
    }
}

/// Adds what `Art` needs to a test `App`: an asset server (it never has to find a file, since a
/// handle is returned at once) and the `Art` resource over it.
#[cfg(test)]
fn insert_test_art(app: &mut App) {
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin { file_path: ASSET_DIR.into(), ..default() },
    ))
    .init_asset::<Image>();
    let art = Art::new(app.world().resource::<AssetServer>().clone());
    app.insert_resource(art);
}

/// Everything that belongs to the current map; despawned on regenerate.
#[derive(Component)]
struct MapEntity;

#[derive(Component)]
struct UnitSprite(UnitId);

/// The selected unit, if any. By id, because dead units leave `World::units` and shift indices.
#[derive(Resource, Default)]
struct Selection(Option<UnitId>);

/// A left click on the map that wasn't part of a drag, in world coordinates.
#[derive(Message)]
struct MapClicked(Vec2);

fn load_art(
    mut commands: Commands,
    assets: Res<AssetServer>,
    sim: Res<Sim>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut art = Art::new((*assets).clone());
    art.trunks = images.add(canopy::blank_trunks());
    // Buildings appear mid-game; loading the sheet now keeps the headless screenshot waiting for it.
    for sheet in sprites::buildings::SHEETS {
        art.image(sheet);
    }
    // Start every nation's characters loading now, so the headless screenshot waits for them.
    for n in Nation::ALL {
        for id in &sim.defs.nation(n).characters {
            art.character(id);
        }
    }
    commands.insert_resource(art);
}

fn setup_camera_and_ui(
    mut commands: Commands,
    sim: Res<Sim>,
    headless: Option<Res<Headless>>,
    mut images: ResMut<Assets<Image>>,
) {
    let map = &sim.world.map;
    let mut camera = commands.spawn((
        Camera2d,
        // UI only picks a camera by itself when that camera draws to the primary window.
        IsDefaultUiCamera,
        Transform::from_xyz(
            map.width() as f32 * TILE / 2.0,
            -map.height() as f32 * TILE / 2.0,
            0.0,
        ),
    ));
    if let Some(headless) = headless {
        let mut target = Image::new_target_texture(
            headless.size.x,
            headless.size.y,
            TextureFormat::Rgba8UnormSrgb,
            None,
        );
        target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        camera.insert(RenderTarget::Image(images.add(target).into()));
    }
}

fn tile_translation(p: Pos, z: f32) -> Vec3 {
    Vec3::new(p.x as f32 * TILE, -p.y as f32 * TILE, z)
}

/// The z of things that stand on the ground in row `y`: resources, item markers and units. Rows
/// further down the screen draw on top, so a tree's canopy covers whatever stands behind it
/// (above it on screen) and a unit in front of the tree walks over its trunk. Everything in the
/// band sits above the floor (z 0 and 0.5) and below designation overlays (4.5).
pub fn row_depth(y: i32) -> f32 {
    1.0 + y as f32 * 0.001
}

/// Within one row: item markers over the tree on their tile, units over both.
pub const MARKER_BIAS: f32 = 0.0002;
const UNIT_BIAS: f32 = 0.0005;

/// A unit's sprite is centred on its tile, 16px at 1:1, so the feet are `UNIT_FEET` below centre.
const UNIT_FEET: f32 = -6.0;

fn unit_translation(p: Pos) -> Vec3 {
    tile_translation(p, row_depth(p.y) + UNIT_BIAS)
}

/// The art id of the character a unit is drawn as; the nation's `characters` list is data.
fn unit_character<'a>(sim: &'a Sim, unit: &Unit) -> &'a str {
    sim.defs.nation(unit.nation).character(unit.variant)
}

/// The ground rule for a tile; `sprites::GROUND` has a row for every terrain and biome.
fn ground_rule(sim: &Sim, t: &Tile) -> &'static sprites::GroundRule {
    let biome = sim.defs.biome(t.biome).id.as_str();
    sprites::ground_rule(t.terrain, biome)
        .unwrap_or_else(|| panic!("no ground sprite for {:?} in {biome}", t.terrain))
}

/// The floor cell for the tile at `p`. Water tiles are bitmask-autotiled against their neighbors
/// (#8, `sprites::water_tile`), and mountain tiles with lower ground beside them draw a cliff
/// (#104, `sprites::cliff_cell`); everything else is the ground rule's own pick.
fn floor_cell(sim: &Sim, p: Pos, rule: &sprites::GroundRule) -> sprites::Cell {
    let map = &sim.world.map;
    match (rule.water, rule.cliff) {
        (Some(set), _) => sprites::water_tile(map, p, set),
        (None, Some(set)) => sprites::cliff_cell(map, p, set).unwrap_or_else(|| rule.floor_at(p)),
        (None, None) => rule.floor_at(p),
    }
}

/// The sprites of a tile's resource. Sprites are bottom-aligned to their tile, so a 2x2 tree
/// stands on its tile and its canopy overlaps the tiles above and beside it.
struct ResourceSprites {
    /// The resource, or for a tree its trunk.
    base: Sprite,
    /// A tree's canopy, a child of `base` that fades when it hides something (`canopy`).
    canopy: Option<Sprite>,
}

fn resource_sprites(art: &Art, sim: &Sim, t: &Tile) -> Option<ResourceSprites> {
    let res = sim.defs.resource(t.resource?).id.as_str();
    let biome = sim.defs.biome(t.biome).id.as_str();
    let rule = sprites::resource_rule(res, biome)
        .unwrap_or_else(|| panic!("no sprite for resource {res} in {biome}"));
    Some(if sprites::trees::is_tree(rule.cell) {
        ResourceSprites {
            base: art.trunk_sprite(rule.cell),
            canopy: Some(art.sprite(sprites::trees::canopy_of(rule.cell))),
        }
    } else {
        ResourceSprites { base: art.sprite(rule.cell), canopy: None }
    })
}

fn spawn_world(mut commands: Commands, art: Res<Art>, sim: Res<Sim>) {
    spawn_entities(&mut commands, &art, &sim);
}

fn spawn_entities(commands: &mut Commands, art: &Art, sim: &Sim) {
    for (p, t) in sim.world.map.iter() {
        let rule = ground_rule(sim, t);
        commands.spawn((
            art.sprite(floor_cell(sim, p, rule)),
            Transform::from_translation(tile_translation(p, 0.0)),
            // Bending can reshape a tile's terrain later (Earth's Raise Earth, #68); this is
            // what lets `bending::sync_terrain` redraw it when that happens.
            TerrainSprite(p, t.terrain),
            MapEntity,
        ));
        if let Some(parts) = resource_sprites(art, sim, t) {
            // Lower rows draw on top so tall sprites overlap correctly. The anchor is the bottom
            // middle of the tile, so a sprite of any height stands on its tile.
            let mut tf = Transform::from_translation(tile_translation(p, row_depth(p.y)));
            tf.translation.y -= TILE / 2.0;
            let mut resource = commands.spawn((
                parts.base,
                Anchor::BOTTOM_CENTER,
                tf,
                ResourceSprite(p),
                MapEntity,
            ));
            if let Some(canopy) = parts.canopy {
                resource.with_children(|tree| {
                    tree.spawn((
                        canopy,
                        Anchor::BOTTOM_CENTER,
                        // A hair above the trunk; their pixels never overlap.
                        Transform::from_xyz(0.0, 0.0, canopy::CANOPY_Z),
                        CanopyFade::new(p),
                    ));
                });
            }
        } else if let Some(cell) = rule.decor_on(&sim.world.map, p) {
            commands.spawn((
                art.sprite(cell),
                Transform::from_translation(tile_translation(p, 0.5)),
                MapEntity,
            ));
        }
    }
    for u in &sim.world.units {
        commands.spawn((
            art.unit_sprite(unit_character(sim, u)),
            Transform::from_translation(unit_translation(u.pos)),
            UnitSprite(u.id),
            MapEntity,
        ));
    }
}

/// One `FixedUpdate` step (`TICKS_PER_SECOND` a second): the clock says how many ticks that is,
/// none while paused.
fn tick_sim(mut sim: ResMut<Sim>) {
    let ticks = sim.clock.per_fixed_step();
    sim.tick_n(ticks);
}

fn controls(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<Sim>,
    mut selection: ResMut<Selection>,
    art: Res<Art>,
    existing: Query<Entity, With<MapEntity>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        selection.0 = None;
    }
    if keys.just_pressed(KeyCode::Space) {
        sim.clock.toggle_pause();
    }
    let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    let nation = digits.iter().position(|k| keys.just_pressed(*k)).map(|i| Nation::ALL[i]);
    let new_seed = keys.just_pressed(KeyCode::KeyR).then(|| sim.world.seed.wrapping_add(1));
    if nation.is_none() && new_seed.is_none() {
        return;
    }
    let seed = new_seed.unwrap_or(sim.world.seed);
    let (width, height) = (sim.world.map.width(), sim.world.map.height());
    sim.0 = Session::new(seed, nation.unwrap_or(sim.world.nation), width, height);
    selection.0 = None;
    for e in &existing {
        commands.entity(e).despawn();
    }
    spawn_entities(&mut commands, &art, &sim);
}

/// Mouse movement (in screen pixels) after which a press becomes a drag rather than a click.
const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Default)]
struct DragState {
    last: Option<Vec2>,
    travelled: f32,
}

fn camera_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    sim: Res<Sim>,
    tool: Res<ActiveTool>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut drag: Local<DragState>,
    mut clicks: MessageWriter<MapClicked>,
    mut camera: Query<(&mut Transform, &mut Projection, &Camera, &GlobalTransform), With<Camera2d>>,
) {
    let (Ok(window), Ok((mut tf, mut projection, cam, cam_tf))) =
        (window.single(), camera.single_mut())
    else {
        return;
    };
    let Projection::Orthographic(ortho) = &mut *projection else { return };
    let map_size = Vec2::new(sim.world.map.width() as f32, sim.world.map.height() as f32) * TILE;

    // Zoom, but never out past the point where the whole map fits on screen.
    let fit_scale = (map_size / window.size()).max_element().max(0.2);
    if scroll.delta.y != 0.0 {
        ortho.scale *= 1.0 - scroll.delta.y.signum() * 0.1;
    }
    ortho.scale = ortho.scale.clamp(0.2, fit_scale);

    // Keyboard panning.
    let mut dir = Vec2::ZERO;
    for (keys_for, d) in [
        ([KeyCode::KeyW, KeyCode::ArrowUp], Vec2::Y),
        ([KeyCode::KeyS, KeyCode::ArrowDown], -Vec2::Y),
        ([KeyCode::KeyA, KeyCode::ArrowLeft], -Vec2::X),
        ([KeyCode::KeyD, KeyCode::ArrowRight], Vec2::X),
    ] {
        if keys_for.iter().any(|k| keys.pressed(*k)) {
            dir += d;
        }
    }
    tf.translation +=
        (dir.normalize_or_zero() * 600.0 * ortho.scale * time.delta_secs()).extend(0.0);

    // Grab-and-drag panning with any mouse button (a designation tool owns the left one). Screen
    // y points down, world y points up.
    let selecting = tool.0 == Tool::Select;
    let buttons: &[MouseButton] = if selecting {
        &[MouseButton::Left, MouseButton::Middle, MouseButton::Right]
    } else {
        &[MouseButton::Middle, MouseButton::Right]
    };
    let cursor = window.cursor_position();
    if buttons.iter().any(|b| mouse.just_pressed(*b)) {
        *drag = DragState { last: cursor, travelled: 0.0 };
    }
    if buttons.iter().any(|b| mouse.pressed(*b))
        && let (Some(last), Some(now)) = (drag.last, cursor)
    {
        let delta = now - last;
        drag.travelled += delta.length();
        if drag.travelled > DRAG_THRESHOLD {
            tf.translation.x -= delta.x * ortho.scale;
            tf.translation.y += delta.y * ortho.scale;
        }
        drag.last = Some(now);
    }
    if selecting
        && mouse.just_released(MouseButton::Left)
        && drag.travelled <= DRAG_THRESHOLD
        && let Some(world) = cursor.and_then(|c| cam.viewport_to_world_2d(cam_tf, c).ok())
    {
        clicks.write(MapClicked(world));
    }

    // Tiles are centered on their coordinates, so the map extends half a tile past them.
    let map_min = Vec2::new(-TILE / 2.0, -(map_size.y - TILE / 2.0));
    let map_max = Vec2::new(map_size.x - TILE / 2.0, TILE / 2.0);
    let half_view = window.size() * ortho.scale / 2.0;
    let clamped = clamp_camera(tf.translation.truncate(), half_view, map_min, map_max);
    tf.translation = clamped.extend(tf.translation.z);
}

/// Clamps a camera center so a view with half-extents `half_view` stays inside the rectangle
/// `[min, max]`. On any axis where the view is larger than the map, the map is centered instead.
fn clamp_camera(center: Vec2, half_view: Vec2, min: Vec2, max: Vec2) -> Vec2 {
    let axis = |c: f32, h: f32, lo: f32, hi: f32| {
        if hi - lo <= 2.0 * h { (lo + hi) / 2.0 } else { c.clamp(lo + h, hi - h) }
    };
    Vec2::new(axis(center.x, half_view.x, min.x, max.x), axis(center.y, half_view.y, min.y, max.y))
}

/// Smoothly moves unit sprites toward their sim positions.
fn sync_units(sim: Res<Sim>, time: Res<Time>, mut units: Query<(&UnitSprite, &mut Transform)>) {
    let t = 1.0 - (-time.delta_secs() * 12.0).exp();
    for (UnitSprite(id), mut tf) in &mut units {
        let Some(u) = sim.world.unit(*id) else { continue };
        let target = unit_translation(u.pos);
        tf.translation = tf.translation.lerp(target, t);
    }
}

/// Removes the sprites of units that have died, and clears the selection if it was one of them.
fn despawn_dead_units(
    mut commands: Commands,
    sim: Res<Sim>,
    mut selection: ResMut<Selection>,
    units: Query<(Entity, &UnitSprite)>,
) {
    for (entity, UnitSprite(id)) in &units {
        if sim.world.unit(*id).is_none() {
            commands.entity(entity).despawn();
        }
    }
    if selection.0.is_some_and(|id| sim.world.unit(id).is_none()) {
        selection.0 = None;
    }
}

fn cursor_tile(window: &Window, camera: &Camera, camera_tf: &GlobalTransform) -> Option<Pos> {
    let cursor = window.cursor_position()?;
    let world = camera.viewport_to_world_2d(camera_tf, cursor).ok()?;
    Some(Pos::new((world.x / TILE).round() as i32, (-world.y / TILE).round() as i32))
}

/// Selects the unit under a map click, or clears the selection when clicking empty ground.
fn select_units(
    mut clicks: MessageReader<MapClicked>,
    mut selection: ResMut<Selection>,
    units: Query<(&UnitSprite, &Transform)>,
) {
    for MapClicked(at) in clicks.read() {
        // Pick the closest unit whose (interpolated) sprite is within roughly half a tile.
        selection.0 = units
            .iter()
            .map(|(UnitSprite(id), tf)| (*id, tf.translation.truncate().distance(*at)))
            .filter(|(_, d)| *d <= TILE * 0.75)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id);
    }
}

/// When a unit becomes selected, moves the camera so the unit is centred in the part of the
/// screen the panels leave free: left of the side panel and above the selection bar. Later
/// panning is the player's; this runs only when the selection changes.
fn center_on_selection(
    sim: Res<Sim>,
    selection: Res<Selection>,
    mut centred: Local<Option<UnitId>>,
    mut camera: Query<(&mut Transform, &Projection, &Camera), With<Camera2d>>,
) {
    let Some(unit) = selection.0.and_then(|id| sim.world.unit(id)) else {
        *centred = None;
        return;
    };
    if *centred == Some(unit.id) {
        return;
    }
    let Ok((mut tf, Projection::Orthographic(ortho), cam)) = camera.single_mut() else { return };
    if cam.logical_viewport_size().is_none() {
        return;
    }
    let (side, bar) = hud::covered_by_panels();
    let at = tile_translation(unit.pos, 0.0).truncate();
    // The free area's centre is `side / 2` left and `bar / 2` above the view's centre.
    let center = at + Vec2::new(side / 2.0, -bar / 2.0) * ortho.scale;
    tf.translation = center.extend(tf.translation.z);
    *centred = Some(unit.id);
}

fn draw_selection(
    selection: Res<Selection>,
    units: Query<(&UnitSprite, &Transform)>,
    mut gizmos: Gizmos,
) {
    let Some(selected) = selection.0 else { return };
    if let Some((_, tf)) = units.iter().find(|(UnitSprite(id), _)| *id == selected) {
        let feet = tf.translation.truncate() + Vec2::Y * UNIT_FEET;
        gizmos.ellipse_2d(
            Isometry2d::from_translation(feet),
            Vec2::new(6.0, 3.0),
            Color::srgb(1.0, 0.85, 0.2),
        );
    }
}

/// Frames to render after the art has loaded before capturing, so sprites and text are settled.
const SETTLE_FRAMES: u32 = 10;
/// Give up if the art hasn't loaded and a frame been captured by then.
const HEADLESS_TIMEOUT_SECS: f32 = 60.0;

/// In headless mode: once the art has loaded, captures the camera's target image to
/// `Headless::path` and exits.
fn headless_screenshot(
    mut commands: Commands,
    headless: Res<Headless>,
    assets: Res<AssetServer>,
    art: Option<Res<Art>>,
    ui_art: Res<ui_style::UiArt>,
    camera: Query<&RenderTarget, With<Camera2d>>,
    time: Res<Time<Real>>,
    mut settled: Local<u32>,
    mut requested: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) {
    if time.elapsed_secs() > HEADLESS_TIMEOUT_SECS {
        error!("headless screenshot timed out after {HEADLESS_TIMEOUT_SECS}s");
        exit.write(AppExit::error());
        return;
    }
    let Some(art) = art else { return };
    let ui_loaded = ui_art.handles().all(|h| assets.is_loaded_with_dependencies(h));
    if *requested || !art.all_loaded() || !ui_loaded {
        return;
    }
    *settled += 1;
    if *settled < SETTLE_FRAMES {
        return;
    }
    let Ok(RenderTarget::Image(target)) = camera.single() else {
        panic!("headless camera must render to an image");
    };
    commands
        .spawn(Screenshot::image(target.handle.clone()))
        .observe(save_to_disk(headless.path.clone()))
        .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            exit.write(AppExit::Success);
        });
    *requested = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Vec2 = Vec2::new(0.0, 0.0);
    const MAX: Vec2 = Vec2::new(100.0, 50.0);

    #[test]
    fn parses_screenshot_size() {
        assert_eq!(parse_size("1280x720"), Some(UVec2::new(1280, 720)));
        assert_eq!(parse_size("0x720"), None);
        assert_eq!(parse_size("1280"), None);
    }

    #[test]
    fn camera_inside_bounds_is_unchanged() {
        let c = Vec2::new(50.0, 25.0);
        assert_eq!(clamp_camera(c, Vec2::new(10.0, 10.0), MIN, MAX), c);
    }

    #[test]
    fn camera_is_pushed_back_from_each_edge() {
        let half = Vec2::new(10.0, 5.0);
        assert_eq!(clamp_camera(Vec2::new(-30.0, -30.0), half, MIN, MAX), Vec2::new(10.0, 5.0));
        assert_eq!(clamp_camera(Vec2::new(500.0, 500.0), half, MIN, MAX), Vec2::new(90.0, 45.0));
    }

    #[test]
    fn view_larger_than_map_centers_that_axis() {
        // Wider than the map horizontally only.
        let c = clamp_camera(Vec2::new(0.0, 0.0), Vec2::new(80.0, 5.0), MIN, MAX);
        assert_eq!(c, Vec2::new(50.0, 5.0));
    }
}
