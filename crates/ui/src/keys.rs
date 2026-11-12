//! The key table (docs/design/tui.md, "Hints"): every key the game answers to, written once.
//!
//! Each [`Entry`] says in which [`Mode`]s it works, which [`Action`]s it stands for, how its keys
//! are labelled ("Esc or T"), and a short verb ("close"). Three things are built from this one
//! table, so they can't drift apart:
//!
//! * the **hint bar**, the status line's keys: [`hints`] gives one mode's items in order, and
//!   [`fit`] says how many of them fit a width (whole items off the end, never cut mid-word);
//! * the **key help** screen and the agent's `help` rows ([`help_rows`]), which add each entry's
//!   longer wording ([`Help`]);
//! * the key paragraph of `fn_tui --help` ([`help_rows`] again).
//!
//! The table is frontend-neutral: it names keys as text and what they do as [`Action`], and the
//! terminal frontend maps its own key codes onto the same [`Action`]s (`fn_tui::input`), with a
//! test that every key it handles is in this table. A new binding is one row here.
//!
//! The wording rule: a key label, then a verb ("Esc or T close", "Enter set as target"). Lists
//! say "Up/Down", grids say "Arrows". Hints join with ` · ` (the frontend picks the glyph).

use std::cell::Cell;
use std::sync::LazyLock;

use crate::Tool;

/// What a key asks for, as the frontend that read it passes it on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Move the cursor `(dx, dy)` tiles; each of `dx` and `dy` is -1, 0 or 1, times `tiles`.
    Move {
        dx: i32,
        dy: i32,
        tiles: i32,
    },
    /// Jump the view a quarter screen (`dx`, `dy` in -1..=1) and carry the cursor.
    Jump {
        dx: i32,
        dy: i32,
    },
    /// `H` `X` `E` `P` `B` `U` or `C`: switch to a designation tool (dropping the corner).
    PickTool(Tool),
    /// `/`: opens the list of the designation tools in the panel's tool section (#664), or
    /// closes it again.
    ShowTools,
    /// `O`: the active tool's toggle. In Harvest it flips the next rectangle between a one-off
    /// designation and a standing order (#81, #277, stands in for the client's held Shift, which
    /// a terminal can't reliably report); in Cancel it flips whether it also takes build sites
    /// and stockpiles (#335). No effect in any other tool. Each tool's toggle is its own table
    /// row (#781), so the hint bar names the one that is active.
    ToggleRepeat,
    /// `R`: in Select mode with the cursor on a building that has another recipe (the Workshop),
    /// switches it to the next one (`Command::SetRecipe`, #712).
    NextRecipe,
    /// `Enter`: in Select mode, select the unit under the cursor (again: the next one there); in
    /// a tool, set a corner or apply the rectangle.
    Confirm,
    /// `Esc`: close the screen; in a tool, drop the corner or with none leave the tool; in Select
    /// mode, close the selection bar.
    Cancel,
    /// `Tab` and Shift+`Tab`: the next and the previous unit, and the cursor goes to it.
    NextUnit,
    PrevUnit,
    /// `F`: the cursor tracks the selected unit.
    Follow,
    /// `T`: the research screen.
    ShowResearch,
    /// `L` and `?`: the message log and the key help.
    ShowLog,
    ShowHelp,
    /// `G`: the Endeavor screen (#242).
    ShowEndeavor,
    /// `N`: the Nation screen (#308): every settler grouped by activity, every store and the
    /// nation's traits.
    ShowNation,
    /// `I`: the abilities screen (#719): every bending ability the settlement has or may yet
    /// unlock, and `Enter` there picks one for the Bend tool. Shown in the hint bar only.
    ShowAbilities,
    /// `M`: the Traders screen (#438): the caravan camped now, its deals and its request.
    ShowTraders,
    /// `F5`: save the game to the quicksave slot.
    QuickSave,
    /// `F9`: replace the game with the quicksave, after asking in a centred box.
    AskQuickLoad,
    /// `F6`: the saves screen (named slots).
    ShowSaves,
    TogglePause,
    Faster,
    Slower,
    /// One tick, while paused.
    Step,
    /// Quit at once (Ctrl+C: raw mode turns the signal into a key).
    QuitNow,
}

impl Action {
    /// Whether `other` is the same kind of action: the same variant, whatever its numbers (a
    /// `Move` is a `Move` in any direction), but a tool key is that tool's own.
    pub fn same_kind(self, other: Action) -> bool {
        match (self, other) {
            (Action::PickTool(a), Action::PickTool(b)) => a == b,
            (a, b) => std::mem::discriminant(&a) == std::mem::discriminant(&b),
        }
    }
}

/// Where the key help lists a binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Cursor,
    Units,
    Tools,
    Time,
    Game,
    /// The keys that mean something else once a designation tool is up.
    InTool,
}

impl Group {
    pub const ALL: [Group; 6] =
        [Self::Cursor, Self::Units, Self::Tools, Self::Time, Self::Game, Self::InTool];

    pub fn title(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor and view",
            Self::Units => "Settlers",
            Self::Tools => "Designating",
            Self::Time => "Time",
            Self::Game => "Screens and game",
            Self::InTool => "In a tool",
        }
    }
}

/// Where a key works: the modes the hint bar names, and two for keys the hint bar never lists
/// (they are in the key help, `?`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The map, no tool, nothing selected.
    Select,
    /// The map, no tool, a settler selected (its bar is up).
    Selected,
    /// A designation tool, no corner set yet.
    Tool,
    /// A designation tool with a corner set.
    ToolCorner,
    Research,
    /// The CHOOSE A SCHOOL box over the research screen (#729).
    ChooseSchool,
    Endeavor,
    Nation,
    /// The abilities screen (`I`, #719).
    Abilities,
    /// The Traders screen (`M`, #438).
    Traders,
    Log,
    /// The message log in the `--watch` spectator (#774): the studio keeps `1`-`3` and `0` for
    /// its panes, so the filters are on `Tab`, and `Esc` is refused (`L` closes it).
    WatchLog,
    Help,
    Saves,
    /// Typing a name for a new save, on the Saves screen.
    SaveName,
    /// The tool list in the panel (`/`, #664).
    Tools,
    /// The Build picker (`U`).
    Build,
    /// The game menu (`Esc` with nothing else to close).
    Menu,
    /// The menu's Switch nation… list.
    MenuNations,
    /// A question that can lose progress, asked in a centred box (new map, switch nation, quit,
    /// load, save over a slot).
    Ask,
    /// Leaving the embark screen: back to the title screen, out of the game, or stay.
    Exit,
    /// The title screen's menu.
    Title,
    /// The title screen's nation picker, the seed step and the save list.
    TitleNation,
    TitleSeed,
    TitleLoad,
    /// The embark screen at the overview, at a middle zoom and at the closest (site) zoom: what
    /// `Enter` and `Esc` do differs.
    Embark,
    EmbarkZoom,
    EmbarkSite,
    /// The embark screen's question, "settle here?", asked on the hint bar.
    Settle,
    /// Every map mode (Select, Selected and a tool). Key help only.
    Map,
    /// Every mode at once: the clock, the screens, prompts. Key help only.
    Anywhere,
}

impl Mode {
    /// The mode's name as the hint bar's badge says it. A tool is named by the tool: see
    /// [`mode_name`].
    pub fn name(self) -> &'static str {
        match self {
            Self::Select | Self::Selected | Self::Map => "SELECT",
            Self::Tool | Self::ToolCorner => "TOOL",
            Self::Research => "RESEARCH",
            Self::ChooseSchool => "CHOOSE A SCHOOL",
            Self::Endeavor => "ENDEAVOR",
            Self::Nation => "NATION",
            Self::Abilities => "ABILITIES",
            Self::Traders => "TRADERS",
            Self::Log | Self::WatchLog => "LOG",
            Self::Help => "KEYS",
            Self::Saves => "SAVES",
            Self::SaveName => "NAME",
            Self::Tools => "TOOLS",
            Self::Build => "BUILD",
            Self::Menu => "MENU",
            Self::MenuNations => "SWITCH",
            Self::Ask => "CONFIRM",
            Self::Exit => "EXIT",
            Self::Title => "TITLE",
            Self::TitleNation => "NATION",
            Self::TitleSeed => "SEED",
            Self::TitleLoad => "LOAD",
            Self::Embark | Self::EmbarkZoom | Self::EmbarkSite => "EMBARK",
            Self::Settle => "SETTLE?",
            Self::Anywhere => "ANY",
        }
    }
}

/// The badge's text for `mode`: a tool mode is named by `tool` ("HARVEST"), the rest by
/// [`Mode::name`].
pub fn mode_name(mode: Mode, tool: Tool) -> String {
    match mode {
        Mode::Tool | Mode::ToolCorner => tool.name().to_uppercase(),
        other => other.name().to_owned(),
    }
}

/// The long wording a binding has in the key help, and where it is listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Help {
    pub group: Group,
    pub what: String,
}

/// One row of the table: some keys, what they do, and where.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The modes whose hint bar shows it, in the table's order.
    pub modes: &'static [Mode],
    /// The actions its keys stand for (empty for a key only a screen reads: `N` on Saves).
    pub actions: Vec<Action>,
    /// The key label: "Enter", "Up/Down", "Esc or T".
    pub keys: String,
    /// The short verb: "close", "set as target".
    pub verb: &'static str,
    /// Its key help row, if it has one.
    pub help: Option<Help>,
    /// The one tool whose hint bar shows it (a tool's own toggle, #781); `None` for a key that
    /// works the same whichever tool is up. The key help lists every row, tool or not.
    pub tool: Option<Tool>,
}

/// One item of a hint bar: "Esc or T close".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    pub keys: String,
    pub verb: &'static str,
}

impl Hint {
    pub fn text(&self) -> String {
        format!("{} {}", self.keys, self.verb)
    }

    /// Its width in characters.
    pub fn width(&self) -> usize {
        self.keys.chars().count() + 1 + self.verb.chars().count()
    }
}

/// One line of the key help: the keys, and what they do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpRow {
    pub group: Group,
    pub keys: String,
    pub what: String,
}

const MAP: &[Mode] = &[Mode::Select, Mode::Selected, Mode::Tool, Mode::ToolCorner];
const IN_SELECT: &[Mode] = &[Mode::Select, Mode::Selected];
const IN_TOOL: &[Mode] = &[Mode::Tool, Mode::ToolCorner];

/// Every key the game answers to.
pub fn table() -> &'static [Entry] {
    static TABLE: LazyLock<Vec<Entry>> = LazyLock::new(build);
    &TABLE
}

/// The tool letters as the hint bar names them, in [`Tool::ALL`] order: "H X E P B U C".
pub fn tool_keys() -> String {
    let letters: Vec<String> =
        Tool::ALL.iter().filter_map(|t| t.letter()).map(String::from).collect();
    letters.join(" ")
}

fn build() -> Vec<Entry> {
    let mut t: Vec<Entry> = Vec::new();
    // The tool the rows being added belong to (#781); `None` for the rest.
    let only = Cell::new(None);
    let mut add = |modes: &'static [Mode],
                   actions: &[Action],
                   keys: &str,
                   verb: &'static str,
                   help: Option<(Group, &str)>| {
        t.push(Entry {
            modes,
            actions: actions.to_vec(),
            keys: keys.to_owned(),
            verb,
            help: help.map(|(group, what)| Help { group, what: what.to_owned() }),
            tool: only.get(),
        });
    };
    let step = Action::Move { dx: 0, dy: -1, tiles: 1 };
    let jump = Action::Jump { dx: 0, dy: -1 };
    let all_tools: Vec<Action> =
        Tool::ALL.iter().filter(|t| t.letter().is_some()).map(|&t| Action::PickTool(t)).collect();
    let tools = tool_keys();

    // The map's modes, one after another. Every one starts with the arrows.
    add(MAP, &[step], "Arrows", "move", None);
    add(&[Mode::Select], &[Action::Confirm], "Enter", "select", None);
    add(&[Mode::Select], &[Action::NextUnit], "Tab", "next unit", None);
    add(&[Mode::Select], &[Action::Cancel], "Esc", "menu", None);
    add(IN_SELECT, &[Action::ShowTools], "/", "tools", None);
    // The abilities screen (#719): a hint-bar item only, no key help row (`B` still picks the
    // Bend tool directly, and the screen is where Bend's abilities are read and chosen).
    add(IN_SELECT, &[Action::ShowAbilities], "I", "abilities", None);
    add(&[Mode::Selected], &[Action::Confirm], "Enter", "next unit here", None);
    add(&[Mode::Selected], &[Action::NextUnit], "Tab", "next unit", None);
    add(&[Mode::Selected], &[Action::Follow], "F", "follow", None);
    add(&[Mode::Selected], &[Action::Cancel], "Esc", "close", None);
    add(&[Mode::Tool], &[Action::Confirm], "Enter", "set corner", None);
    add(&[Mode::Tool], &[Action::Cancel], "Esc", "leave tool", None);
    add(&[Mode::ToolCorner], &[Action::Confirm], "Enter", "apply", None);
    add(&[Mode::ToolCorner], &[Action::Cancel], "Esc", "drop corner", None);

    // The screens, the Build picker and the menu.
    add(&[Mode::Research], &[step], "Arrows", "choose", None);
    add(&[Mode::Research], &[Action::Confirm], "Enter", "set as target", None);
    add(&[Mode::Research], &[Action::Cancel, Action::ShowResearch], "Esc or T", "close", None);
    add(&[Mode::Research], &[Action::PickTool(Tool::CancelHarvest)], "X", "clear target", None);
    add(&[Mode::ChooseSchool], &[Action::Confirm], "Enter", "choose", None);
    add(&[Mode::ChooseSchool], &[Action::Cancel], "Esc", "back", None);

    add(&[Mode::Nation], &[step], "Up/Down", "choose", None);
    add(&[Mode::Nation], &[Action::Confirm], "Enter", "select", None);
    add(&[Mode::Nation], &[Action::Cancel, Action::ShowNation], "Esc or N", "close", None);
    add(&[Mode::Nation], &[jump], "PgUp/PgDn", "page", None);

    add(&[Mode::Abilities], &[step], "Up/Down", "choose", None);
    add(&[Mode::Abilities], &[Action::Confirm], "Enter", "bend", None);
    add(&[Mode::Abilities], &[Action::Cancel, Action::ShowAbilities], "Esc or I", "close", None);
    add(&[Mode::Abilities], &[jump], "PgUp/PgDn", "page", None);

    add(&[Mode::Traders], &[step], "Up/Down", "choose", None);
    add(&[Mode::Traders], &[Action::Confirm], "Enter", "take", None);
    add(&[Mode::Traders], &[Action::PickTool(Tool::CancelHarvest)], "X", "cancel", None);
    add(&[Mode::Traders], &[Action::Cancel, Action::ShowTraders], "Esc or M", "close", None);

    add(&[Mode::Endeavor], &[step], "Up/Down", "scroll", None);
    add(&[Mode::Endeavor], &[Action::Confirm], "Enter", "place", None);
    add(&[Mode::Endeavor], &[Action::Cancel, Action::ShowEndeavor], "Esc or G", "close", None);
    add(&[Mode::Endeavor], &[Action::PickTool(Tool::CancelHarvest)], "X", "withdraw", None);
    add(&[Mode::Endeavor], &[jump], "PgUp/PgDn", "page", None);

    add(&[Mode::Log], &[step], "Up/Down", "choose", None);
    add(&[Mode::Log], &[Action::Confirm], "Enter", "jump there", None);
    add(&[Mode::Log], &[], "Right", "open", None);
    add(&[Mode::Log], &[], "1-5", "filter", None);
    add(&[Mode::Log], &[jump], "PgUp/PgDn", "page", None);
    add(&[Mode::Log], &[Action::Cancel, Action::ShowLog], "Esc or L", "close", None);

    // The same log in the watch spectator: `Tab` for the filters, `L` the only way out.
    add(&[Mode::WatchLog], &[step], "Up/Down", "choose", None);
    add(&[Mode::WatchLog], &[Action::Confirm], "Enter", "jump there", None);
    add(&[Mode::WatchLog], &[], "Right", "open", None);
    add(&[Mode::WatchLog], &[Action::NextUnit, Action::PrevUnit], "Tab", "filter", None);
    add(&[Mode::WatchLog], &[jump], "PgUp/PgDn", "page", None);
    add(&[Mode::WatchLog], &[Action::ShowLog], "L", "close", None);

    add(&[Mode::Help], &[step], "Up/Down", "scroll", None);
    add(&[Mode::Help], &[Action::Cancel, Action::ShowHelp], "Esc or ?", "close", None);
    add(&[Mode::Help], &[jump], "PgUp/PgDn", "page", None);
    add(&[Mode::Help], &[Action::NextUnit, Action::PrevUnit], "Tab", "switch tab", None);

    add(&[Mode::Saves], &[step], "Up/Down", "choose", None);
    add(&[Mode::Saves], &[Action::Confirm], "Enter", "load", None);
    add(&[Mode::Saves], &[Action::Cancel, Action::ShowSaves], "Esc or F6", "close", None);
    add(&[Mode::Saves], &[], "Shift+S", "save over", None);
    add(&[Mode::Saves], &[], "N", "new", None);

    // Naming a new save: every key types, so only these three do anything else.
    add(&[Mode::SaveName], &[], "Type", "a name", None);
    add(&[Mode::SaveName], &[], "Enter", "save", None);
    add(&[Mode::SaveName], &[], "Esc", "cancel", None);

    // The tool list in the panel (#664): the arrows choose, Enter or the tool's own letter picks.
    add(&[Mode::Tools], &[step], "Up/Down", "choose", None);
    add(&[Mode::Tools], &[Action::Confirm], "Enter", "pick", None);
    add(&[Mode::Tools], &[Action::Cancel, Action::ShowTools], "Esc or /", "close", None);
    add(&[Mode::Tools], &all_tools, &tools, "by letter", None);

    add(&[Mode::Build], &[], "Up/Down", "choose", None);
    add(&[Mode::Build], &[], "Enter", "build", None);
    add(&[Mode::Build], &[], "Esc or U", "close", None);

    add(&[Mode::Menu], &[], "Up/Down", "choose", None);
    add(&[Mode::Menu], &[], "Enter", "select", None);
    add(&[Mode::Menu], &[], "Esc", "resume", None);

    add(&[Mode::MenuNations], &[], "Up/Down", "choose", None);
    add(&[Mode::MenuNations], &[], "Enter", "choose", None);
    add(&[Mode::MenuNations], &[], "Esc", "back", None);

    // The confirm box (`Esc` is always the safe answer, the cursor starts on it, and `y` alone
    // does nothing).
    add(&[Mode::Ask], &[], "Up/Down", "choose", None);
    add(&[Mode::Ask], &[], "Enter", "answer", None);
    add(&[Mode::Ask], &[], "Esc", "cancel", None);

    // Quitting from the embark screen, which has no game to save: the same two ways out as the
    // game's Quit… box.
    add(&[Mode::Exit], &[], "T", "title screen", None);
    add(&[Mode::Exit], &[], "Q or Enter", "exit", None);
    add(&[Mode::Exit], &[], "Esc", "stay", None);

    // The title screen, a page at a time. It has no key help, so no `? keys` tail.
    add(&[Mode::Title], &[], "Up/Down", "choose", None);
    add(&[Mode::Title], &[], "Enter", "select", None);
    add(&[Mode::TitleNation], &[], "Arrows or 1-4", "choose", None);
    add(&[Mode::TitleNation], &[], "Enter", "confirm", None);
    add(&[Mode::TitleNation], &[], "Esc", "back", None);
    add(&[Mode::TitleSeed], &[], "Up/Down", "pick row", None);
    add(&[Mode::TitleSeed], &[], "Left/Right", "change", None);
    add(&[Mode::TitleSeed], &[], "Enter", "embark", None);
    add(&[Mode::TitleSeed], &[], "Esc", "back", None);
    add(&[Mode::TitleLoad], &[], "Up/Down", "choose", None);
    add(&[Mode::TitleLoad], &[], "Enter", "load", None);
    add(&[Mode::TitleLoad], &[], "Esc", "back", None);

    // The embark screen, by zoom, and its settle question.
    const EMBARK: &[Mode] = &[Mode::Embark, Mode::EmbarkZoom, Mode::EmbarkSite];
    add(EMBARK, &[], "Arrows", "move", None);
    add(&[Mode::Embark, Mode::EmbarkZoom], &[], "Enter", "zoom in", None);
    add(&[Mode::EmbarkSite], &[], "Enter or E", "settle", None);
    add(&[Mode::EmbarkZoom, Mode::EmbarkSite], &[], "-", "zoom out", None);
    add(&[Mode::Embark, Mode::EmbarkZoom], &[], "E", "settle", None);
    add(EMBARK, &[], "R", "new seed", None);
    add(&[Mode::Embark], &[], "Esc", "back", None);
    add(&[Mode::Settle], &[], "Y", "settle here", None);
    add(&[Mode::Settle], &[], "Any other key", "stay", None);

    // The key help's own rows, in its order: one row per key, short enough for half a screen
    // (anything that needs a paragraph is left to the screen it is about). Their keys are the
    // whole spelling (WASD too).
    let cursor = Some(Group::Cursor);
    let units = Some(Group::Units);
    let in_tool = Some(Group::InTool);
    let help = |g: Option<Group>, what| g.map(|g| (g, what));
    add(&[Mode::Map], &[step], "Arrows or WASD", "move", help(cursor, "Move 1 tile (Shift: 10)"));
    add(&[Mode::Map], &[jump], "Ctrl+Arrows", "jump", help(cursor, "Jump a quarter screen"));
    add(&[Mode::Map], &[jump], "PgUp/PgDn", "jump", help(cursor, "Jump a quarter screen"));
    add(
        &[Mode::Map],
        &[Action::Confirm],
        "Enter",
        "select",
        help(units, "Select what is under the cursor"),
    );
    add(&[Mode::Map], &[Action::NextUnit], "Tab", "next unit", help(units, "Next settler"));
    add(
        &[Mode::Map],
        &[Action::PrevUnit],
        "Shift+Tab",
        "previous unit",
        help(units, "Previous settler"),
    );
    add(&[Mode::Map], &[Action::Follow], "F", "follow", help(units, "Follow the selected settler"));
    add(&[Mode::Map], &[Action::Cancel], "Esc", "close", help(units, "Deselect, or open the menu"));
    add(
        &[Mode::Map],
        &[Action::ShowTools],
        "/",
        "tools",
        help(Some(Group::Tools), "List the tools in the panel"),
    );
    for tool in Tool::ALL {
        // Select is Esc; every other tool has a letter.
        if tool.letter().is_none() {
            continue;
        }
        add(
            &[Mode::Map],
            &[Action::PickTool(tool)],
            tool.key(),
            "tool",
            Some((Group::Tools, tool_help(tool))),
        );
    }
    add(
        &[Mode::Map],
        &[Action::Confirm],
        "Enter",
        "set corner",
        help(in_tool, "Set a corner, then apply"),
    );
    add(
        &[Mode::Map],
        &[Action::Cancel],
        "Esc",
        "drop corner",
        help(in_tool, "Drop the corner, then leave"),
    );
    // A tool's toggles (#781): one row each, in the hint bar only while that tool is up. They
    // are the Tool section's settings (`toolpanel`), as short verbs.
    only.set(Some(Tool::Harvest));
    add(IN_TOOL, &[Action::ToggleRepeat], "O", "repeat", help(in_tool, "Standing order (Harvest)"));
    only.set(Some(Tool::CancelHarvest));
    add(
        IN_TOOL,
        &[Action::ToggleRepeat],
        "O",
        "teardown",
        help(in_tool, "Also sites, stockpiles (Cancel)"),
    );
    // The Bend tool's own way to the abilities screen (#719), hint bar only like the toggles.
    only.set(Some(Tool::Bend));
    add(IN_TOOL, &[Action::ShowAbilities], "I", "abilities", None);
    only.set(None);
    // Select mode's `R` on a Workshop (#712): in no hint bar, only in the key help.
    add(
        &[],
        &[Action::NextRecipe],
        "R",
        "recipe",
        help(units, "Switch a Workshop's recipe (Select)"),
    );
    // Select mode's `O` on a chute (#683): in no hint bar, only in the key help.
    add(&[], &[], "O", "link", help(in_tool, "Move a chute foot or partner (Select)"));

    let time = Some(Group::Time);
    add(&[Mode::Anywhere], &[Action::TogglePause], "Space", "pause", help(time, "Pause or run"));
    add(&[Mode::Anywhere], &[Action::Faster], "+ or =", "faster", help(time, "Faster"));
    add(&[Mode::Anywhere], &[Action::Slower], "- or _", "slower", help(time, "Slower"));
    add(&[Mode::Anywhere], &[Action::Step], ".", "one tick", help(time, "One tick, while paused"));

    let game = Some(Group::Game);
    add(&[Mode::Anywhere], &[Action::ShowNation], "N", "nation", help(game, "Nation"));
    add(&[Mode::Anywhere], &[Action::ShowResearch], "T", "research", help(game, "Research"));
    add(&[Mode::Anywhere], &[Action::ShowEndeavor], "G", "endeavor", help(game, "Endeavor"));
    add(&[Mode::Anywhere], &[Action::ShowTraders], "M", "traders", help(game, "Traders"));
    add(&[Mode::Anywhere], &[Action::ShowLog], "L", "log", help(game, "Message log"));
    // The log's own filters: in no hint bar here (`Mode::Log` and `Mode::WatchLog` have them),
    // only in the key help.
    add(&[], &[], "1-5", "filter", help(game, "Log: one kind of line (0: all)"));
    add(&[], &[], "Tab", "filter", help(game, "Log in --watch: next kind"));
    add(&[Mode::Anywhere], &[Action::ShowHelp], "?", "keys", help(game, "Key help and legend"));
    add(&[Mode::Anywhere], &[Action::QuickSave], "F5", "quicksave", help(game, "Quicksave"));
    add(
        &[Mode::Anywhere],
        &[Action::AskQuickLoad],
        "F9",
        "quickload",
        help(game, "Quickload (asks first)"),
    );
    add(&[Mode::Anywhere], &[Action::ShowSaves], "F6", "saves", help(game, "Saves"));
    add(
        &[Mode::Anywhere],
        &[Action::Cancel],
        "Esc",
        "close",
        help(game, "Game menu: save, load, new map, quit"),
    );
    add(
        &[Mode::Anywhere],
        &[Action::QuitNow],
        "Ctrl+C",
        "quit now",
        help(game, "Quit at once, without asking"),
    );
    t
}

/// What a tool's key does, as the key help words it: a few words, one row.
fn tool_help(tool: Tool) -> &'static str {
    match tool {
        Tool::Select | Tool::Gate => "",
        Tool::Harvest => "Harvest: chop, mine, gather",
        Tool::CancelHarvest => "Cancel a mark",
        Tool::Clear | Tool::Deconstruct => "Clear the ground",
        Tool::Stockpile => "Stockpile",
        Tool::Bend => "Bend: channel the nation's ability",
        Tool::Build => "Build",
        Tool::Sow => "Sow a farm plot",
    }
}

/// The hint bar's items for `mode`, in the table's order, without any tool's own rows
/// ([`hints_for`]). Leaves out the tail ([`tail`]).
pub fn hints(mode: Mode) -> Vec<Hint> {
    hints_for(mode, Tool::Select)
}

/// [`hints`] with what `tool`, the active tool, adds: its own toggles (#781). Only a tool mode
/// has any.
pub fn hints_for(mode: Mode, tool: Tool) -> Vec<Hint> {
    table()
        .iter()
        .filter(|e| e.modes.contains(&mode) && e.tool.is_none_or(|t| t == tool))
        .map(|e| Hint { keys: e.keys.clone(), verb: e.verb })
        .collect()
}

/// The hint bar's fixed tail, "? keys": the one item every mode ends with.
pub fn tail() -> Hint {
    let e = table()
        .iter()
        .find(|e| e.actions == [Action::ShowHelp] && e.modes == [Mode::Anywhere])
        .expect("the table has the key help's own row");
    Hint { keys: e.keys.clone(), verb: e.verb }
}

/// How many of `items` fit in `room` cells when joined by a separator `sep` cells wide: the
/// longest run from the start, so whole items go off the end and none is ever cut.
pub fn fit(items: &[Hint], room: usize, sep: usize) -> usize {
    let mut used = 0;
    for (i, item) in items.iter().enumerate() {
        used += item.width() + if i > 0 { sep } else { 0 };
        if used > room {
            return i;
        }
    }
    items.len()
}

/// The key help, one row per entry that has a [`Help`], in the table's order.
pub fn help_rows() -> Vec<HelpRow> {
    table()
        .iter()
        .filter_map(|e| {
            e.help.as_ref().map(|h| HelpRow {
                group: h.group,
                keys: e.keys.clone(),
                what: h.what.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(mode: Mode) -> String {
        let items: Vec<String> = hints(mode).iter().map(Hint::text).collect();
        items.join(" · ")
    }

    #[test]
    fn every_hint_is_a_key_label_then_a_verb() {
        for e in table() {
            assert!(!e.keys.is_empty() && !e.verb.is_empty(), "{e:?}");
            assert!(!e.keys.contains("  ") && !e.verb.contains("  "), "{e:?}");
            assert!(!e.verb.chars().next().unwrap().is_uppercase(), "{e:?}: verbs are lower case");
        }
    }

    #[test]
    fn lists_say_up_down_and_grids_say_arrows() {
        assert!(line(Mode::Research).starts_with("Arrows choose"));
        for mode in [
            Mode::Nation,
            Mode::Abilities,
            Mode::Traders,
            Mode::Log,
            Mode::Help,
            Mode::Saves,
            Mode::Build,
            Mode::Menu,
            Mode::MenuNations,
            Mode::Ask,
            Mode::Title,
            Mode::TitleSeed,
            Mode::TitleLoad,
        ] {
            assert!(line(mode).starts_with("Up/Down "), "{mode:?}: {}", line(mode));
        }
        assert!(line(Mode::Embark).starts_with("Arrows move"));
        assert!(line(Mode::Select).starts_with("Arrows move"));
    }

    #[test]
    fn the_research_hints_read_as_the_issue_words_them() {
        assert_eq!(
            line(Mode::Research),
            "Arrows choose · Enter set as target · Esc or T close · X clear target"
        );
    }

    /// Select mode has one key for the tools, `/`, not a run of their letters (#664); the list
    /// it opens names every letter, and the letters still work from the map.
    #[test]
    fn select_names_the_one_tools_key_and_the_list_names_every_letter() {
        for mode in [Mode::Select, Mode::Selected] {
            let hints = hints(mode);
            let tools = hints.iter().find(|h| h.verb == "tools").expect("a tools item");
            assert_eq!(tools.keys, "/");
            assert!(hints.iter().all(|h| h.keys.split(' ').filter(|k| k.len() == 1).count() < 2));
        }
        let by_letter = hints(Mode::Tools).into_iter().find(|h| h.verb == "by letter").unwrap();
        for tool in Tool::ALL {
            if let Some(letter) = tool.letter() {
                assert!(by_letter.keys.split(' ').any(|k| k == letter.to_string()), "{letter}");
            }
        }
        assert_eq!(
            line(Mode::Tools),
            "Up/Down choose · Enter pick · Esc or / close · H X E P B U C by letter"
        );
    }

    /// A tool's toggle is a row of its own (#781): in the tool modes' hints for that tool only,
    /// and one `O` row a tool in the key help, plus Select's `O` on a chute (#683).
    #[test]
    fn a_tools_toggles_are_its_own_rows() {
        let text = |mode, tool| {
            let items: Vec<String> = hints_for(mode, tool).iter().map(Hint::text).collect();
            items.join(" · ")
        };
        for mode in [Mode::Tool, Mode::ToolCorner] {
            assert!(text(mode, Tool::Harvest).ends_with(" · O repeat"));
            assert!(text(mode, Tool::CancelHarvest).ends_with(" · O teardown"));
            for tool in [Tool::Clear, Tool::Stockpile, Tool::Bend, Tool::Build, Tool::Sow] {
                assert!(!text(mode, tool).contains("O "), "{tool:?}");
            }
            // The abilities key is the Bend tool's, and every map mode without a tool names it
            // too (#719); it is in no key help row.
            assert!(text(mode, Tool::Bend).ends_with(" · I abilities"));
            assert!(!text(mode, Tool::Harvest).contains("abilities"));
        }
        assert!(text(Mode::Select, Tool::Select).contains("I abilities"));
        assert!(help_rows().iter().all(|r| r.keys != "I"));
        // The other modes never carry one, and `hints` is the tool-less list.
        assert!(!text(Mode::Select, Tool::Harvest).contains("O "));
        assert_eq!(line(Mode::Tool), "Arrows move · Enter set corner · Esc leave tool");
        let rows: Vec<_> = help_rows().into_iter().filter(|r| r.keys == "O").collect();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r.group == Group::InTool));
        assert!(rows[2].what.contains("chute"));
    }

    /// The title and embark screens' hints (#686), as their old footers worded them, in the
    /// table's `key verb` pattern.
    #[test]
    fn the_title_and_embark_screens_have_their_own_hints() {
        assert_eq!(line(Mode::Title), "Up/Down choose · Enter select");
        assert_eq!(line(Mode::TitleNation), "Arrows or 1-4 choose · Enter confirm · Esc back");
        assert_eq!(
            line(Mode::TitleSeed),
            "Up/Down pick row · Left/Right change · Enter embark · Esc back"
        );
        assert_eq!(line(Mode::TitleLoad), "Up/Down choose · Enter load · Esc back");
        assert_eq!(
            line(Mode::Embark),
            "Arrows move · Enter zoom in · E settle · R new seed · Esc back"
        );
        assert_eq!(
            line(Mode::EmbarkZoom),
            "Arrows move · Enter zoom in · - zoom out · E settle · R new seed"
        );
        assert_eq!(
            line(Mode::EmbarkSite),
            "Arrows move · Enter or E settle · - zoom out · R new seed"
        );
        assert_eq!(line(Mode::Settle), "Y settle here · Any other key stay");
    }

    #[test]
    fn the_tail_is_the_key_help() {
        assert_eq!(tail().text(), "? keys");
    }

    #[test]
    fn fit_drops_whole_items_from_the_end() {
        let items = hints(Mode::Research);
        let all: usize = items.iter().map(Hint::width).sum::<usize>() + 3 * 3;
        assert_eq!(fit(&items, 200, 3), 4);
        assert_eq!(fit(&items, all, 3), 4);
        // One cell short of all four: the last goes, the rest stay whole.
        assert_eq!(fit(&items, all - 3 - 1, 3), 3);
        assert_eq!(fit(&items, items[0].width(), 3), 1);
        assert_eq!(fit(&items, items[0].width() - 1, 3), 0);
    }

    #[test]
    fn the_key_help_has_every_group_and_every_tool() {
        let rows = help_rows();
        for g in Group::ALL {
            assert!(rows.iter().any(|r| r.group == g), "{g:?} has no rows");
        }
        for tool in Tool::ALL {
            if tool.letter().is_some() {
                assert!(
                    rows.iter().any(|r| r.keys == tool.key() && r.what.starts_with(tool.name())),
                    "{} has no row",
                    tool.name()
                );
            }
        }
    }

    #[test]
    fn same_kind_ignores_numbers_but_not_tools() {
        let up = Action::Move { dx: 0, dy: -1, tiles: 1 };
        assert!(up.same_kind(Action::Move { dx: 1, dy: 0, tiles: 10 }));
        assert!(Action::Move { dx: 0, dy: 1, tiles: 1 }.same_kind(up));
        assert!(Action::PickTool(Tool::Bend).same_kind(Action::PickTool(Tool::Bend)));
        assert!(!Action::PickTool(Tool::Bend).same_kind(Action::PickTool(Tool::Sow)));
        assert!(!up.same_kind(Action::Confirm));
    }

    #[test]
    fn a_mode_name_is_the_tool_s_in_a_tool_mode() {
        assert_eq!(mode_name(Mode::Tool, Tool::Harvest), "HARVEST");
        assert_eq!(mode_name(Mode::ToolCorner, Tool::Build), "BUILD");
        assert_eq!(mode_name(Mode::Research, Tool::Harvest), "RESEARCH");
    }
}
