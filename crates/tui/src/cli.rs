//! The command line:
//!
//! ```text
//! fn_tui SEED NATION [--frame [--size WxH] [--ticks N] [--ansi]] [--ascii] [--color MODE]
//! ```

use fn_sim::Nation;

use crate::color::ColorDepth;

pub const USAGE: &str = "\
usage: fn_tui SEED NATION [options]
  SEED     the map seed (an integer)
  NATION   air, water, earth or fire

options:
  --frame        print one frame to stdout and exit (no terminal needed)
  --size WxH     the frame's size in cells, with --frame (default 120x40)
  --ticks N      run N ticks before the frame, with --frame (default 0)
  --ansi         keep the colour escapes in the frame, with --frame
  --ascii        draw with the ASCII glyph set
  --color MODE   truecolor, 256, 16 or never (default: from NO_COLOR, COLORTERM,
                 WT_SESSION and TERM)

Without --frame this starts the game in the terminal: arrows or WASD move the cursor
(Shift: 10 tiles), Space pauses, + and - change speed, . steps a tick, q quits.";

pub const DEFAULT_SIZE: (u16, u16) = (120, 40);

#[derive(Clone, Debug, PartialEq)]
pub struct Args {
    pub seed: u64,
    pub nation: Nation,
    pub frame: bool,
    pub size: (u16, u16),
    pub ticks: u32,
    pub ascii: bool,
    pub ansi: bool,
    pub color: Option<ColorDepth>,
}

pub fn parse_nation(s: &str) -> Option<Nation> {
    Nation::ALL.into_iter().find(|n| n.as_str().eq_ignore_ascii_case(s))
}

fn parse_size(s: &str) -> Option<(u16, u16)> {
    let (w, h) = s.split_once(['x', 'X'])?;
    let (w, h) = (w.parse().ok()?, h.parse().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

/// Parses the arguments after the program name. The error is a message for the user.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut args = args.into_iter();
    let (mut seed, mut nation) = (None, None);
    let mut out = Args {
        seed: 0,
        nation: Nation::Earth,
        frame: false,
        size: DEFAULT_SIZE,
        ticks: 0,
        ascii: false,
        ansi: false,
        color: None,
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
                out.size =
                    parse_size(&v).ok_or(format!("--size wants WxH, such as 120x40, got {v:?}"))?;
            }
            "--ticks" => {
                let v = value("--ticks")?;
                out.ticks =
                    v.parse().map_err(|_| format!("--ticks wants a whole number, got {v:?}"))?;
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
    out.seed = seed.ok_or("missing SEED")?;
    out.nation = nation.ok_or("missing NATION")?;
    for (flag, given) in
        [("--size", out.size != DEFAULT_SIZE), ("--ticks", out.ticks > 0), ("--ansi", out.ansi)]
    {
        if given && !out.frame {
            return Err(format!("{flag} only means something with --frame"));
        }
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
        assert_eq!(a.seed, 7);
        assert_eq!(a.nation, Nation::Earth);
        assert!(a.frame && a.ascii && a.ansi);
        assert_eq!((a.size, a.ticks), ((100, 30), 600));
        assert_eq!(a.color, Some(ColorDepth::Indexed256));
    }

    #[test]
    fn the_equals_form_and_the_defaults() {
        let a = parse_str("--frame 3 Fire --size=80x24").unwrap();
        assert_eq!((a.seed, a.nation, a.size), (3, Nation::Fire, (80, 24)));
        let a = parse_str("3 air").unwrap();
        assert!(!a.frame && !a.ascii);
        assert_eq!((a.size, a.ticks, a.color), (DEFAULT_SIZE, 0, None));
    }

    #[test]
    fn mistakes_say_what_is_wrong() {
        for (args, want) in [
            ("", "missing SEED"),
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
