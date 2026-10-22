//! Bevy client: draws the `fn_sim` world and forwards player input. Owns no game rules.
//!
//!     cargo run -p fn_client -- [seed] [air|water|earth|fire]
//!
//! Controls: WASD/arrows pan, mouse wheel zoom, R new map, 1-4 switch nation, Space pause.
//!
//! `FN_SCREENSHOT=out.png` renders a few seconds, saves a screenshot and exits. Agents use this
//! to check visual changes without a human looking at the window.

use std::collections::HashMap;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PrimaryWindow, WindowLevel};
use fn_sim::world::TICKS_PER_SECOND;
use fn_sim::{Defs, MapGenConfig, Nation, Pos, Terrain, Tile, World};

const TILE: f32 = 16.0;
const MAP_WIDTH: i32 = 96;
const MAP_HEIGHT: i32 = 64;
const STARTING_UNITS: u32 = 6;
const ASSET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
const KENNEY: &str = "third_party/kenney-tiny-town/tilemap_packed.png";
const LEGACY: &str = "third_party/thefournations-2013";

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let nation = args.next().and_then(|s| parse_nation(&s)).unwrap_or(Nation::Earth);
    let screenshot_mode = std::env::var_os("FN_SCREENSHOT").is_some();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin { file_path: ASSET_DIR.into(), ..default() })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "The Four Nations (rust spike)".into(),
                        // Screenshot runs are for agents; don't grab the human's focus.
                        focused: !screenshot_mode,
                        window_level: if screenshot_mode {
                            WindowLevel::AlwaysOnBottom
                        } else {
                            WindowLevel::Normal
                        },
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(Time::<Fixed>::from_hz(f64::from(TICKS_PER_SECOND)))
        .insert_resource(Sim::new(seed, nation))
        .add_systems(Startup, (load_art, setup_camera_and_ui, spawn_world).chain())
        .add_systems(FixedUpdate, tick_sim)
        .add_systems(Update, (controls, camera_controls, sync_units, hover_info, hud).chain())
        .add_systems(Update, auto_screenshot.run_if(|| std::env::var_os("FN_SCREENSHOT").is_some()))
        .run();
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
    fn new(seed: u64, nation: Nation) -> Self {
        let defs = Defs::bundled();
        let cfg = MapGenConfig { seed, width: MAP_WIDTH, height: MAP_HEIGHT, ..default() };
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

fn setup_camera_and_ui(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Transform::from_xyz(MAP_WIDTH as f32 * TILE / 2.0, -MAP_HEIGHT as f32 * TILE / 2.0, 0.0),
    ));
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
    commands.spawn((Text::new(""), style.clone(), panel(true), HoverText));
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
        (Terrain::Water, _) => art.legacy_terrain("terrain/water"),
        (Terrain::Coast, "tundra") => art.legacy_terrain("terrain/snow-coast"),
        (Terrain::Coast, _) => art.legacy_terrain("terrain/coast"),
        (Terrain::Mountain, "tundra") => art.legacy_terrain("terrain/snow-stones"),
        (Terrain::Mountain, _) => art.legacy_terrain("terrain/stones"),
        (Terrain::Land, "desert") => art.legacy_terrain("terrain/desert"),
        (Terrain::Land, "tundra") => art.legacy_terrain("terrain/snow"),
        (Terrain::Land, "forest") => {
            let mut s = art.kenney(if vary == 0 { 1 } else { 0 });
            s.color = Color::srgb(0.75, 0.9, 0.75);
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
            Transform::from_translation(tile_translation(u.pos, 5.0) + Vec3::Y * 4.0)
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
    art: Res<Art>,
    existing: Query<Entity, With<MapEntity>>,
) {
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
    *sim = Sim::new(seed, nation.unwrap_or(sim.world.nation));
    for e in &existing {
        commands.entity(e).despawn();
    }
    spawn_entities(&mut commands, &art, &sim);
}

fn camera_controls(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut tf, mut projection)) = camera.single_mut() else { return };
    let Projection::Orthographic(ortho) = &mut *projection else { return };

    if scroll.delta.y != 0.0 {
        ortho.scale = (ortho.scale * (1.0 - scroll.delta.y.signum() * 0.1)).clamp(0.2, 4.0);
    }
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
}

/// Smoothly moves unit sprites toward their sim positions.
fn sync_units(sim: Res<Sim>, time: Res<Time>, mut units: Query<(&UnitSprite, &mut Transform)>) {
    let t = 1.0 - (-time.delta_secs() * 12.0).exp();
    for (UnitSprite(i), mut tf) in &mut units {
        let Some(u) = sim.world.units.get(*i) else { continue };
        let target = tile_translation(u.pos, 5.0 + u.pos.y as f32 * 0.001) + Vec3::Y * 4.0;
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
    mut text: Query<&mut Text, With<HoverText>>,
) {
    let (Ok(window), Ok((camera, camera_tf)), Ok(mut text)) =
        (window.single(), camera.single(), text.single_mut())
    else {
        return;
    };
    let Some(p) = cursor_tile(window, camera, camera_tf) else { return };
    let Some(tile) = sim.world.map.get(p) else {
        text.0 = String::new();
        return;
    };
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
        "{} | seed {} | tick {}{}\nmove x{} carry x{} earthworks x{} heat x{} water x{} power x{} build x{}\n[WASD] pan  [wheel] zoom  [R] new map  [1-4] nation  [Space] pause",
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

/// Saves a screenshot to `$FN_SCREENSHOT` after the world has rendered for a while, then exits.
fn auto_screenshot(
    mut commands: Commands,
    time: Res<Time>,
    mut taken_at: Local<Option<f32>>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    match *taken_at {
        None if now > 3.0 => {
            let path = std::env::var("FN_SCREENSHOT").expect("checked by run_if");
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            *taken_at = Some(now);
        }
        Some(t) if now > t + 1.0 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
