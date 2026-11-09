//! `balance trace` (`docs/design/balance-harness.md`, section 3; #535): reruns one run and prints
//! what a flagged run looked like from inside. Per minute over a window, one row of the probe's
//! numbers (population, idle settlers by reason, stores, each settler's lowest need, the flags
//! raised so far), and at a chosen minute an ASCII crop of the live map around the anchor with
//! the settlers, buildings, stockpiles and resources that block walking marked. #487 took an
//! hour of throwaway programs to see settlers boxed into a pocket; this is that picture in one
//! command, and `balance verdict` prints the command for every flagged run.
//!
//! The run is `fn_sim::balance::run_observed`: the same run `balance verdict` plays for the same
//! seed, nation, style and preset (the detectors are `data/harness.toml`'s), watched by a closure
//! that only reads. Nothing is printed until the run reaches the last minute asked for.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::ExitCode;

use fn_sim::balance::{Minute, Occupation, PEOPLE, RunSpec, run_observed};
use fn_sim::defs::ResourceDef;
use fn_sim::geom::{Footprint, Pos};
use fn_sim::jobs::ItemId;
use fn_sim::world::UnitId;
use fn_sim::{Defs, Nation, World, mapgen};

use crate::config::Config;

/// The style and preset a run has when the command names none: the reference run, as
/// `report::trace_command` leaves them out for.
const DEFAULT_STYLE: &str = "steady";
const DEFAULT_PRESET: &str = "normal";

/// Tiles from the anchor to the edge of the crop when `--radius` isn't given.
const DEFAULT_RADIUS: i32 = 20;

/// What `balance trace` was asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceArgs {
    pub seed: u64,
    pub nation: Nation,
    pub style: String,
    pub preset: String,
    pub from: u32,
    pub to: u32,
    pub map_at: Option<u32>,
    pub radius: i32,
    pub help: bool,
}

pub fn help() -> &'static str {
    "usage: balance trace --seed S --nation N [--style ID] [--preset P] --from M --to M\n\
     \x20                    [--map-at M] [--radius R]\n\n\
     Reruns one run (the same one `balance verdict` played for that seed, nation, style and\n\
     preset) and prints its per-minute probe rows from minute M to minute M: population, idle\n\
     settlers by reason, stores, each settler's lowest need, and the flags raised so far.\n\
     --map-at also prints an ASCII crop of the live map around the anchor at that minute, with\n\
     the settlers, buildings, stockpiles and blocking resources marked, and a legend.\n\n\
     --style     a play style from data/harness.toml (default steady)\n\
     --preset    a difficulty preset (default normal)\n\
     --radius    tiles from the anchor to the crop's edge (default 20)\n"
}

pub fn parse_args(args: &[String]) -> Result<TraceArgs, String> {
    let (mut seed, mut nation, mut from, mut to) = (None, None, None, None);
    let mut out = TraceArgs {
        seed: 0,
        nation: Nation::Water,
        style: DEFAULT_STYLE.to_owned(),
        preset: DEFAULT_PRESET.to_owned(),
        from: 0,
        to: 0,
        map_at: None,
        radius: DEFAULT_RADIUS,
        help: false,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{flag} needs a value"));
        let whole = |v: String, what: &str| {
            v.parse::<u32>().map_err(|_| format!("{what} needs a whole number of minutes"))
        };
        match flag.as_str() {
            "--seed" => {
                seed = Some(
                    value()?
                        .parse::<u64>()
                        .map_err(|_| "--seed needs a whole number".to_owned())?,
                )
            }
            "--nation" => {
                let id = value()?;
                nation = Some(
                    Nation::ALL
                        .iter()
                        .copied()
                        .find(|n| n.as_str() == id)
                        .ok_or_else(|| format!("unknown nation {id:?}"))?,
                );
            }
            "--style" => out.style = value()?,
            "--preset" => out.preset = value()?,
            "--from" => from = Some(whole(value()?, "--from")?),
            "--to" => to = Some(whole(value()?, "--to")?),
            "--map-at" => out.map_at = Some(whole(value()?, "--map-at")?),
            "--radius" => {
                out.radius = value()?
                    .parse::<i32>()
                    .ok()
                    .filter(|r| (1..=200).contains(r))
                    .ok_or_else(|| "--radius needs a whole number from 1 to 200".to_owned())?
            }
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if out.help {
        return Ok(out);
    }
    out.seed = seed.ok_or("--seed is required")?;
    out.nation = nation.ok_or("--nation is required")?;
    out.from = from.ok_or("--from is required")?;
    out.to = to.ok_or("--to is required")?;
    if out.from > out.to {
        return Err(format!("--from {} is after --to {}", out.from, out.to));
    }
    Ok(out)
}

/// `balance trace`: prints the trace, or the error and the usage.
pub fn cmd(args: &[String]) -> ExitCode {
    match run(args) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}\n\n{}", help());
            ExitCode::from(2)
        }
    }
}

/// The text `balance trace` prints for `args`.
pub fn run(args: &[String]) -> Result<String, String> {
    let a = parse_args(args)?;
    if a.help {
        return Ok(help().to_owned());
    }
    let defs = Defs::bundled();
    let config = Config::bundled(&defs);
    let style = config.styles.get(&a.style).ok_or_else(|| {
        let known: Vec<_> = config.styles.iter().map(|s| s.id.clone()).collect();
        format!("unknown style {:?} (known: {})", a.style, known.join(", "))
    })?;
    if !defs.difficulty.presets.iter().any(|p| p.id == a.preset) {
        let known: Vec<_> = defs.difficulty.presets.iter().map(|p| p.id.clone()).collect();
        return Err(format!("unknown preset {:?} (known: {})", a.preset, known.join(", ")));
    }

    // The run stops half a minute after the last minute wanted: its probe fires at the whole
    // minute, and nothing later is read.
    let last = a.to.max(a.map_at.unwrap_or(0));
    let mut spec = RunSpec::new(a.seed, a.nation, last as f32 + 0.5, &defs);
    spec.preset = Some(a.preset.clone());
    spec.player = style.player(PEOPLE);
    spec.detectors = config.detectors;

    let mut trace = Trace::new(&a);
    let record = run_observed(&spec, &defs, &mut |m| trace.observe(m, &defs));
    let mut out = trace.finish();

    let deaths: Vec<_> = record
        .autopsies
        .iter()
        .filter(|d| (a.from as f32..=a.to as f32 + 1.0).contains(&d.minute))
        .collect();
    if !deaths.is_empty() {
        out.push_str("\ndeaths in the window\n");
        for d in deaths {
            let _ = writeln!(out, "  minute {:.0}  {}: {}", d.minute, d.name, d.summary());
        }
    }
    let flags: Vec<_> = record.flags.iter().filter(|r| r.minute <= a.to as f32).collect();
    if !flags.is_empty() {
        out.push_str("\nflags raised by minute ");
        let _ = writeln!(out, "{}", a.to);
        for r in flags {
            let _ = writeln!(
                out,
                "  {} raised at minute {:.0}, condition since {:.0}",
                r.flag.name(),
                r.minute,
                r.since
            );
        }
    }
    Ok(out)
}

/// A trace being collected: the rows as the run goes, and the crop when its minute comes.
struct Trace {
    from: u32,
    to: u32,
    map_at: Option<u32>,
    radius: i32,
    title: String,
    rows: Vec<String>,
    crop: Option<String>,
    /// Each settler's mark, fixed at the minute it is first seen so a row and the map agree.
    marks: BTreeMap<UnitId, char>,
}

impl Trace {
    fn new(a: &TraceArgs) -> Self {
        Self {
            from: a.from,
            to: a.to,
            map_at: a.map_at,
            radius: a.radius,
            title: format!(
                "balance trace: seed {} {} style {} preset {}, minutes {}-{}",
                a.seed,
                a.nation.as_str(),
                a.style,
                a.preset,
                a.from,
                a.to
            ),
            rows: Vec::new(),
            crop: None,
            marks: BTreeMap::new(),
        }
    }

    fn observe(&mut self, m: Minute<'_>, defs: &Defs) {
        let minute = m.sample.minute;
        let marks = &mut self.marks;
        for u in m.world.units.iter().filter(|u| !u.is_avatar()) {
            let next = settler_mark(marks.len());
            marks.entry(u.id).or_insert(next);
        }
        if (self.from..=self.to).contains(&minute) {
            self.rows.push(row(&m, &self.marks, defs));
        }
        if self.map_at == Some(minute) {
            self.crop = Some(crop(m.world, defs, &self.marks, self.radius));
        }
    }

    fn finish(self) -> String {
        let mut out = format!("{}\n\n", self.title);
        out.push_str(&row_header());
        for r in &self.rows {
            out.push_str(r);
            out.push('\n');
        }
        if self.rows.is_empty() {
            out.push_str("(the run ended before this window)\n");
        }
        if let Some(c) = &self.crop {
            let _ = write!(out, "\nmap at minute {}\n\n{c}", self.map_at.unwrap_or(0));
        } else if let Some(at) = self.map_at {
            let _ = writeln!(out, "\n(no map at minute {at}: the run ended first)");
        }
        out
    }
}

/// A settler's mark on the map and in the rows: 1 to 9, then letters that no marker or resource
/// uses.
fn settler_mark(index: usize) -> char {
    const LETTERS: &str = "ACDEGHIJKLMNOQRSUVWXZ";
    match index {
        0..=8 => char::from(b'1' + index as u8),
        _ => LETTERS.chars().nth(index - 9).unwrap_or('Z'),
    }
}

fn row_header() -> String {
    format!(
        "{:>4} {:>3} {:>4}  {:<REASONS$}  {:<STORES$}  {}\n",
        "min", "pop", "idle", "idle by reason", "stores", "lowest need (hun thi ene hea, %)"
    )
}

/// Column widths of the reasons and stores.
const REASONS: usize = 26;
const STORES: usize = 29;

/// One minute's row.
fn row(m: &Minute<'_>, marks: &BTreeMap<UnitId, char>, defs: &Defs) -> String {
    let world = m.world;
    let mut reasons = [0u32; 7];
    let mut needs = Vec::new();
    for u in world.units.iter().filter(|u| !u.is_avatar()) {
        let occ = Occupation::read(world, u.id, defs).unwrap_or(Occupation::Idle);
        if let Some(slot) = reasons.get_mut(reason_slot(occ)) {
            *slot += 1;
        }
        let (name, pct) = lowest_need(u, defs);
        needs.push(format!("{}:{name}{pct}", marks.get(&u.id).copied().unwrap_or('?')));
    }
    let idle_by_reason: Vec<String> = reasons
        .iter()
        .zip(REASON_NAMES)
        .skip(1)
        .filter(|(n, _)| **n > 0)
        .map(|(n, name)| format!("{name} {n}"))
        .collect();
    let mut stores = Vec::new();
    for i in 0..defs.items.len() {
        let item = ItemId(i as u16);
        let n = world.stored_total(item);
        if n > 0 {
            stores.push(format!("{} {n}", defs.item(item).id));
        }
    }
    let mut flags: Vec<String> =
        m.watch.raised().iter().map(|r| r.flag.name().to_owned()).collect();
    if m.sample.longest_stranded > 0 {
        flags.push(format!("stranded-run:{}", m.sample.longest_stranded));
    }
    if m.sample.longest_stuck > 0 {
        flags.push(format!("stuck-run:{}", m.sample.longest_stuck));
    }
    format!(
        "{:>4} {:>3} {:>4}  {:<REASONS$}  {:<STORES$}  {}{}",
        m.sample.minute,
        m.sample.population,
        m.sample.idle,
        or_dash(idle_by_reason.join(", ")),
        or_dash(stores.join(" ")),
        needs.join(" "),
        if flags.is_empty() { String::new() } else { format!("  [{}]", flags.join(" ")) },
    )
}

fn or_dash(s: String) -> String {
    if s.is_empty() { "-".to_owned() } else { s }
}

/// The idle reasons in the order a row lists them; slot 0 (busy) is never printed.
const REASON_NAMES: [&str; 7] =
    ["busy", "no work", "unreachable", "out of chi", "stockpile full", "too hurt", "unexplained"];

fn reason_slot(o: Occupation) -> usize {
    match o {
        Occupation::Busy => 0,
        Occupation::NoWork => 1,
        Occupation::Unreachable => 2,
        Occupation::OutOfChi => 3,
        Occupation::StockpileFull => 4,
        Occupation::TooHurt => 5,
        Occupation::Idle => 6,
    }
}

/// The need a settler has least of, as a three-letter name and a whole percent of its full level.
fn lowest_need(u: &fn_sim::world::Unit, defs: &Defs) -> (&'static str, u32) {
    let full = defs.needs.max();
    let health_full = defs.needs.health.max;
    let levels = [
        ("hun", u.needs.hunger / full),
        ("thi", u.needs.thirst / full),
        ("ene", u.needs.energy / full),
        ("hea", u.needs.health / health_full),
    ];
    let (name, level) =
        levels.into_iter().min_by(|a, b| a.1.total_cmp(&b.1)).expect("there are four needs");
    (name, (level * 100.0).round().clamp(0.0, 100.0) as u32)
}

/// Marks on top of the terrain, in the order they win a tile.
const AVATAR: char = '@';
const STOCKPILE: char = 'P';
const BLOCKING_BUILDING: char = '#';
const WALKABLE_BUILDING: char = '|';

/// The crop of `world` within `radius` tiles of the settlement's anchor (clamped to the map, and
/// slid inward at an edge so it keeps its size when the map is big enough), drawn with
/// `mapgen::tile_glyph` and marked, with a legend. Resources are the walls of a pocket: they
/// block walking, so they stay on the map with their own glyphs, and the legend names each.
fn crop(world: &World, defs: &Defs, marks: &BTreeMap<UnitId, char>, radius: i32) -> String {
    let anchor = world.settlement_anchor();
    let (w, h) = (world.map.width(), world.map.height());
    let span = 2 * radius + 1;
    let x0 = (anchor.x - radius).clamp(0, (w - span).max(0));
    let y0 = (anchor.y - radius).clamp(0, (h - span).max(0));
    let (x1, y1) = ((x0 + span).min(w), (y0 + span).min(h));

    let mut grid: Vec<Vec<char>> = (y0..y1)
        .map(|y| (x0..x1).map(|x| mapgen::tile_glyph(&world.map[Pos::new(x, y)], defs)).collect())
        .collect();
    let mut put = |p: Pos, c: char| {
        if (x0..x1).contains(&p.x) && (y0..y1).contains(&p.y) {
            grid[(p.y - y0) as usize][(p.x - x0) as usize] = c;
        }
    };

    let mut building_counts: BTreeMap<(bool, String), u32> = BTreeMap::new();
    for (foot, id) in world.buildings() {
        let def = defs.building(id);
        let glyph = if def.passable { WALKABLE_BUILDING } else { BLOCKING_BUILDING };
        for t in Footprint::tiles(foot) {
            put(t, glyph);
        }
        if foot.tiles().any(|t| (x0..x1).contains(&t.x) && (y0..y1).contains(&t.y)) {
            *building_counts.entry((def.passable, def.name.clone())).or_default() += 1;
        }
    }
    let mut stockpile_tiles = 0;
    for p in world.stockpile_zones() {
        put(p, STOCKPILE);
        stockpile_tiles += 1;
    }
    let mut settlers: Vec<(char, &fn_sim::world::Unit)> = Vec::new();
    for u in &world.units {
        if u.is_avatar() {
            put(u.pos, AVATAR);
        } else {
            let mark = marks.get(&u.id).copied().unwrap_or('?');
            put(u.pos, mark);
            settlers.push((mark, u));
        }
    }

    // The ruler: a digit every ten tiles, and the row numbers beside the rows.
    let mut out = String::new();
    let _ = writeln!(
        out,
        "x {x0}..{} y {y0}..{} around the anchor ({}, {}) on a {w}x{h} map",
        x1 - 1,
        y1 - 1,
        anchor.x,
        anchor.y
    );
    let mut ruler = String::from("    ");
    for x in x0..x1 {
        ruler.push(if x % 10 == 0 { char::from(b'0' + ((x / 10) % 10) as u8) } else { ' ' });
    }
    let _ = writeln!(out, "{}", ruler.trim_end());
    for (i, row) in grid.iter().enumerate() {
        let _ = writeln!(out, "{:>3} {}", y0 + i as i32, row.iter().collect::<String>());
    }

    out.push_str("\nlegend\n");
    let _ = writeln!(out, "  1-9, A-Z  settlers (below)    {AVATAR}  the Avatar");
    let _ = writeln!(
        out,
        "  {STOCKPILE}  stockpile ({stockpile_tiles} tiles)    {BLOCKING_BUILDING}  building that blocks walking    {WALKABLE_BUILDING}  building you can walk through"
    );
    if !building_counts.is_empty() {
        let list: Vec<String> = building_counts
            .iter()
            .map(|((passable, name), n)| {
                format!("{name} x{n} ({})", if *passable { "walkable" } else { "blocks" })
            })
            .collect();
        let _ = writeln!(out, "  buildings in view: {}", list.join(", "));
    }
    let shown: String = grid.iter().flatten().collect();
    let blockers: Vec<String> = defs
        .resources
        .iter()
        .filter(|r| r.glyph != ' ' && shown.contains(r.glyph) && !r.buried)
        .map(resource_legend)
        .collect();
    if !blockers.is_empty() {
        let _ = writeln!(
            out,
            "  resources (each blocks walking until it is harvested): {}",
            blockers.join(", ")
        );
    }
    let _ = writeln!(
        out,
        "  terrain  ~ sea  - lake  = river  + ford  . coast  ^ mountain  _ tunnel; any other\n           glyph is open ground (a biome) or a resource"
    );

    let _ = writeln!(out, "\nsettlers");
    for (mark, u) in settlers {
        let occ = Occupation::read(world, u.id, defs).unwrap_or(Occupation::Idle);
        let doing = match u.task() {
            Some(t) => format!("{t:?}").split('(').next().unwrap_or("").to_owned(),
            None => format!("idle ({occ:?})"),
        };
        let (name, pct) = lowest_need(u, defs);
        let reach = world.reach(u.id).map_or_else(String::new, |r| {
            let yn = |b: bool| if b { "yes" } else { "NO" };
            let stock = r.stockpile.map_or("none marked", yn);
            format!("; can walk to water {}, stockpile {stock}, home {}", yn(r.water), yn(r.anchor))
        });
        let _ = writeln!(
            out,
            "  {mark}  {} at ({}, {}): {doing}; lowest need {name} {pct}%{reach}",
            u.name, u.pos.x, u.pos.y
        );
    }
    out
}

fn resource_legend(r: &ResourceDef) -> String {
    format!("{} {}", r.glyph, r.name)
}

#[cfg(test)]
mod tests;
