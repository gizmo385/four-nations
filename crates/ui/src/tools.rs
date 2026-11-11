//! Designation tools: which command each tool issues for a tile, which key picks it, and the loop
//! that applies a tool to every tile of a rectangle. The frontends turn a drag (a mouse's, or two
//! `Enter`s) into a rectangle; this turns the rectangle into `Command`s and a notice.
//!
//! Keys: `H` harvest (chop, quarry, gather, mine, whatever each tile's resource gives), `X` cancel
//! (withdraws a harvest, a bend, a Sow designation or a Clear mark -- the job designations a
//! rectangle usually means to drop). Held with `O` (`Pick::teardown`, #335: a cancel over a
//! rectangle used to also sweep up every stockpile and build site it crossed, costing two
//! playtests their stores), `X` also withdraws a build site or a stockpile tile; without it
//! those are left alone. `E` clear the ground (docs/design/clearing.md, #411/#426: uproots a
//! plant or works a resource's ordinary harvest, removing it for good); `E` again switches it to
//! buildings mode, which marks finished buildings to be taken down (#342), and back. `P` stockpile, `B` the
//! nation's bending ability (pressed again it cycles through the abilities the settlement can
//! use), `U` the build tool (pressed again it cycles through the buildings the nation can
//! place), `C` sow an empty farm plot (issue #198; a ripe one is reaped on its own, with no
//! tool) and `Esc` back to selecting.

use fn_sim::bending::AbilityId;
use fn_sim::building::BuildingId;
use fn_sim::world::{Command, CommandError};
use fn_sim::{Defs, Nation, Pos};

use crate::build;
use crate::session::Session;

/// What the pointer (or the cursor's `Enter`) does on the map.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tool {
    /// Pick a unit.
    #[default]
    Select,
    Harvest,
    CancelHarvest,
    /// Remove whatever resource stands on a tile, for good (docs/design/clearing.md, #411/#426):
    /// the Clear tool's ground mode, the one `E` starts in. `X` also withdraws a Clear mark, as
    /// it does any other order.
    Clear,
    /// The Clear tool's buildings mode (#342, docs/design/clearing.md, section 5): `E` pressed
    /// again while clearing the ground. Marks each finished building a rectangle touches to be
    /// taken down, once per building. Not in `ALL`: it shares Clear's key, name and panel row,
    /// and only its banner badge (`CLEAR buildings`) tells the two apart. `X` takes the mark back.
    Deconstruct,
    Stockpile,
    /// Channel one of the nation's abilities (docs/design/bending.md): its signature one, or
    /// the next the settlement can use after `B` again (`Session::bend_choice`).
    Bend,
    /// Place the building picked in the picker (#111).
    Build,
    /// Sow an empty farm plot (issue #198). A ripe one is reaped on its own; there's no tool
    /// for that.
    Sow,
    /// Place the endeavor's gate (#643): armed from the Endeavor screen once the endeavor is
    /// Ready, never by a letter key of its own, so it isn't in [`Tool::ALL`]. One `Enter` places
    /// the site centred on the cursor (`crate::gate::place`); there is no rectangle.
    Gate,
}

/// What a tool needs besides the tile to decide on a command.
#[derive(Clone, Copy, Default, Debug)]
pub struct Pick {
    /// The ability the Bend tool channels, resolved once per drag (`bend_ability`); `Bend`
    /// issues nothing without one.
    pub ability: Option<AbilityId>,
    /// Held Shift (#81): a harvest designation becomes a standing order that re-queues itself
    /// every time the resource ripens again. No effect on any other tool.
    pub repeating: bool,
    /// The building to place; `Build` issues nothing without one.
    pub building: Option<BuildingId>,
    /// The Cancel tool's own `O` toggle (#335): without it, a rectangle cancel withdraws only
    /// job designations (a harvest, a bend, a Sow or a Clear mark) and leaves a build site or a
    /// stockpile tile alone, so cancelling harvest jobs over a stockpiled area doesn't tear the
    /// stockpile (and the goods on it) down by accident. No effect on any other tool.
    pub teardown: bool,
}

/// What the cancel tool can withdraw on a tile, other than a harvest designation.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Here {
    /// A build site is pending on the tile.
    pub site: bool,
    /// The tile is designated as stockpile.
    pub zone: bool,
    /// The tile has a Sow designation on it (#198).
    pub sowing: bool,
    /// The tile has a Clear mark on it (docs/design/clearing.md, #411/#426).
    pub clearing: bool,
    /// The tile has a bend designated on it (`World::bend_designations`).
    pub bend: bool,
    /// The finished building on the tile is marked to be taken down (#342).
    pub deconstructing: bool,
    /// The tile is a farm plot's anchor: `Some(true)` with a standing sow order on it, else
    /// `Some(false)` (docs/design/horizons-phase-1.md, `calendar`).
    pub plot: Option<bool>,
}

impl Here {
    /// What is designated on `pos` right now.
    pub fn at(session: &Session, pos: Pos) -> Self {
        let world = &session.world;
        Self {
            site: world.build_site(pos).is_some(),
            zone: world.stockpile_zones().any(|z| z == pos),
            sowing: world.sow_designations().any(|z| z == pos),
            clearing: world.clear_designations().any(|z| z == pos),
            bend: world.bend_designations().any(|(_, p)| p == pos),
            deconstructing: world.is_marked_for_deconstruct(pos),
            plot: world
                .plot_stage(pos)
                .filter(|_| world.footprint_at(pos).is_some_and(|f| f.anchor == pos))
                .map(|_| world.is_standing_sow(pos)),
        }
    }
}

impl Tool {
    /// Every tool, in the order the side panel lists them.
    pub const ALL: [Tool; 8] = [
        Self::Select,
        Self::Harvest,
        Self::CancelHarvest,
        Self::Clear,
        Self::Stockpile,
        Self::Bend,
        Self::Build,
        Self::Sow,
    ];

    /// The short name for a button.
    pub fn name(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Harvest => "Harvest",
            Self::CancelHarvest => "Cancel",
            Self::Clear | Self::Deconstruct => "Clear",
            Self::Stockpile => "Stockpile",
            Self::Bend => "Bend",
            Self::Build => "Build",
            Self::Sow => "Sow",
            Self::Gate => "Gate",
        }
    }

    /// The key that picks the tool, as shown to the player: "H", or "Esc" for `Select`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Select => "Esc",
            Self::Harvest => "H",
            Self::CancelHarvest => "X",
            Self::Clear | Self::Deconstruct => "E",
            Self::Stockpile => "P",
            Self::Bend => "B",
            Self::Build => "U",
            Self::Sow => "C",
            Self::Gate => "G Enter",
        }
    }

    /// The letter key that picks the tool; `None` for `Select`, which is `Esc`, and for `Gate`,
    /// which the Endeavor screen arms. Always upper case.
    pub fn letter(self) -> Option<char> {
        match self {
            Self::Select | Self::Gate => None,
            other => other.key().chars().next(),
        }
    }

    /// The tool a letter key picks, in either case.
    pub fn from_letter(c: char) -> Option<Self> {
        let c = c.to_ascii_uppercase();
        Self::ALL.into_iter().find(|t| t.letter() == Some(c))
    }

    /// What the tool does, in a sentence.
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "select units",
            // The client that read held Shift as a standing order is gone (#281); the TUI's
            // own `O` key stands in for it (#277).
            Self::Harvest => "designate harvest (chop, mine, gather)",
            Self::CancelHarvest => {
                "cancel a harvest, bend, sow or clear mark (O: also a build site or stockpile)"
            }
            Self::Clear => "clear the ground",
            Self::Deconstruct => "take down buildings",
            Self::Stockpile => "designate stockpile",
            Self::Bend => "channel the nation's ability",
            Self::Build => "place the chosen building (U: next building)",
            Self::Sow => "sow an empty farm plot",
            Self::Gate => "place the gate",
        }
    }

    /// `label`, except that `Bend` names the ability it channels, since "channel the nation's
    /// ability" alone doesn't say which (docs/design/bending.md). With more than one usable it
    /// says how to move on to the next.
    pub fn hint(self, session: &Session) -> String {
        if self == Self::Bend
            && let Some(id) = bend_ability(session)
        {
            let def = session.defs.ability(id);
            let more = if usable_abilities(session).len() > 1 { " (B: next ability)" } else { "" };
            return format!("{}: {}{more}", def.verb, def.description);
        }
        if self == Self::Gate {
            return format!("place {}: Enter at the cursor", session.defs.endeavor.name);
        }
        self.label().to_owned()
    }

    /// What the tool does, in a sentence or two, for the Tool section of the side panel (#657):
    /// the text the hint bar once carried after its `|`, without the keys (they are the key
    /// table's, `crate::keys`). `Bend` says what the ability it channels does, from the data.
    /// `""` for `Select`.
    pub fn about(self, session: &Session) -> String {
        match self {
            Self::Select => String::new(),
            Self::Harvest => "Chop, mine or gather what is inside the rectangle.".to_owned(),
            Self::CancelHarvest => {
                "Take back the harvest, bend, sow and clear marks inside the rectangle.".to_owned()
            }
            Self::Clear => "Clear the trees, rock and plants inside the rectangle for good.".into(),
            Self::Deconstruct => "Take down the finished buildings the rectangle touches.".into(),
            Self::Stockpile => "Mark the rectangle as stockpile, where goods are kept.".into(),
            Self::Bend => match bend_ability(session) {
                Some(id) => session.defs.ability(id).description.clone(),
                None => "Bend the nation's ability inside the rectangle.".to_owned(),
            },
            Self::Build => "Place the chosen building as often as the rectangle holds it.".into(),
            Self::Sow => "Sow the empty farm plots inside the rectangle.".to_owned(),
            Self::Gate => format!("Place {} at the cursor.", session.defs.endeavor.name),
        }
    }

    /// What the Tool section says when a rectangle would do nothing: "Nothing to harvest".
    pub fn nothing(self) -> &'static str {
        match self {
            Self::Select | Self::Gate => "",
            Self::Harvest => "Nothing to harvest",
            Self::CancelHarvest => "Nothing to remove",
            Self::Clear => "Nothing to clear",
            Self::Deconstruct => "No buildings to take down",
            Self::Stockpile => "Nothing to add",
            Self::Bend => "Nothing to bend",
            Self::Build => "Nothing to build",
            Self::Sow => "No empty plots to sow",
        }
    }

    /// The command this tool issues for one tile, if it issues any. `here` says what else is
    /// designated on the tile. The cancel tool withdraws a job designation (a bend, a Sow mark,
    /// a Clear mark, else a plain harvest) and leaves a build site or a stockpile tile alone,
    /// unless `pick.teardown` (#335) is set, in which case those come first.
    pub fn command(self, pos: Pos, here: Here, pick: Pick) -> Option<Command> {
        match self {
            Self::Select => None,
            // The standing-order toggle on a farm plot is its standing sow order (`calendar`).
            Self::Harvest if pick.repeating && here.plot.is_some() => {
                Some(Command::StandingSow { pos, on: here.plot == Some(false) })
            }
            Self::Harvest if pick.repeating => Some(Command::DesignateRepeatingHarvest { pos }),
            Self::Harvest => Some(Command::DesignateHarvest { pos }),
            Self::CancelHarvest if pick.teardown && here.site => Some(Command::CancelBuild { pos }),
            Self::CancelHarvest if pick.teardown && here.zone => {
                Some(Command::CancelStockpile { pos })
            }
            Self::CancelHarvest if here.bend => Some(Command::CancelBend { pos }),
            Self::CancelHarvest if here.sowing => Some(Command::CancelSow { pos }),
            Self::CancelHarvest if here.clearing => Some(Command::CancelClear { pos }),
            Self::CancelHarvest if here.deconstructing => Some(Command::CancelDeconstruct { pos }),
            Self::CancelHarvest => Some(Command::CancelHarvest { pos }),
            Self::Clear => Some(Command::DesignateClear { pos }),
            Self::Deconstruct => Some(Command::Deconstruct { pos }),
            Self::Stockpile => Some(Command::DesignateStockpile { pos }),
            Self::Bend => Some(Command::Bend { ability: pick.ability?, pos }),
            Self::Build => Some(Command::DesignateBuild { pos, building: pick.building? }),
            Self::Sow => Some(Command::Sow { pos }),
            // The gate's anchor depends on its footprint, which `command` has no data for:
            // `apply_rect` hands it to `crate::gate::place` instead.
            Self::Gate => None,
        }
    }
}

/// The nation's signature ability: its first in the data, which needs no research. `None` only
/// if a nation has no ability defined at all, which the data tests guard against.
pub fn ability_for_nation(defs: &Defs, nation: Nation) -> Option<AbilityId> {
    defs.abilities.iter().position(|a| a.nation == nation).map(|i| AbilityId(i as u16))
}

/// Every ability this settlement can use now (its nation's, with their tech known), in data
/// order: the signature one first, then what research has unlocked (#123).
pub fn usable_abilities(session: &Session) -> Vec<AbilityId> {
    (0..session.defs.abilities.len() as u16)
        .map(AbilityId)
        .filter(|&id| session.world.check_ability(id, &session.defs).is_ok())
        .collect()
}

/// The ability the Bend tool channels: the one picked with `B` while it is still usable, else the
/// first usable (the signature ability).
pub fn bend_ability(session: &Session) -> Option<AbilityId> {
    let usable = usable_abilities(session);
    session
        .bend_choice
        .filter(|c| usable.contains(c))
        .or_else(|| usable.first().copied())
        .or_else(|| ability_for_nation(&session.defs, session.world.nation))
}

/// `B` pressed while the Bend tool is already active moves on to the next usable ability,
/// wrapping round. Any other key leaves the choice alone, so coming back to `B` returns to it.
pub fn cycle_ability(session: &mut Session, current: Tool, requested: Tool) {
    if current != Tool::Bend || requested != Tool::Bend {
        return;
    }
    let usable = usable_abilities(session);
    let now = bend_ability(session);
    let next = now.and_then(|n| usable.iter().position(|&a| a == n)).map_or(0, |i| i + 1);
    session.bend_choice = usable.get(next % usable.len().max(1)).copied();
}

/// Every tile in the rectangle spanned by two corners, row by row.
pub fn tiles_between(a: Pos, b: Pos) -> impl Iterator<Item = Pos> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| Pos::new(x, y)))
}

/// The anchors a Build drag from `from` to `to` stamps for a building of `footprint` size: a
/// lattice from the drag's start, stepping by the footprint away from it, keeping only the stamps
/// that fit inside the dragged rectangle, in row-major order. So a drag along a row lays
/// buildings side by side, and a 1x1 drag stamps every tile, as for any other tool. A drag too
/// small to fit one (a click) places one, anchored for the cursor (`build::anchor_for`).
pub fn build_stamps(from: Pos, to: Pos, footprint: (u8, u8)) -> Vec<Pos> {
    // The anchor's coordinate on one axis for each stamp that fits between `start` and `end`.
    let axis = |start: i32, end: i32, size: u8| -> Vec<i32> {
        let size = i32::from(size);
        let fit = ((end - start).abs() + 1) / size;
        (0..fit)
            .map(|k| if end >= start { start + k * size } else { start - (k + 1) * size + 1 })
            .collect()
    };
    let (xs, ys) = (axis(from.x, to.x, footprint.0), axis(from.y, to.y, footprint.1));
    if xs.is_empty() || ys.is_empty() {
        return vec![build::anchor_for(from, footprint)];
    }
    let mut stamps: Vec<Pos> =
        ys.iter().flat_map(|&y| xs.iter().map(move |&x| Pos::new(x, y))).collect();
    stamps.sort_by_key(|p| (p.y, p.x));
    stamps
}

/// The tool and building choice after the player asks for `requested`. `Build` picks the first
/// building the nation can place when none is chosen (or the chosen one is locked); asking for
/// it again while it is active moves on to the next one. Every other tool leaves the choice
/// alone, so `Build` returns to the building last picked. `Clear` (its key, `E`) starts in
/// ground mode, and asked for again it toggles between ground and buildings (#342,
/// docs/design/clearing.md, section 5): a careless rectangle uproots trees, never houses.
pub fn switch_tool(
    session: &Session,
    current: Tool,
    choice: Option<BuildingId>,
    requested: Tool,
) -> (Tool, Option<BuildingId>) {
    if matches!(requested, Tool::Clear | Tool::Deconstruct) {
        let next = match current {
            Tool::Clear => Tool::Deconstruct,
            _ => Tool::Clear,
        };
        return (next, choice);
    }
    if requested != Tool::Build {
        return (requested, choice);
    }
    let rows = build::picker_rows(session);
    let choice = if current == Tool::Build {
        build::next_choice(&rows, choice)
    } else {
        build::valid_choice(&rows, choice)
    };
    (Tool::Build, choice)
}

/// How many of each kind a Cancel rectangle withdrew (#335: "the cancel notice says what was
/// removed, by kind"), so the notice can say "12 harvest marks, 1 stockpile" instead of a bare
/// count that can't tell a dropped harvest job from a torn-down stockpile.
#[derive(Default)]
struct CancelTally {
    harvest: u32,
    bend: u32,
    sow: u32,
    clear: u32,
    deconstruct: u32,
    site: u32,
    zone: u32,
}

impl CancelTally {
    /// Counts one command a Cancel rectangle's tile issued and the sim accepted.
    fn record(&mut self, command: Command) {
        match command {
            Command::CancelHarvest { .. } => self.harvest += 1,
            Command::CancelBend { .. } => self.bend += 1,
            Command::CancelSow { .. } => self.sow += 1,
            Command::CancelClear { .. } => self.clear += 1,
            Command::CancelDeconstruct { .. } => self.deconstruct += 1,
            Command::CancelBuild { .. } => self.site += 1,
            Command::CancelStockpile { .. } => self.zone += 1,
            _ => {}
        }
    }

    /// What the rectangle withdrew, by kind, in the notice's order.
    fn counts(&self) -> Vec<Count> {
        let kinds = [
            (self.harvest, "harvest mark", "harvest marks"),
            (self.bend, "bend mark", "bend marks"),
            (self.sow, "sow mark", "sow marks"),
            (self.clear, "clear mark", "clear marks"),
            (self.deconstruct, "building mark", "building marks"),
            (self.site, "build site", "build sites"),
            (self.zone, "stockpile", "stockpiles"),
        ];
        kinds
            .into_iter()
            .filter(|&(n, ..)| n > 0)
            .map(|(n, one, many)| Count {
                n,
                one: one.to_owned(),
                many: many.to_owned(),
                then: "to remove".to_owned(),
            })
            .collect()
    }

    /// "cancel: 12 harvest marks, 1 stockpile, 2 skipped (not designated)", in the same
    /// "skipped (reason)" shape every other tool's notice has, but with the done count broken
    /// out by kind instead of a single number that can't tell a harvest mark from a stockpile.
    fn notice(&self, refused: u32, last_error: Option<CommandError>, session: &Session) -> String {
        let mut kinds = Vec::new();
        let mut push = |n: u32, one: &str, many: &str| {
            if n > 0 {
                kinds.push(format!("{n} {}", if n == 1 { one } else { many }));
            }
        };
        push(self.harvest, "harvest mark", "harvest marks");
        push(self.bend, "bend mark", "bend marks");
        push(self.sow, "sow mark", "sow marks");
        push(self.clear, "clear mark", "clear marks");
        push(self.deconstruct, "building mark", "building marks");
        push(self.site, "build site", "build sites");
        push(self.zone, "stockpile", "stockpiles");
        let done = if kinds.is_empty() { "0 done".to_owned() } else { kinds.join(", ") };
        match last_error {
            Some(e) => {
                let why = build::describe_error(session, e);
                format!("cancel: {done}, {refused} skipped ({why})")
            }
            None => format!("cancel: {done}, {refused} skipped"),
        }
    }
}

/// The tiles a buildings-mode Clear rectangle (#342) issues `Deconstruct` on: one per finished
/// building it touches (its anchor), in the order the rectangle first meets each, so a 2x2 hall
/// counts once in the notice rather than once marked and three times "already designated". A
/// rectangle that touches no building at all gives its tiles back, so the notice still says why
/// nothing was marked ("no building here").
fn deconstruct_targets(session: &Session, from: Pos, to: Pos) -> Vec<Pos> {
    let mut anchors: Vec<Pos> = Vec::new();
    for pos in tiles_between(from, to) {
        if let Some(b) = session.world.building_record(pos)
            && !anchors.contains(&b.anchor)
        {
            anchors.push(b.anchor);
        }
    }
    if anchors.is_empty() { tiles_between(from, to).collect() } else { anchors }
}

/// Something a rectangle marks, counted: "7 trees to chop". The words are the data's
/// (`HarvestDef::noun`) or the tool's own; the count is the number of tiles (or sites) whose
/// command the sim accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Count {
    pub n: u32,
    one: String,
    many: String,
    /// What applying does to them, after the noun: "to chop".
    then: String,
}

impl Count {
    /// "7 trees", "1 tree".
    pub fn noun(&self) -> String {
        format!("{} {}", self.n, if self.n == 1 { &self.one } else { &self.many })
    }

    /// "7 trees to chop": what applying the tool would do to them.
    pub fn preview(&self) -> String {
        format!("{} {}", self.noun(), self.then)
    }
}

/// One more of `(one, many, then)` among `counts`.
fn bump(counts: &mut Vec<Count>, one: &str, many: &str, then: &str) {
    if let Some(c) = counts.iter_mut().find(|c| c.one == one && c.then == then) {
        c.n += 1;
        return;
    }
    counts.push(Count { n: 1, one: one.to_owned(), many: many.to_owned(), then: then.to_owned() });
}

/// A rectangle for a tool: what [`apply_order`] does and [`preview_order`] says it would do.
#[derive(Clone, Copy, Debug)]
pub struct Order {
    pub tool: Tool,
    pub from: Pos,
    pub to: Pos,
    /// The standing-order toggle (#81, `Pick::repeating`).
    pub repeating: bool,
    /// The Cancel tool's `O` toggle (#335, `Pick::teardown`).
    pub teardown: bool,
    /// The Build tool's building.
    pub choice: Option<BuildingId>,
}

/// What a rectangle did, or, from [`preview_order`], would do. Both come out of the one loop
/// (`run_order`), so the preview and the result cannot disagree (#657).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The old one-line result ("designate harvest: 12 done, 3 skipped"), for the log and the
    /// agent's notice.
    pub notice: String,
    /// What the summary calls the marks: "Marked", "Added" or "Removed".
    pub lead: &'static str,
    /// What was marked, by kind, for the tools that tell kinds apart (Harvest by what it works,
    /// Cancel by what it withdrew); one entry for the rest. Empty when nothing was.
    pub counts: Vec<Count>,
    /// Commands the sim refused (the tiles with nothing for the tool).
    pub refused: u32,
    /// Marks no settler can reach (#337); only counted when applying.
    pub unreachable: u32,
    /// Why the last command was refused, as a notice words it ("requires Fire Making").
    pub why: Option<String>,
    /// The Build tool's tiles of resources to clear before the sites can go up.
    pub first: u32,
    /// The Build tool's total cost for the sites marked: "12 Wood, 4 Stone".
    pub cost: Option<String>,
}

impl Report {
    /// Whether any command went through.
    pub fn done(&self) -> bool {
        self.counts.iter().any(|c| c.n > 0)
    }

    /// What the rectangle did, in a sentence: "Marked 7 trees, 2 rock.", or "Nothing marked:
    /// nothing to harvest." The Build tool adds the tiles it clears first.
    pub fn summary(&self) -> String {
        if !self.done() {
            let nothing = match self.lead {
                "Removed" => "Nothing removed",
                "Added" => "Nothing added",
                _ => "Nothing marked",
            };
            return match &self.why {
                Some(why) => format!("{nothing}: {why}."),
                None => format!("{nothing}."),
            };
        }
        // What to do with them is the preview's; "Marked" needs only the things, so plants to
        // gather and plants to pick are plants.
        let mut things: Vec<Count> = Vec::new();
        for c in &self.counts {
            match things.iter_mut().find(|t| t.one == c.one) {
                Some(t) => t.n += c.n,
                None => things.push(c.clone()),
            }
        }
        let kinds: Vec<String> = things.iter().map(Count::noun).collect();
        let mut text = format!("{} {}.", self.lead, kinds.join(", "));
        if self.first > 0 {
            let tiles = if self.first == 1 { "tile" } else { "tiles" };
            text.push_str(&format!(" Clearing {} {tiles} first.", self.first));
        }
        if self.unreachable > 0 {
            text.push_str(&format!(" {} unreachable.", self.unreachable));
        }
        text
    }
}

/// Applies `tool` to every tile in the rectangle `from`..=`to` (for `Build`, to every footprint
/// `build_stamps` fits in it) through `Session::apply` and returns the notice to show, such as "designate harvest: 12 done, 3 skipped (nothing to
/// harvest), 2 unreachable". `repeating` is the Shift state (#81); `teardown` is the Cancel
/// tool's own `O` toggle (#335, `Pick::teardown`); `choice` is the Build tool's building.
///
/// The sim decides what each tile accepts; a refusal is counted and its reason (the last one's)
/// shown, even for a one-tile click, so a rejected bend or harvest says why. For `Harvest` and
/// `Stockpile`, a designation that went through is also checked against `World::job_unreachable`
/// (#337: "designating a job or stockpile no settler can reach says so in the notice"), so a tile
/// on the wrong side of a river, wall or shore is called out rather than silently "done". `Cancel`
/// builds its own notice instead (`CancelTally::notice`), broken down by what it withdrew.
pub fn apply_rect(
    session: &mut Session,
    tool: Tool,
    from: Pos,
    to: Pos,
    repeating: bool,
    teardown: bool,
    choice: Option<BuildingId>,
) -> String {
    apply_order(session, Order { tool, from, to, repeating, teardown, choice }).notice
}

/// [`apply_rect`], with the counts of what it marked: the Tool section's "Marked 7 trees."
pub fn apply_order(session: &mut Session, order: Order) -> Report {
    run_order(session, order, true)
}

/// What [`apply_order`] would do, without doing it: the same loop run over a copy of the
/// session, so the counts are the ones applying gives. It costs that copy, so a frontend asks
/// only while a corner is set. Marks nobody can reach are not looked for (that needs the
/// settlement's regions worked out, which the real thing does), so `unreachable` is 0.
pub fn preview_order(session: &Session, order: Order) -> Report {
    run_order(&mut session.clone(), order, false)
}

/// The loop behind [`apply_order`] and [`preview_order`]. `reach` asks whether each Harvest or
/// Stockpile mark can be reached.
fn run_order(session: &mut Session, order: Order, reach: bool) -> Report {
    let Order { tool, from, to, repeating, teardown, choice } = order;
    if tool == Tool::Select {
        return Report::default();
    }
    // One site, centred where the rectangle ends: the gate has no rectangle of its own.
    if tool == Tool::Gate {
        let notice = crate::gate::place(session, to).unwrap_or_else(|why| why);
        return Report { notice, ..Report::default() };
    }
    let rows = build::picker_rows(session);
    let building = build::valid_choice(&rows, choice);
    let pick = Pick { ability: bend_ability(session), repeating, building, teardown };
    let what = match (tool, building) {
        (Tool::Build, Some(id)) => {
            let name = rows.iter().find(|r| r.id == id).map_or("", |r| r.name.as_str());
            format!("build {name}")
        }
        // Short, not the key-help label ("clear the ground"): docs/design/clearing.md's own
        // example notice is `clear: 6 marked, 2 skipped (can't be cleared)`.
        (Tool::Clear, _) => "clear".to_owned(),
        (Tool::Deconstruct, _) => "clear buildings".to_owned(),
        _ => tool.label().to_owned(),
    };
    let targets: Vec<Pos> = match (tool, building) {
        (Tool::Build, Some(id)) => build_stamps(from, to, session.defs.building(id).footprint),
        (Tool::Deconstruct, _) => deconstruct_targets(session, from, to),
        _ => tiles_between(from, to).collect(),
    };
    // So `job_unreachable` below has an answer even for the very first designation in a fresh
    // game, before any idle unit's own job search would otherwise have warmed the cache (#337).
    if reach && matches!(tool, Tool::Harvest | Tool::Stockpile) {
        session.world.ensure_regions();
    }
    let (mut done, mut refused, mut unreachable, mut first) = (0, 0, 0, 0);
    let mut counts: Vec<Count> = Vec::new();
    let mut tally = CancelTally::default();
    let mut last_error = None;
    for pos in targets {
        let Some(command) = tool.command(pos, Here::at(session, pos), pick) else { continue };
        // What the tile holds, before the command changes it.
        let harvest = (tool == Tool::Harvest)
            .then(|| {
                let resource = session.world.map.get(pos)?.resource?;
                let h = session.defs.harvest(resource)?;
                Some((h.noun.clone(), h.noun_many.clone(), format!("to {}", h.verb.to_lowercase())))
            })
            .flatten();
        let clears = match (tool, building) {
            (Tool::Build, Some(id)) => session.world.to_clear(pos, id, &session.defs).len() as u32,
            _ => 0,
        };
        match session.apply(command) {
            Ok(()) => {
                done += 1;
                match tool {
                    Tool::Harvest if let Command::StandingSow { on, .. } = command => {
                        let then = if on { "to re-sow" } else { "to stop re-sowing" };
                        bump(&mut counts, "plot", "plots", then);
                    }
                    Tool::Harvest => {
                        let (one, many, then) = harvest.unwrap_or_else(|| {
                            ("tile".to_owned(), "tiles".to_owned(), "to harvest".to_owned())
                        });
                        bump(&mut counts, &one, &many, &then);
                    }
                    Tool::CancelHarvest => tally.record(command),
                    Tool::Clear => bump(&mut counts, "tile", "tiles", "to clear"),
                    Tool::Deconstruct => bump(&mut counts, "building", "buildings", "to take down"),
                    Tool::Stockpile => bump(&mut counts, "tile", "tiles", "to add"),
                    Tool::Bend => bump(&mut counts, "tile", "tiles", "to bend"),
                    Tool::Build => {
                        first += clears;
                        bump(&mut counts, "site", "sites", "to build");
                    }
                    Tool::Sow => bump(&mut counts, "plot", "plots", "to sow"),
                    Tool::Select | Tool::Gate => {}
                }
                if reach
                    && matches!(tool, Tool::Harvest | Tool::Stockpile)
                    && session.world.job_unreachable(pos) == Some(true)
                {
                    unreachable += 1;
                }
            }
            Err(e) => {
                refused += 1;
                last_error = Some(e);
            }
        }
    }
    match tool {
        Tool::Harvest => counts.sort_by(|a, b| (&a.then, &a.one).cmp(&(&b.then, &b.one))),
        Tool::CancelHarvest => counts = tally.counts(),
        _ => {}
    }
    let why = last_error.map(|e| build::describe_error(session, e));
    let cost = match (tool, building) {
        (Tool::Build, Some(id)) if done > 0 => Some(build::cost_of(session, id, done)),
        _ => None,
    };
    let lead = match tool {
        Tool::CancelHarvest => "Removed",
        Tool::Stockpile => "Added",
        _ => "Marked",
    };
    let notice = if tool == Tool::CancelHarvest {
        tally.notice(refused, last_error, session)
    } else {
        let suffix =
            if unreachable > 0 { format!(", {unreachable} unreachable") } else { String::new() };
        // Clear marks tiles rather than finishing work on them (a unit still has to do the
        // clearing), so its own notice says "marked" (docs/design/clearing.md, "One removal
        // vocabulary"); every other tool keeps "done".
        let verb = if matches!(tool, Tool::Clear | Tool::Deconstruct) { "marked" } else { "done" };
        match &why {
            Some(why) => format!("{what}: {done} {verb}, {refused} skipped ({why}){suffix}"),
            None => format!("{what}: {done} {verb}, {refused} skipped{suffix}"),
        }
    };
    Report { notice, lead, counts, refused, unreachable, why, first, cost }
}

#[cfg(test)]
mod tests {
    use fn_sim::world::Command;

    use super::*;

    #[test]
    fn rectangle_covers_both_corners_whatever_the_drag_direction() {
        let a = Pos::new(5, 7);
        let b = Pos::new(3, 8);
        let tiles: Vec<Pos> = tiles_between(a, b).collect();
        assert_eq!(tiles.len(), 6);
        assert_eq!(tiles.first(), Some(&Pos::new(3, 7)));
        assert_eq!(tiles.last(), Some(&Pos::new(5, 8)));
        assert_eq!(tiles, tiles_between(b, a).collect::<Vec<_>>());
    }

    #[test]
    fn a_click_is_a_one_tile_rectangle() {
        let p = Pos::new(2, 2);
        assert_eq!(tiles_between(p, p).collect::<Vec<_>>(), [p]);
    }

    #[test]
    fn each_tool_issues_its_own_command() {
        let pos = Pos::new(1, 2);
        let none = Pick::default();
        assert_eq!(Tool::Select.command(pos, Here::default(), none), None);
        assert_eq!(
            Tool::Harvest.command(pos, Here::default(), none),
            Some(Command::DesignateHarvest { pos })
        );
        assert_eq!(
            Tool::CancelHarvest.command(pos, Here::default(), none),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(
            Tool::Stockpile.command(pos, Here::default(), none),
            Some(Command::DesignateStockpile { pos })
        );
        let ability = AbilityId(0);
        let bend = Pick { ability: Some(ability), ..none };
        assert_eq!(
            Tool::Bend.command(pos, Here::default(), bend),
            Some(Command::Bend { ability, pos })
        );
        let building = BuildingId(2);
        let build = Pick { building: Some(building), ..none };
        assert_eq!(
            Tool::Build.command(pos, Here::default(), build),
            Some(Command::DesignateBuild { pos, building })
        );
        assert_eq!(Tool::Sow.command(pos, Here::default(), none), Some(Command::Sow { pos }));
        assert_eq!(
            Tool::Clear.command(pos, Here::default(), none),
            Some(Command::DesignateClear { pos })
        );
    }

    #[test]
    fn cancel_withdraws_a_clear_mark_where_there_is_one() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        let teardown = Pick { teardown: true, ..none };
        let clearing = Here { clearing: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, clearing, none),
            Some(Command::CancelClear { pos })
        );
        // A build site is left alone without the teardown toggle (#335): the clear mark still
        // goes, same as any other job designation.
        let site = Here { site: true, clearing: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, site, none),
            Some(Command::CancelClear { pos })
        );
        // With teardown on, the build site comes first.
        assert_eq!(
            Tool::CancelHarvest.command(pos, site, teardown),
            Some(Command::CancelBuild { pos })
        );
        // The Clear tool itself is unaffected by what else is on the tile.
        assert_eq!(Tool::Clear.command(pos, clearing, none), Some(Command::DesignateClear { pos }));
    }

    #[test]
    fn cancel_withdraws_a_sow_designation_where_there_is_one() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        let teardown = Pick { teardown: true, ..none };
        let sowing = Here { sowing: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, sowing, none),
            Some(Command::CancelSow { pos })
        );
        // A build site is left alone without the teardown toggle (#335): the sow mark still
        // goes.
        let site = Here { site: true, sowing: true, ..Here::default() };
        assert_eq!(Tool::CancelHarvest.command(pos, site, none), Some(Command::CancelSow { pos }));
        // With teardown on, the build site comes first.
        assert_eq!(
            Tool::CancelHarvest.command(pos, site, teardown),
            Some(Command::CancelBuild { pos })
        );
    }

    #[test]
    fn build_needs_a_chosen_building_to_issue_a_command() {
        assert_eq!(Tool::Build.command(Pos::new(0, 0), Here::default(), Pick::default()), None);
    }

    /// #335: a build site isn't a job designation, so a plain cancel leaves it alone and falls
    /// back to a plain harvest cancel (refused as "not designated" once the sim sees there's no
    /// harvest job either); only the teardown toggle withdraws it.
    #[test]
    fn cancel_withdraws_a_build_site_only_with_teardown_on() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        let teardown = Pick { teardown: true, ..none };
        let site = Here { site: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, site, none),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(
            Tool::CancelHarvest.command(pos, site, teardown),
            Some(Command::CancelBuild { pos })
        );
        // Only the cancel tool does.
        assert_eq!(Tool::Harvest.command(pos, site, none), Some(Command::DesignateHarvest { pos }));
    }

    /// #335: a stockpile tile isn't a job designation either, so the same teardown gate applies.
    #[test]
    fn cancel_withdraws_a_stockpile_tile_only_with_teardown_on() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        let teardown = Pick { teardown: true, ..none };
        let zone = Here { zone: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, zone, none),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(
            Tool::CancelHarvest.command(pos, zone, teardown),
            Some(Command::CancelStockpile { pos })
        );
        assert_eq!(
            Tool::Stockpile.command(pos, zone, none),
            Some(Command::DesignateStockpile { pos })
        );
        // A build site comes first, with teardown on.
        let both = Here { site: true, zone: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, both, teardown),
            Some(Command::CancelBuild { pos })
        );
    }

    /// #335: a bend designation is a job like a harvest's, so a plain cancel withdraws it too.
    #[test]
    fn cancel_withdraws_a_bend_designation_where_there_is_one() {
        let pos = Pos::new(4, 4);
        let none = Pick::default();
        let bending = Here { bend: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, bending, none),
            Some(Command::CancelBend { pos })
        );
        // A bend comes first (an arbitrary tie-break; a tile never actually carries both).
        let sowing = Here { bend: true, sowing: true, ..Here::default() };
        assert_eq!(
            Tool::CancelHarvest.command(pos, sowing, none),
            Some(Command::CancelBend { pos })
        );
    }

    #[test]
    fn holding_shift_makes_a_harvest_designation_repeating() {
        let pos = Pos::new(1, 2);
        let shift = Pick { repeating: true, ..Pick::default() };
        assert_eq!(
            Tool::Harvest.command(pos, Here::default(), shift),
            Some(Command::DesignateRepeatingHarvest { pos })
        );
        // Shift only changes what the harvest tool does.
        assert_eq!(
            Tool::CancelHarvest.command(pos, Here::default(), shift),
            Some(Command::CancelHarvest { pos })
        );
        assert_eq!(Tool::Select.command(pos, Here::default(), shift), None);
    }

    #[test]
    fn bend_needs_an_ability_to_issue_a_command() {
        assert_eq!(Tool::Bend.command(Pos::new(0, 0), Here::default(), Pick::default()), None);
    }

    #[test]
    fn ability_for_nation_finds_each_nation_s_one_signature_ability() {
        let defs = Defs::bundled();
        for nation in Nation::ALL {
            let id = ability_for_nation(&defs, nation).unwrap_or_else(|| panic!("{nation:?}"));
            assert_eq!(defs.ability(id).nation, nation);
        }
    }

    #[test]
    fn a_new_game_can_use_only_the_signature_ability_so_b_has_nothing_to_cycle() {
        for nation in Nation::ALL {
            let mut s = Session::new(1, nation, 48, 32);
            let signature = ability_for_nation(&s.defs, nation).unwrap();
            assert_eq!(usable_abilities(&s), [signature], "{nation:?}");
            assert_eq!(bend_ability(&s), Some(signature));
            cycle_ability(&mut s, Tool::Bend, Tool::Bend);
            assert_eq!(bend_ability(&s), Some(signature), "{nation:?}");
            assert!(!Tool::Bend.hint(&s).contains("next ability"));
        }
    }

    #[test]
    fn b_again_cycles_through_what_research_has_unlocked() {
        // Fire's Inner Fire needs Dancing Dragon; research it the slow way, with the settlers
        // kept fed so the passive trickle has time to pay for it. Forging, a prereq on the way
        // (through Smelting), also unlocks Weld (#363), so it comes along for the ride.
        let mut s = Session::new(3, Nation::Fire, 48, 32);
        let dragon = s.defs.tech_id("dancing_dragon").unwrap();
        let (hearth, inner, weld) = (
            s.defs.ability_id("hearthflame").unwrap(),
            s.defs.ability_id("inner_fire").unwrap(),
            s.defs.ability_id("weld").unwrap(),
        );
        assert_eq!(bend_ability(&s), Some(hearth));
        s.apply(Command::SetResearch { tech: dragon }).unwrap();
        for _ in 0..200_000 {
            if s.world.research().is_known(dragon) {
                break;
            }
            s.tick();
        }
        assert!(s.world.research().is_known(dragon), "researched");
        assert_eq!(usable_abilities(&s), [hearth, inner, weld]);
        assert!(Tool::Bend.hint(&s).contains("B: next ability"));

        // Picking the tool leaves the choice; B again moves on, wrapping round.
        cycle_ability(&mut s, Tool::Select, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(hearth));
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(inner));
        assert!(Tool::Bend.hint(&s).starts_with(&s.defs.ability(inner).verb));
        // Another tool and back keeps it; the tool then issues that ability.
        cycle_ability(&mut s, Tool::Bend, Tool::Harvest);
        cycle_ability(&mut s, Tool::Harvest, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(inner));
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(weld));
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(hearth));
    }

    /// #248: in a mixed-nation settlement (#240) `B` also cycles onto a newcomer's signature
    /// ability, not just the settlement's own nation's.
    #[test]
    fn b_also_cycles_onto_a_newcomers_nation() {
        let mut s = Session::new(1, Nation::Fire, 48, 32);
        let hearth = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        let raise_earth = ability_for_nation(&s.defs, Nation::Earth).unwrap();
        assert_eq!(usable_abilities(&s), [hearth], "no Earth newcomer yet");

        s.world.units[1].nation = Some(Nation::Earth);
        let usable = usable_abilities(&s);
        assert_eq!(usable, [raise_earth, hearth], "data order, not which nation founded it");
        assert!(Tool::Bend.hint(&s).contains("B: next ability"));

        assert_eq!(bend_ability(&s), Some(raise_earth), "the first usable, with no choice yet");
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(hearth));
        cycle_ability(&mut s, Tool::Bend, Tool::Bend);
        assert_eq!(bend_ability(&s), Some(raise_earth), "wraps round");
    }

    #[test]
    fn every_tool_has_a_key_and_the_letters_are_distinct() {
        assert_eq!(Tool::Select.key(), "Esc");
        assert_eq!(Tool::Select.letter(), None);
        let letters: Vec<char> = Tool::ALL.iter().filter_map(|t| t.letter()).collect();
        assert_eq!(letters, ['H', 'X', 'E', 'P', 'B', 'U', 'C']);
        for tool in Tool::ALL {
            if let Some(c) = tool.letter() {
                assert_eq!(Tool::from_letter(c), Some(tool));
                assert_eq!(Tool::from_letter(c.to_ascii_lowercase()), Some(tool));
                assert_eq!(tool.key(), c.to_string());
            }
        }
        assert_eq!(Tool::from_letter('Z'), None);
    }

    #[test]
    fn the_hint_names_the_nation_s_own_ability_for_bend() {
        let s = Session::new(1, Nation::Fire, 48, 32);
        let hint = Tool::Bend.hint(&s);
        let id = ability_for_nation(&s.defs, Nation::Fire).unwrap();
        assert!(hint.starts_with(&s.defs.ability(id).verb), "{hint}");
        assert_eq!(Tool::Harvest.hint(&s), Tool::Harvest.label());
    }

    /// A free land tile near the first unit, away from every unit, with room for a 1x1
    /// building's door (#226).
    fn free_tile(s: &Session) -> Pos {
        let bed = s.defs.building_id("bed").unwrap();
        s.world
            .map
            .iter()
            .find(|(p, t)| {
                t.terrain == fn_sim::Terrain::Land
                    && t.resource.is_none()
                    && !s.world.units.iter().any(|u| u.pos == *p)
                    && s.world.check_build(*p, bed, &s.defs).is_ok()
            })
            .map(|(p, _)| p)
            .expect("a land tile")
    }

    #[test]
    fn the_build_key_picks_a_placeable_building_and_pressed_again_cycles() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let rows = build::picker_rows(&s);
        let open: Vec<BuildingId> = rows.iter().filter(|r| !r.locked()).map(|r| r.id).collect();
        assert!(open.len() >= 2);
        let (tool, choice) = switch_tool(&s, Tool::Select, None, Tool::Build);
        assert_eq!((tool, choice), (Tool::Build, Some(open[0])));
        let (tool, choice) = switch_tool(&s, tool, choice, Tool::Build);
        assert_eq!((tool, choice), (Tool::Build, Some(open[1])));
        // Another tool keeps the choice, so coming back to Build returns to it.
        let (tool, kept) = switch_tool(&s, tool, choice, Tool::Harvest);
        assert_eq!((tool, kept), (Tool::Harvest, choice));
        assert_eq!(switch_tool(&s, tool, kept, Tool::Build), (Tool::Build, choice));
    }

    #[test]
    fn the_build_key_never_cycles_through_another_nation_s_buildings() {
        let s = Session::new(7, Nation::Earth, 64, 64);
        let orchard = s.defs.building_id("orchard").unwrap();
        let (mut tool, mut choice) = (Tool::Select, None);
        for _ in 0..s.defs.buildings.len() * 2 {
            (tool, choice) = switch_tool(&s, tool, choice, Tool::Build);
            assert_ne!(choice, Some(orchard));
        }
        // A stale choice of a hidden building falls back to a placeable one.
        let (_, fixed) = switch_tool(&s, Tool::Select, Some(orchard), Tool::Build);
        assert_ne!(fixed, Some(orchard));
        assert!(fixed.is_some());
    }

    #[test]
    fn applying_a_rectangle_counts_what_the_sim_accepted_and_says_why_for_the_rest() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let pos = free_tile(&s);
        let to = Pos::new(pos.x + 1, pos.y);
        let notice = apply_rect(&mut s, Tool::Stockpile, pos, to, false, false, None);
        assert!(notice.starts_with("designate stockpile: "), "{notice}");
        let done = s.world.stockpile_zones().count();
        assert!(done >= 1);
        assert!(notice.contains(&format!("{done} done, {} skipped", 2 - done)), "{notice}");

        // Off the map: all refused, with the sim's reason.
        let notice = apply_rect(
            &mut s,
            Tool::Harvest,
            Pos::new(-2, -2),
            Pos::new(-2, -2),
            false,
            false,
            None,
        );
        assert!(
            notice.starts_with("designate harvest (chop, mine, gather): 0 done, 1 skipped ("),
            "{notice}"
        );
        assert!(notice.ends_with(')'), "{notice}");

        assert_eq!(apply_rect(&mut s, Tool::Select, pos, to, false, false, None), "");
    }

    /// A harvest designated on an island nobody can reach says so in the notice (#337:
    /// "designating a job or stockpile no settler can reach says so in the notice"), not just
    /// "done" like one that will actually get worked.
    #[test]
    fn designating_an_unreachable_harvest_says_so_in_the_notice() {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Defs, Grid, Nation, Terrain, Tile, World};

        use crate::clock::Clock;

        let defs = Defs::bundled();
        let rows = ["~~~~~~~~~", "~T~.....~", "~~~.....~", "~~~~~~~~~"];
        let map = Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
            let c = rows[p.y as usize].as_bytes()[p.x as usize];
            Tile {
                elevation: 100,
                temperature: 128,
                moisture: 128,
                terrain: if c == b'~' { Terrain::Water } else { Terrain::Land },
                biome: BiomeId(0),
                resource: (c == b'T').then(|| defs.resource_id("oak_tree").unwrap()),
                building: None,
            }
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(3, 2)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };
        let island = Pos::new(1, 1);
        let notice = apply_rect(&mut s, Tool::Harvest, island, island, false, false, None);
        assert!(notice.contains("1 done"), "{notice}");
        assert!(notice.contains("1 unreachable"), "{notice}");

        // An ordinary, reachable designation names no unreachable count at all.
        let reachable = Pos::new(5, 1);
        let notice = apply_rect(&mut s, Tool::Stockpile, reachable, reachable, false, false, None);
        assert!(!notice.contains("unreachable"), "{notice}");
    }

    /// Marking a scroll nobody can reach says so in the notice too (#380, "Marking a scroll no
    /// settler can reach says so in the notice"): the same `apply_rect` path above, just with
    /// the resource a playtest actually lost track of.
    #[test]
    fn designating_an_unreachable_scroll_says_so_in_the_notice() {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Defs, Grid, Nation, Terrain, Tile, World};

        use crate::clock::Clock;

        let defs = Defs::bundled();
        let scroll = defs.resource_id("ancient_scroll").unwrap();
        let rows = ["~~~~~~~~~", "~T~.....~", "~~~.....~", "~~~~~~~~~"];
        let map = Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
            let c = rows[p.y as usize].as_bytes()[p.x as usize];
            Tile {
                elevation: 100,
                temperature: 128,
                moisture: 128,
                terrain: if c == b'~' { Terrain::Water } else { Terrain::Land },
                biome: BiomeId(0),
                resource: (c == b'T').then_some(scroll),
                building: None,
            }
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(3, 2)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };
        let island = Pos::new(1, 1);
        let notice = apply_rect(&mut s, Tool::Harvest, island, island, false, false, None);
        assert!(notice.contains("1 done"), "{notice}");
        assert!(notice.contains("1 unreachable"), "{notice}");
    }

    /// docs/design/clearing.md, #411/#426: a fruit tree marks ("clear: N marked"), but a fish
    /// spot -- a renewable with no `[harvest.clear]` table -- is refused with "can't be
    /// cleared", the reason the notice names.
    #[test]
    fn clear_marks_a_fruit_tree_and_refuses_a_fish_spot_as_cant_be_cleared() {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Defs, Grid, Nation, Terrain, Tile, World};

        use crate::clock::Clock;

        let mut defs = Defs::bundled();
        let fruit = defs.resource_id("fruit_tree").unwrap();
        let fish = defs.resource_id("fish_spot").unwrap();
        // Fishing itself is still behind its own tech in the bundled data; drop the gate here so
        // the refusal below is `CannotClear`, not `NotResearched` (the tech gate isn't this
        // test's own subject, as `with_2x2_hall`'s footprint mutation elsewhere in this module
        // also isn't).
        defs.harvests.iter_mut().find(|h| h.resource == fish).unwrap().unlocked_by = None;
        let (fruit_pos, fish_pos) = (Pos::new(1, 0), Pos::new(2, 0));
        let map = Grid::from_fn(4, 2, |p| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if p == fish_pos { Terrain::Water } else { Terrain::Land },
            biome: BiomeId(0),
            resource: if p == fruit_pos {
                Some(fruit)
            } else if p == fish_pos {
                Some(fish)
            } else {
                None
            },
            building: None,
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(0, 1)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };

        let notice = apply_rect(&mut s, Tool::Clear, fruit_pos, fish_pos, false, false, None);
        assert_eq!(notice, "clear: 1 marked, 1 skipped (can't be cleared)", "{notice}");
        assert_eq!(s.world.clear_designations().collect::<Vec<_>>(), [fruit_pos]);
    }

    #[test]
    fn a_bend_drag_designates_the_whole_chain_across_a_lake_at_once() {
        // #304: a Raise Earth drag spanning a lake used to be refused past its first tile; the
        // sim now chains a pending designation off its already-pending neighbour, so the whole
        // line the player dragged over designates in one go, with nothing skipped.
        use fn_sim::defs::BiomeId;
        use fn_sim::{Grid, Terrain, Tile, TileMap, World};

        use crate::clock::Clock;

        let defs = Defs::bundled();
        let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
        let map: TileMap = Grid::from_fn(10, 3, |p| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if (3..=6).contains(&p.x) { Terrain::Lake } else { Terrain::Land },
            biome: plains,
            resource: None,
            building: None,
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(1, 1)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };
        s.bend_choice = Some(s.defs.ability_id("raise_earth").unwrap());

        let notice =
            apply_rect(&mut s, Tool::Bend, Pos::new(3, 1), Pos::new(6, 1), false, false, None);
        assert_eq!(notice, "channel the nation's ability: 4 done, 0 skipped", "{notice}");
        for x in 3..=6 {
            let pos = Pos::new(x, 1);
            assert!(s.world.bend_designations().any(|(_, p)| p == pos), "x={x}");
        }
    }

    /// The restoke rule (#359): a flame that's still nearly full refuses a second Hearthflame on
    /// the same tile, and the notice names why -- the same "done, skipped (reason)" shape every
    /// other tool already reports through, so no bend-specific plumbing was needed.
    #[test]
    fn restoking_a_still_fresh_flame_is_skipped_with_the_restoke_reason() {
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        s.bend_choice = Some(s.defs.ability_id("hearthflame").unwrap());
        let pos = free_tile(&s);

        let first = apply_rect(&mut s, Tool::Bend, pos, pos, false, false, None);
        assert_eq!(first, "channel the nation's ability: 1 done, 0 skipped", "{first}");
        s.tick_n(2000);
        assert!(s.world.flames().iter().any(|f| f.pos == pos), "the flame should be lit by now");

        let second = apply_rect(&mut s, Tool::Bend, pos, pos, false, false, None);
        assert_eq!(
            second,
            "channel the nation's ability: 0 done, 1 skipped \
             (flame still burning, restoke below half)",
            "{second}"
        );
    }

    /// #335: a plain cancel leaves a build site alone (it isn't a job designation); the
    /// teardown toggle withdraws it.
    #[test]
    fn the_build_tool_names_the_building_and_cancel_withdraws_the_site_with_teardown_on() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let bed = s.defs.building_id("bed").unwrap();
        let pos = free_tile(&s);
        let notice = apply_rect(&mut s, Tool::Build, pos, pos, false, false, Some(bed));
        assert_eq!(notice, "build Bed: 1 done, 0 skipped");
        assert!(s.world.build_site(pos).is_some());
        let notice = apply_rect(&mut s, Tool::CancelHarvest, pos, pos, false, false, None);
        assert_eq!(notice, "cancel: 0 done, 1 skipped (not designated)");
        assert!(s.world.build_site(pos).is_some(), "left alone without teardown");
        let notice = apply_rect(&mut s, Tool::CancelHarvest, pos, pos, false, true, None);
        assert_eq!(notice, "cancel: 1 build site, 0 skipped");
        assert!(s.world.build_site(pos).is_none());
    }

    /// #335: a plain cancel over a rectangle keeps a stockpile zone (and the goods on it) in
    /// place; the teardown toggle clears it, as it used to unconditionally.
    #[test]
    fn cancel_drags_over_a_stockpile_and_clears_it_only_with_teardown_on() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let pos = free_tile(&s);
        let to = Pos::new(pos.x + 1, pos.y);
        apply_rect(&mut s, Tool::Stockpile, pos, to, false, false, None);
        let zones: Vec<Pos> = s.world.stockpile_zones().collect();
        assert!(!zones.is_empty());
        let notice = apply_rect(&mut s, Tool::CancelHarvest, pos, to, false, false, None);
        assert!(notice.starts_with("cancel: 0 done"), "{notice}");
        assert_eq!(s.world.stockpile_zones().count(), zones.len(), "left alone without teardown");
        let notice = apply_rect(&mut s, Tool::CancelHarvest, pos, to, false, true, None);
        assert!(notice.contains(&format!("{} stockpile", zones.len())), "{notice}");
        assert_eq!(s.world.stockpile_zones().count(), 0);
    }

    #[test]
    fn a_locked_building_is_replaced_by_the_first_open_one() {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let campfire = s.defs.building_id("campfire").unwrap();
        let pos = free_tile(&s);
        // Earth hasn't learned fire making: the locked choice falls back, so the tool still
        // builds something rather than failing on the tech gate.
        let notice = apply_rect(&mut s, Tool::Build, pos, pos, false, false, Some(campfire));
        assert!(notice.ends_with("1 done, 0 skipped"), "{notice}");
        assert_ne!(s.world.build_site(pos).map(|(b, _, _)| b), Some(campfire));
    }

    /// A session whose bed is a 2x2 building (no bundled building is bigger yet, #214), with a
    /// clear 6x3 field of land from the returned top-left tile: the footprint's own two rows,
    /// plus the row below for every anchor's door (#226).
    fn with_2x2_hall() -> (Session, BuildingId, Pos) {
        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let bed = s.defs.building_id("bed").unwrap();
        let def = &mut s.defs.buildings[bed.0 as usize];
        def.footprint = (2, 2);
        def.bed = None;
        let corner = Pos::new(20, 20);
        for p in tiles_between(corner, Pos::new(corner.x + 5, corner.y + 2)) {
            let t = s.world.map.get_mut(p).unwrap();
            t.terrain = fn_sim::Terrain::Land;
            t.resource = None;
        }
        (s, bed, corner)
    }

    #[test]
    fn a_build_drag_stamps_footprints_side_by_side_and_skips_the_blocked_ones() {
        let (mut s, hall, c) = with_2x2_hall();
        // A fish spot, not an oak tree: an oak is clearable (no tech gate, no `regrow`), so
        // build-over would queue its clear instead of skipping the stamp
        // (`docs/design/clearing.md`, issue #425). A fish spot has no `[harvest.clear]` and
        // Earth hasn't researched fishing, so it still can't be built over.
        let fish = s.defs.resource_id("fish_spot").unwrap();
        s.world.map.get_mut(Pos::new(c.x + 5, c.y + 1)).unwrap().resource = Some(fish);
        let notice = apply_rect(
            &mut s,
            Tool::Build,
            c,
            Pos::new(c.x + 5, c.y + 1),
            false,
            false,
            Some(hall),
        );
        assert_eq!(notice, "build Bed: 2 done, 1 skipped (a fishing spot is in the way)");
        let anchors: Vec<Pos> = s.world.build_sites().map(|(f, _, _)| f.anchor).collect();
        assert_eq!(anchors, [c, Pos::new(c.x + 2, c.y)]);
    }

    #[test]
    fn build_stamps_tile_a_drag_from_its_start_and_fit_inside_it() {
        let p = |x, y| Pos::new(x, y);
        assert_eq!(build_stamps(p(0, 0), p(5, 1), (2, 2)), [p(0, 0), p(2, 0), p(4, 0)]);
        // A 5-wide drag fits two; the leftover column is not stamped.
        assert_eq!(build_stamps(p(0, 0), p(4, 1), (2, 2)), [p(0, 0), p(2, 0)]);
        // Dragged up and to the left, the lattice grows from the start that way.
        assert_eq!(
            build_stamps(p(5, 3), p(0, 0), (2, 2)),
            [p(0, 0), p(2, 0), p(4, 0), p(0, 2), p(2, 2), p(4, 2)]
        );
        // A click, or a drag too small for one, places one at the cursor's anchor.
        assert_eq!(build_stamps(p(7, 7), p(7, 7), (3, 3)), [p(6, 6)]);
        assert_eq!(build_stamps(p(7, 7), p(8, 7), (2, 2)), [p(7, 7)]);
        // 1x1 stamps every tile in the rectangle, in the same order as any other tool.
        let one: Vec<Pos> = tiles_between(p(4, 2), p(1, 3)).collect();
        assert_eq!(build_stamps(p(4, 2), p(1, 3), (1, 1)), one);
    }

    #[test]
    fn a_2x2_site_is_cancelled_from_any_of_its_tiles() {
        let (mut s, hall, c) = with_2x2_hall();
        apply_rect(&mut s, Tool::Build, c, c, false, false, Some(hall));
        let corner = Pos::new(c.x + 1, c.y + 1);
        assert_eq!(Here::at(&s, corner), Here::at(&s, c));
        assert!(Here::at(&s, corner).site);
        // Needs the teardown toggle (#335): a build site isn't a job designation.
        let notice = apply_rect(&mut s, Tool::CancelHarvest, corner, corner, false, true, None);
        assert_eq!(notice, "cancel: 1 build site, 0 skipped");
        assert_eq!(s.world.build_sites().count(), 0);
    }

    /// #335: the acceptance criteria's own scenario -- a plain cancel over a rectangle that
    /// covers both a stockpile already holding goods and a tile still marked for harvest keeps
    /// the stockpile and its goods in place, withdraws the harvest mark, and the notice names
    /// both by kind.
    #[test]
    fn a_plain_cancel_over_a_stockpile_and_harvest_marks_keeps_the_stockpile_and_its_goods() {
        use fn_sim::defs::BiomeId;
        use fn_sim::{Defs, Grid, Nation, Terrain, Tile, World};

        use crate::clock::Clock;

        let defs = Defs::bundled();
        let oak = defs.resource_id("oak_tree").unwrap();
        // A settler at (0,0), a tree to chop at (2,0), the stockpile at (4,0) right next to a
        // second tree at (5,0): the rectangle below cancels exactly those last two tiles.
        let rows = ["L.T..T..", "........", "........"];
        let map = Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
            let c = rows[p.y as usize].as_bytes()[p.x as usize];
            Tile {
                elevation: 100,
                temperature: 128,
                moisture: 128,
                terrain: Terrain::Land,
                biome: BiomeId(0),
                resource: (c == b'T').then_some(oak),
                building: None,
            }
        });
        let world = World::with_map(1, map, Nation::Earth, &[Pos::new(0, 0)], &defs);
        let mut s = Session { world, defs, clock: Clock::default(), bend_choice: None };

        let (stockpile, second_tree) = (Pos::new(4, 0), Pos::new(5, 0));
        apply_rect(&mut s, Tool::Stockpile, stockpile, stockpile, false, false, None);
        apply_rect(&mut s, Tool::Harvest, Pos::new(2, 0), Pos::new(2, 0), false, false, None);

        // Run until the first tree is chopped and hauled to the stockpile.
        for _ in 0..20_000 {
            if s.world.stored_at(stockpile).is_some() {
                break;
            }
            s.tick();
        }
        let stored = s.world.stored_at(stockpile).expect("the first tree's wood made it there");
        assert!(stored.count > 0);

        apply_rect(&mut s, Tool::Harvest, second_tree, second_tree, false, false, None);
        assert!(s.world.harvest_designations().any(|p| p == second_tree));

        let notice =
            apply_rect(&mut s, Tool::CancelHarvest, stockpile, second_tree, false, false, None);
        assert_eq!(notice, "cancel: 1 harvest mark, 1 skipped (not designated)", "{notice}");
        assert!(!s.world.harvest_designations().any(|p| p == second_tree), "{notice}");
        // The stockpile, and the wood already on it, are untouched.
        assert!(s.world.stockpile_zones().any(|z| z == stockpile), "{notice}");
        assert_eq!(s.world.stored_at(stockpile), Some(stored), "{notice}");
    }

    /// #342, docs/design/clearing.md, section 5: `E` starts in ground mode and toggles to
    /// buildings and back; coming from any other tool always lands on ground.
    #[test]
    fn e_starts_in_ground_mode_and_toggles_to_buildings() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        let next = |current| switch_tool(&s, current, None, Tool::Clear).0;
        assert_eq!(next(Tool::Select), Tool::Clear);
        assert_eq!(next(Tool::Harvest), Tool::Clear);
        assert_eq!(next(Tool::Clear), Tool::Deconstruct);
        assert_eq!(next(Tool::Deconstruct), Tool::Clear);
        assert_eq!(Tool::from_letter('e'), Some(Tool::Clear));
        assert!(!Tool::ALL.contains(&Tool::Deconstruct));
    }

    /// A buildings-mode rectangle marks each building it touches once, however many of its
    /// tiles it covers, and `X` takes the mark back from any of them.
    #[test]
    fn a_buildings_rectangle_marks_each_building_once_and_cancel_takes_it_back() {
        use fn_sim::geom::Footprint;

        let mut s = Session::new(7, Nation::Earth, 64, 64);
        let hall = s.defs.building_id("study_hall").unwrap();
        let bed = s.defs.building_id("bed").unwrap();
        // Somewhere free of buildings, away from the camp.
        let (a, b) = (Pos::new(2, 2), Pos::new(5, 2));
        for p in tiles_between(a, Pos::new(5, 3)) {
            assert!(s.world.building_record(p).is_none(), "{p:?}");
        }
        let defs = s.defs.clone();
        s.world.put_building(Footprint::new(a, 2, 2), hall, &defs);
        s.world.put_building(Footprint::single(b), bed, &defs);
        let notice = apply_rect(&mut s, Tool::Deconstruct, a, Pos::new(5, 3), false, false, None);
        assert_eq!(notice, "clear buildings: 2 marked, 0 skipped");
        assert_eq!(s.world.deconstruct_designations().collect::<Vec<_>>(), [a, b]);

        let ground = Pos::new(4, 3);
        let notice = apply_rect(&mut s, Tool::Deconstruct, ground, ground, false, false, None);
        assert_eq!(notice, "clear buildings: 0 marked, 1 skipped (no building here)");

        let corner = Pos::new(3, 3);
        let notice = apply_rect(&mut s, Tool::CancelHarvest, corner, corner, false, false, None);
        assert_eq!(notice, "cancel: 1 building mark, 0 skipped");
        assert_eq!(s.world.deconstruct_designations().collect::<Vec<_>>(), [b]);
    }
}
