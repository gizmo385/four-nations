//! Bevy client: draws the `fn_sim` world and forwards player input. Owns no game rules.
//!
//!     cargo run -p fn_client -- [seed] [air|water|earth|fire] [width] [height]
//!
//! Controls: click a unit to select it (Esc deselects), WASD/arrows or mouse drag pan, mouse wheel
//! zoom, R new map, 1-4 switch nation, Space pause.
//!
//! `FN_SCREENSHOT=out.png` runs headless: no window is created at all (winit is disabled and the
//! camera renders into an offscreen `Image`), and once the art has loaded it saves one frame as a
//! PNG and exits. `FN_SCREENSHOT_SIZE=WIDTHxHEIGHT` sets the image size (default 1280x720). Agents
//! use this, via `cargo xtask screenshot SEED NATION OUT.png [WIDTH HEIGHT]`, to check visual
//! changes without a window popping up and stealing the human's focus.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::camera::RenderTarget;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::window::{ExitCondition, PrimaryWindow};
use bevy::winit::WinitPlugin;
use fn_sim::world::TICKS_PER_SECOND;
use fn_sim::{Defs, MapGenConfig, Nation, Pos, Terrain, Tile, World};

const TILE: f32 = 16.0;
const STARTING_UNITS: u32 = 6;
const ASSET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
const KENNEY: &str = "third_party/kenney-tiny-town/tilemap_packed.png";
const LEGACY: &str = "third_party/thefournations-2013";

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
    app.insert_resource(Time::<Fixed>::from_hz(f64::from(TICKS_PER_SECOND)))
        .insert_resource(Sim::new(seed, nation, width, height))
        .init_resource::<Selection>()
        .add_message::<MapClicked>()
        .add_systems(Startup, (load_art, setup_camera_and_ui, spawn_world).chain())
        .add_systems(FixedUpdate, tick_sim)
        .add_systems(
            Update,
            (
                controls,
                camera_controls,
                sync_units,
                select_units,
                draw_selection,
                selection_info,
                hover_info,
                hud,
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

#[derive(Resource)]
struct Sim {
    world: World,
    defs: Defs,
    paused: bool,
}

impl Sim {
    fn new(seed: u64, nation: Nation, width: i32, height: i32) -> Self {
        let defs = Defs::bundled();
        let cfg = MapGenConfig { seed, width, height, ..default() };
        let world = World::new(&cfg, nation, STARTING_UNITS, &defs);
        Self { world, defs, paused: false }
    }
}

#[derive(Resource)]
struct Art {
    kenney: Handle<Image>,
    kenney_layout: Handle<TextureAtlasLayout>,
    legacy: HashMap<String, Handle<Image>>,
}

impl Art {
    fn legacy(&self, name: &str) -> Handle<Image> {
        self.legacy.get(name).unwrap_or_else(|| panic!("legacy sprite {name} not loaded")).clone()
    }

    /// A 16px 2013 terrain tile with its baked-in 1px frame cropped off, so tiles join seamlessly.
    fn legacy_terrain(&self, name: &str) -> Sprite {
        Sprite {
            image: self.legacy(name),
            rect: Some(Rect::new(1.0, 1.0, 15.0, 15.0)),
            custom_size: Some(Vec2::splat(TILE)),
            ..default()
        }
    }

    fn kenney(&self, index: usize) -> Sprite {
        Sprite::from_atlas_image(
            self.kenney.clone(),
            TextureAtlas { layout: self.kenney_layout.clone(), index },
        )
    }
}

/// Everything that belongs to the current map; despawned on regenerate.
#[derive(Component)]
struct MapEntity;

#[derive(Component)]
struct UnitSprite(usize);

#[derive(Component)]
struct HoverText;

#[derive(Component)]
struct SelectionText;

/// Index into `World::units` of the selected unit, if any.
#[derive(Resource, Default)]
struct Selection(Option<usize>);

/// A left click on the map that wasn't part of a drag, in world coordinates.
#[derive(Message)]
struct MapClicked(Vec2);

#[derive(Component)]
struct HudText;

fn load_art(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let mut legacy = HashMap::new();
    let mut names: Vec<String> = [
        "terrain/water",
        "terrain/coast",
        "terrain/desert",
        "terrain/snow",
        "terrain/snow-coast",
        "terrain/stones",
        "terrain/snow-stones",
        "terrain/earth-stones",
        "terrain/snow-tree",
        "resources/gold",
        "resources/cabbage1",
    ]
    .map(String::from)
    .to_vec();
    for n in Nation::ALL {
        names.extend((1..=3).map(|i| format!("units/{}-{i}", n.as_str())));
    }
    for name in names {
        let handle = assets.load(format!("{LEGACY}/{name}.png"));
        legacy.insert(name, handle);
    }
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(16), 12, 11, None, None);
    commands.insert_resource(Art {
        kenney: assets.load(KENNEY),
        kenney_layout: layouts.add(layout),
        legacy,
    });
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
    let panel = |top: bool| Node {
        position_type: PositionType::Absolute,
        left: Val::Px(8.0),
        top: if top { Val::Px(8.0) } else { Val::Auto },
        bottom: if top { Val::Auto } else { Val::Px(8.0) },
        padding: UiRect::all(Val::Px(6.0)),
        ..default()
    };
    let style = (
        TextFont { font_size: 14.0.into(), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
    );
    commands.spawn((Text::new(""), style.clone(), panel(true), Visibility::Hidden, HoverText));
    commands.spawn((
        Text::new(""),
        style.clone(),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(8.0),
            top: Val::Px(8.0),
            padding: UiRect::all(Val::Px(6.0)),
            ..default()
        },
        Visibility::Hidden,
        SelectionText,
    ));
    commands.spawn((Text::new(""), style, panel(false), HudText));
}

fn tile_translation(p: Pos, z: f32) -> Vec3 {
    Vec3::new(p.x as f32 * TILE, -p.y as f32 * TILE, z)
}

/// Picks a sprite for a tile's ground layer.
fn ground_sprite(art: &Art, sim: &Sim, p: Pos, t: &Tile) -> Sprite {
    let biome = sim.defs.biome(t.biome).id.as_str();
    // Cheap per-position variation so grass doesn't look tiled.
    let vary = ((p.x * 7 + p.y * 13) % 5) as usize;
    match (t.terrain, biome) {
        // Rivers and lakes reuse the sea tile until they get their own art.
        (Terrain::Water | Terrain::River | Terrain::Lake, _) => art.legacy_terrain("terrain/water"),
        (Terrain::Coast, "tundra") => art.legacy_terrain("terrain/snow-coast"),
        (Terrain::Coast, _) => art.legacy_terrain("terrain/coast"),
        (Terrain::Mountain, "tundra") => art.legacy_terrain("terrain/snow-stones"),
        (Terrain::Mountain, _) => art.legacy_terrain("terrain/stones"),
        (Terrain::Land, "desert") => art.legacy_terrain("terrain/desert"),
        (Terrain::Land, "tundra") => art.legacy_terrain("terrain/snow"),
        // Wooded biomes share the grass sprite, told apart by tint.
        (Terrain::Land, "forest" | "taiga" | "swamp") => {
            let mut s = art.kenney(if vary == 0 { 1 } else { 0 });
            s.color = match biome {
                "taiga" => Color::srgb(0.75, 0.85, 0.85),
                "swamp" => Color::srgb(0.6, 0.7, 0.5),
                _ => Color::srgb(0.75, 0.9, 0.75),
            };
            s
        }
        (Terrain::Land, _) => art.kenney([0, 0, 0, 1, 2][vary]),
    }
}

/// Picks a sprite (and vertical offset, for tall sprites) for a tile's resource, if any.
fn resource_sprite(art: &Art, sim: &Sim, t: &Tile) -> Option<(Sprite, f32)> {
    let res = sim.defs.resource(t.resource?).id.as_str();
    let biome = sim.defs.biome(t.biome).id.as_str();
    Some(match (res, biome) {
        ("fir_tree", "tundra") => (Sprite::from_image(art.legacy("terrain/snow-tree")), TILE / 2.0),
        ("fir_tree", _) => (art.kenney(28), 0.0),
        ("oak_tree", _) => (art.kenney(5), 0.0),
        ("gold_vein", _) => (Sprite::from_image(art.legacy("resources/gold")), 0.0),
        ("stone", _) => (Sprite::from_image(art.legacy("terrain/earth-stones")), 0.0),
        ("cabbage", _) => (Sprite::from_image(art.legacy("resources/cabbage1")), 0.0),
        _ => (art.kenney(29), 0.0),
    })
}

fn spawn_world(mut commands: Commands, art: Res<Art>, sim: Res<Sim>) {
    spawn_entities(&mut commands, &art, &sim);
}

fn spawn_entities(commands: &mut Commands, art: &Art, sim: &Sim) {
    for (p, t) in sim.world.map.iter() {
        commands.spawn((
            ground_sprite(art, sim, p, t),
            Transform::from_translation(tile_translation(p, 0.0)),
            MapEntity,
        ));
        if let Some((sprite, dy)) = resource_sprite(art, sim, t) {
            // Lower rows draw on top so tall sprites overlap correctly.
            let z = 1.0 + p.y as f32 * 0.001;
            let mut tf = Transform::from_translation(tile_translation(p, z));
            tf.translation.y += dy;
            commands.spawn((sprite, tf, MapEntity));
        }
    }
    for (i, u) in sim.world.units.iter().enumerate() {
        let name = format!("units/{}-{}", u.nation.as_str(), u.variant + 1);
        commands.spawn((
            Sprite::from_image(art.legacy(&name)),
            Transform::from_translation(tile_translation(u.pos, 5.0) + Vec3::Y * UNIT_Y_OFFSET)
                .with_scale(Vec3::splat(0.6)),
            UnitSprite(i),
            MapEntity,
        ));
    }
}

fn tick_sim(mut sim: ResMut<Sim>) {
    if sim.paused {
        return;
    }
    let Sim { world, defs, .. } = &mut *sim;
    world.tick(defs);
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
        sim.paused = !sim.paused;
    }
    let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    let nation = digits.iter().position(|k| keys.just_pressed(*k)).map(|i| Nation::ALL[i]);
    let new_seed = keys.just_pressed(KeyCode::KeyR).then(|| sim.world.seed.wrapping_add(1));
    if nation.is_none() && new_seed.is_none() {
        return;
    }
    let seed = new_seed.unwrap_or(sim.world.seed);
    let (width, height) = (sim.world.map.width(), sim.world.map.height());
    *sim = Sim::new(seed, nation.unwrap_or(sim.world.nation), width, height);
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

    // Grab-and-drag panning with any mouse button. Screen y points down, world y points up.
    let buttons = [MouseButton::Left, MouseButton::Middle, MouseButton::Right];
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
    if mouse.just_released(MouseButton::Left)
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
    for (UnitSprite(i), mut tf) in &mut units {
        let Some(u) = sim.world.units.get(*i) else { continue };
        let target =
            tile_translation(u.pos, 5.0 + u.pos.y as f32 * 0.001) + Vec3::Y * UNIT_Y_OFFSET;
        tf.translation = tf.translation.lerp(target, t);
    }
}

fn cursor_tile(window: &Window, camera: &Camera, camera_tf: &GlobalTransform) -> Option<Pos> {
    let cursor = window.cursor_position()?;
    let world = camera.viewport_to_world_2d(camera_tf, cursor).ok()?;
    Some(Pos::new((world.x / TILE).round() as i32, (-world.y / TILE).round() as i32))
}

fn hover_info(
    sim: Res<Sim>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut text: Query<(&mut Text, &mut Visibility), With<HoverText>>,
) {
    let (Ok(window), Ok((camera, camera_tf)), Ok((mut text, mut visibility))) =
        (window.single(), camera.single(), text.single_mut())
    else {
        return;
    };
    let tile =
        cursor_tile(window, camera, camera_tf).and_then(|p| Some((p, sim.world.map.get(p)?)));
    let Some((p, tile)) = tile else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    let resource = tile.resource.map_or("None", |r| sim.defs.resource(r).name.as_str());
    text.0 = format!(
        "Tile ({}, {})\nTerrain: {:?}\nBiome: {}\nElevation: {}\nResource: {}",
        p.x,
        p.y,
        tile.terrain,
        sim.defs.biome(tile.biome).name,
        tile.elevation,
        resource
    );
}

fn hud(sim: Res<Sim>, mut text: Query<&mut Text, With<HudText>>) {
    let Ok(mut text) = text.single_mut() else { return };
    let def = sim.defs.nation(sim.world.nation);
    let m = &def.modifiers;
    text.0 = format!(
        "{} | seed {} | tick {}{}\nmove x{} carry x{} earthworks x{} heat x{} water x{} power x{} build x{}\n[click] select  [WASD/drag] pan  [wheel] zoom  [R] new map  [1-4] nation  [Space] pause",
        def.name,
        sim.world.seed,
        sim.world.tick,
        if sim.paused { " (paused)" } else { "" },
        m.move_speed,
        m.carry_capacity,
        m.earthworks,
        m.heating,
        m.water_work,
        m.power_generation,
        m.construction,
    );
}

/// Unit sprites are drawn this far above their tile center (they're taller than a tile).
const UNIT_Y_OFFSET: f32 = 4.0;

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
            .map(|(UnitSprite(i), tf)| (*i, tf.translation.truncate().distance(*at)))
            .filter(|(_, d)| *d <= TILE * 0.75)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
    }
}

fn draw_selection(
    selection: Res<Selection>,
    units: Query<(&UnitSprite, &Transform)>,
    mut gizmos: Gizmos,
) {
    let Some(selected) = selection.0 else { return };
    if let Some((_, tf)) = units.iter().find(|(UnitSprite(i), _)| *i == selected) {
        let feet = tf.translation.truncate() - Vec2::Y * UNIT_Y_OFFSET;
        gizmos.ellipse_2d(
            Isometry2d::from_translation(feet),
            Vec2::new(8.0, 4.0),
            Color::srgb(1.0, 0.85, 0.2),
        );
    }
}

fn selection_info(
    sim: Res<Sim>,
    selection: Res<Selection>,
    mut text: Query<(&mut Text, &mut Visibility), With<SelectionText>>,
) {
    let Ok((mut text, mut visibility)) = text.single_mut() else { return };
    let Some(unit) = selection.0.and_then(|i| sim.world.units.get(i)) else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    text.0 = format!(
        "{}\n{}\nTile ({}, {})\n[Esc] deselect",
        unit.name,
        sim.defs.nation(unit.nation).name,
        unit.pos.x,
        unit.pos.y
    );
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
    let loaded = std::iter::once(&art.kenney)
        .chain(art.legacy.values())
        .all(|h| assets.is_loaded_with_dependencies(h));
    if *requested || !loaded {
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
