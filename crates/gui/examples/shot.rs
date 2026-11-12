//! `cargo xtask gui-shot SEED NATION [--keys SCRIPT] [--size WxH] --out FILE.png` (#875): one
//! frame of the window, rendered by `egui_kittest`'s wgpu path and written as a PNG. It plays the
//! part `fn_tui --frame` plays for the terminal: something to look at, attached to a PR; nothing
//! compares it with anything.
//!
//! It needs the `shots` feature (the xtask passes it) and a wgpu adapter: a GPU, or Mesa's
//! lavapipe (`mesa-vulkan-drivers` on Debian and Ubuntu). Without one it exits 1 with a message.

use std::panic;
use std::path::PathBuf;
use std::process::ExitCode;

use eframe::egui;
use egui_kittest::Harness;
use fn_gui::app::GuiApp;
use fn_tui::shell::Shell;

const USAGE: &str = "usage: gui-shot SEED NATION [--keys SCRIPT] [--size WxH] --out FILE.png";

struct Args {
    seed: u64,
    nation: fn_sim::Nation,
    keys: Vec<fn_ui::script::Step>,
    size: (u32, u32),
    out: PathBuf,
}

fn parse(args: Vec<String>) -> Result<Args, String> {
    let mut positional = Vec::new();
    let (mut keys, mut size, mut out) = (String::new(), (1280, 800), None);
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut value = |flag: &str| it.next().ok_or_else(|| format!("{flag} needs a value"));
        match a.as_str() {
            "--keys" => keys = value("--keys")?,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--size" => {
                let v = value("--size")?;
                size = v
                    .split_once(['x', 'X'])
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .filter(|&(w, h)| w >= 320 && h >= 240)
                    .ok_or_else(|| format!("--size {v}: expected WxH, at least 320x240"))?;
            }
            _ if a.starts_with("--") => return Err(format!("unknown option {a}")),
            _ => positional.push(a),
        }
    }
    let [seed, nation] = positional.as_slice() else {
        return Err("expected SEED and NATION".to_owned());
    };
    Ok(Args {
        seed: seed.parse().map_err(|_| format!("SEED {seed}: not a number"))?,
        nation: fn_tui::cli::parse_nation(nation)
            .ok_or_else(|| format!("NATION {nation}: not a nation"))?,
        keys: fn_ui::script::parse(&keys).map_err(|e| format!("--keys: {e}"))?,
        size,
        out: out.ok_or("--out FILE.png is required")?,
    })
}

fn main() -> ExitCode {
    let args = match parse(std::env::args().skip(1).collect()) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("gui-shot: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    // `egui_kittest` panics when wgpu finds no adapter. Say so plainly instead of a backtrace.
    panic::set_hook(Box::new(|_| {}));
    match panic::catch_unwind(|| render(&args)) {
        Ok(Ok(())) => {
            println!("gui-shot: wrote {}", args.out.display());
            ExitCode::SUCCESS
        }
        Ok(Err(e)) => {
            eprintln!("gui-shot: {e}");
            ExitCode::FAILURE
        }
        Err(_) => {
            eprintln!(
                "gui-shot: no wgpu adapter. The shot renders through wgpu, so it needs a GPU or \
                 a software Vulkan driver (Mesa's lavapipe: `apt install mesa-vulkan-drivers`)."
            );
            ExitCode::FAILURE
        }
    }
}

fn render(args: &Args) -> Result<(), String> {
    let mut shell = Shell::new_game(fn_tui::new_session(args.seed, args.nation), (120, 40), None);
    shell.play(&args.keys);
    let (w, h) = args.size;
    let mut app = GuiApp::with_shell(shell);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(w as f32, h as f32))
        .wgpu()
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| app.show(ui));
    // The app asks for a repaint every frame, so `run()` would never settle: a few steps lay out
    // the window, size the shell to it and let the map widget make its first paint.
    harness.run_steps(4);
    let image = harness.render()?;
    if let Some(dir) = args.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    image.save(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))
}
