//! The command line:
//!
//! ```text
//! fn_tui [SEED NATION] [--frame [--size WxH] [--ticks N] [--keys SCRIPT] [--ansi]] [--ascii]
//!        [--color MODE] [--embark | --start X,Y] [--size small|medium|large] [--rough]
//! ```
//!
//! With no `SEED`/`NATION`, the game opens on the title screen (#280): New game, Continue, Load
//! and Quit. Giving both still skips straight to a game, as scripts, `--frame` and the tests
//! rely on.
//!
//! `--embark` and `--start` (#290, docs/design/embark.md) need `SEED NATION`: `--embark` opens
//! embark for them instead of starting the game, and `--start X,Y` founds a game at that site
//! straight away. `--size` is overloaded: `WxH` (such as `120x40`) is the frame's size, with
//! `--frame`; `small`, `medium` or `large` is the map size preset, which (with `--rough`) only
//! means something alongside `--embark` or `--start`.

use fn_sim::Nation;

use crate::color::ColorDepth;
use crate::keys::{self, Step};

pub const USAGE: &str = "\
usage: fn_tui [SEED NATION] [options]
  SEED     the map seed (an integer); with NATION, skips the title screen
  NATION   air, water, earth or fire

With neither, fn_tui opens on the title screen: New game (pick a nation and a seed), Continue
(the newest save), Load (any save) and Quit.

options:
  --frame        print one frame to stdout and exit (no terminal needed)
  --size WxH     the frame's size in cells, with --frame (default 120x40)
  --ticks N      run N ticks before the frame, with --frame (default 0)
  --keys SCRIPT  play a key script after those ticks, with --frame: tokens separated by
                 spaces, each a key (h, d, S), a named key (<enter> <esc> <space> <tab>
                 <up> <s-up> <c-up> <pgdn>), a repeat (<right*5>) or ticks (<tick 600>).
                 Example: 'h <right*5> <enter> <down*3> <enter> <tick 600>'
  --ansi        keep the colour escapes in the frame, with --frame
  --ascii        draw with the ASCII glyph set
  --color MODE   truecolor, 256, 16 or never (default: from NO_COLOR, COLORTERM,
                 WT_SESSION and TERM)
  --embark       open embark for SEED NATION instead of starting the game (works with --frame
                 and --keys)
  --start X,Y    start a founded game at that site, needs SEED NATION
  --size small|medium|large  the map size preset, with --embark or --start (default small)
  --rough        a rough survey, with --embark or --start

Without --frame this starts the game in the terminal: arrows or WASD move the cursor
(Shift: 10 tiles), Enter selects a unit (Tab cycles, F follows, Esc closes the bar), H X P B U
pick the harvest, cancel, stockpile, bend and build tools (U again cycles the building) and
Enter sets a rectangle's corners, Space pauses, + and - change speed, . steps a tick, T opens
the research tree, L the message log, ? the key help, R starts a new map, 1-4 switch nation
(each asks y/n first), F5 quicksaves, F9 quickloads (asks first), F6 opens the saves screen
(Enter loads, Shift+S saves over, N a new slot) and q quits. Saves go in FN_SAVE_DIR, else the
platform's data directory.";

pub const DEFAULT_SIZE: (u16, u16) = (120, 40);

#[derive(Clone, Debug, PartialEq)]
pub struct Args {
    /// `None` with neither `SEED` nor `NATION`: the title screen decides both.
    pub seed: Option<u64>,
    pub nation: Option<Nation>,
    pub frame: bool,
    pub size: (u16, u16),
    pub ticks: u32,
    /// The parsed `--keys` script, played after the ticks.
    pub keys: Vec<Step>,
    pub ascii: bool,
    pub ansi: bool,
    pub color: Option<ColorDepth>,
    /// `--embark` (#290): open embark for `SEED NATION` instead of starting the game.
    pub embark: bool,
    /// `--start X,Y`: found a game at that site straight away.
    pub start: Option<(i32, i32)>,
    /// `--size small|medium|large`: the map size preset's id, for `--embark` or `--start`.
    /// `None` is the default (small).
    pub map_size: Option<String>,
    /// `--rough`: a rough survey, for `--embark` or `--start`.
    pub rough: bool,
}

pub fn parse_nation(s: &str) -> Option<Nation> {
    Nation::ALL.into_iter().find(|n| n.as_str().eq_ignore_ascii_case(s))
}

fn parse_size(s: &str) -> Option<(u16, u16)> {
    let (w, h) = s.split_once(['x', 'X'])?;
    let (w, h) = (w.parse().ok()?, h.parse().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

/// `small`, `medium` or `large` (case-insensitive): a `data/embark.toml` map size preset's id.
fn parse_map_size_id(s: &str) -> Option<String> {
    ["small", "medium", "large"]
        .into_iter()
        .find(|id| id.eq_ignore_ascii_case(s))
        .map(str::to_owned)
}

/// `X,Y`: a tile position, for `--start`.
fn parse_pos(s: &str) -> Option<(i32, i32)> {
    let (x, y) = s.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// Parses the arguments after the program name. The error is a message for the user.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut args = args.into_iter();
    let (mut seed, mut nation) = (None, None);
    let mut keys_given = false;
    let mut out = Args {
        seed: None,
        nation: None,
        frame: false,
        size: DEFAULT_SIZE,
        ticks: 0,
        keys: Vec::new(),
        ascii: false,
        ansi: false,
        color: None,
        embark: false,
        start: None,
        map_size: None,
        rough: false,
    };
    while let Some(arg) = args.next() {
        // `--flag=value` and `--flag value` both work.
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (arg.clone(), None),
        };
        let mut value = |name: &str| {
            inline.clone().or_else(|| args.next()).ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "--frame" => out.frame = true,
            "--ascii" => out.ascii = true,
            "--ansi" => out.ansi = true,
            "--size" => {
                let v = value("--size")?;
                if let Some(wh) = parse_size(&v) {
                    out.size = wh;
                } else if let Some(id) = parse_map_size_id(&v) {
                    out.map_size = Some(id);
                } else {
                    return Err(format!(
                        "--size wants WxH (such as 120x40) or small/medium/large, got {v:?}"
                    ));
                }
            }
            "--embark" => out.embark = true,
            "--start" => {
                let v = value("--start")?;
                out.start = Some(
                    parse_pos(&v).ok_or(format!("--start wants X,Y, such as 50,40, got {v:?}"))?,
                );
            }
            "--rough" => out.rough = true,
            "--ticks" => {
                let v = value("--ticks")?;
                out.ticks =
                    v.parse().map_err(|_| format!("--ticks wants a whole number, got {v:?}"))?;
            }
            "--keys" => {
                let v = value("--keys")?;
                out.keys = keys::parse(&v).map_err(|e| format!("--keys: {e}"))?;
                keys_given = true;
            }
            "--color" => {
                let v = value("--color")?;
                out.color = Some(
                    ColorDepth::parse(&v)
                        .ok_or(format!("--color wants truecolor, 256, 16 or never, got {v:?}"))?,
                );
            }
            "-h" | "--help" => return Err(USAGE.to_owned()),
            f if f.starts_with('-') && f.len() > 1 => return Err(format!("unknown option {f}")),
            positional if seed.is_none() => {
                seed = Some(
                    positional
                        .parse()
                        .map_err(|_| format!("SEED must be a whole number, got {positional:?}"))?,
                );
            }
            positional if nation.is_none() => {
                nation = Some(parse_nation(positional).ok_or(format!(
                    "NATION must be air, water, earth or fire, got {positional:?}"
                ))?);
            }
            extra => return Err(format!("unexpected argument {extra:?}")),
        }
    }
    match (seed, nation) {
        // Neither given: the title screen decides both.
        (None, None) => {}
        (Some(s), Some(n)) => {
            out.seed = Some(s);
            out.nation = Some(n);
        }
        (Some(_), None) => return Err("missing NATION".to_owned()),
        (None, Some(_)) => return Err("missing SEED".to_owned()),
    }
    for (flag, given) in [
        ("--size", out.size != DEFAULT_SIZE),
        ("--ticks", out.ticks > 0),
        ("--keys", keys_given),
        ("--ansi", out.ansi),
    ] {
        if given && !out.frame {
            return Err(format!("{flag} only means something with --frame"));
        }
    }
    if out.embark && out.start.is_some() {
        return Err("--embark and --start can't both be given".to_owned());
    }
    if (out.embark || out.start.is_some() || out.rough || out.map_size.is_some())
        && (out.seed.is_none() || out.nation.is_none())
    {
        return Err("--embark, --start, --size (small/medium/large) and --rough need SEED NATION"
            .to_owned());
    }
    if (out.rough || out.map_size.is_some()) && !(out.embark || out.start.is_some()) {
        return Err("--size (small/medium/large) and --rough need --embark or --start".to_owned());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(s: &str) -> Result<Args, String> {
        parse(s.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn a_frame_with_every_option() {
        let a = parse_str("7 earth --frame --size 100x30 --ticks 600 --ascii --ansi --color 256")
            .unwrap();
        assert_eq!(a.seed, Some(7));
        assert_eq!(a.nation, Some(Nation::Earth));
        assert!(a.frame && a.ascii && a.ansi);
        assert_eq!((a.size, a.ticks), ((100, 30), 600));
        assert_eq!(a.color, Some(ColorDepth::Indexed256));
    }

    #[test]
    fn the_equals_form_and_the_defaults() {
        let a = parse_str("--frame 3 Fire --size=80x24").unwrap();
        assert_eq!((a.seed, a.nation, a.size), (Some(3), Some(Nation::Fire), (80, 24)));
        let a = parse_str("3 air").unwrap();
        assert!(!a.frame && !a.ascii);
        assert_eq!((a.size, a.ticks, a.color), (DEFAULT_SIZE, 0, None));
    }

    #[test]
    fn neither_seed_nor_nation_is_the_title_screen() {
        let a = parse_str("").unwrap();
        assert_eq!((a.seed, a.nation), (None, None));
        assert!(!a.frame);
        // Still valid with --frame: the title screen renders too (#280).
        let a = parse_str("--frame --size 100x30").unwrap();
        assert_eq!((a.seed, a.nation, a.size), (None, None, (100, 30)));
    }

    #[test]
    fn a_key_script_is_one_argument_and_is_parsed_up_front() {
        let args = ["7", "earth", "--frame", "--keys", "h <right*2> <tick 5>"].map(str::to_owned);
        let a = parse(args).unwrap();
        assert_eq!(a.keys.len(), 1 + 2 + 1);
        assert_eq!(a.keys[3], Step::Tick(5));
        let a = parse_str("7 earth --frame --keys=d").unwrap();
        assert_eq!(a.keys.len(), 1);
        assert!(parse_str("7 earth --frame").unwrap().keys.is_empty());
        let err = parse(["7", "earth", "--frame", "--keys", "oops"].map(str::to_owned));
        assert!(err.unwrap_err().starts_with("--keys: \"oops\""));
        assert!(parse_str("7 earth --keys d").unwrap_err().contains("only means something"));
    }

    /// `--size` is overloaded (#290): `WxH` is the frame's size, `small`/`medium`/`large` is
    /// the map preset, and only the frame form needs `--frame`.
    #[test]
    fn size_is_overloaded_between_the_frame_and_the_map_preset() {
        let a = parse_str("7 earth --embark --size medium --rough").unwrap();
        assert_eq!(a.map_size.as_deref(), Some("medium"));
        assert_eq!(a.size, DEFAULT_SIZE, "the frame size is untouched");
        assert!(a.embark && a.rough);

        let a = parse_str("7 earth --frame --size 100x30").unwrap();
        assert_eq!(a.size, (100, 30));
        assert_eq!(a.map_size, None);
    }

    #[test]
    fn embark_and_start_need_seed_and_nation_and_cant_both_be_given() {
        assert!(parse_str("--embark").unwrap_err().contains("need SEED NATION"));
        assert!(parse_str("--start 5,5").unwrap_err().contains("need SEED NATION"));
        assert!(
            parse_str("7 earth --rough").unwrap_err().contains("need --embark or --start"),
            "rough alone, with neither, is a mistake"
        );
        let err = parse_str("7 earth --embark --start 5,5").unwrap_err();
        assert!(err.contains("can't both be given"), "{err}");

        let a = parse_str("7 earth --embark").unwrap();
        assert!(a.embark && a.start.is_none());
        let a = parse_str("7 earth --start 50,40").unwrap();
        assert_eq!(a.start, Some((50, 40)));
    }

    #[test]
    fn mistakes_say_what_is_wrong() {
        for (args, want) in [
            ("7", "missing NATION"),
            ("x earth", "SEED"),
            ("7 mars", "NATION"),
            ("7 earth --bogus", "unknown option --bogus"),
            ("7 earth extra", "unexpected argument"),
            ("7 earth --frame --size 10", "WxH"),
            ("7 earth --frame --size 0x5", "WxH"),
            ("7 earth --frame --ticks", "needs a value"),
            ("7 earth --frame --ticks -1", "whole number"),
            ("7 earth --color rgb", "--color wants"),
            ("7 earth --size 80x24", "only means something with --frame"),
            ("7 earth --ticks 5", "only means something with --frame"),
            ("7 earth --ansi", "only means something with --frame"),
        ] {
            let err = parse_str(args).unwrap_err();
            assert!(err.contains(want), "{args:?}: {err}");
        }
    }
}
