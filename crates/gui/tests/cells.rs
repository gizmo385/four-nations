//! The cell painter (#872): what it draws is what the terminal shows, glyph for glyph, and what
//! it costs. Nothing here opens a window or needs a GPU.

use std::time::Instant;

use eframe::egui::{self, Color32, Pos2, Shape};
use fn_gui::cells::{self, CellLayout, DEFAULT_BG, Metrics};
use fn_sim::Nation;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_shell, to_text};
use fn_tui::shell::Shell;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

const SIZE: (u16, u16) = (120, 40);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Water), SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

/// A context whose fonts are installed the way the window installs them.
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    fn_gui::setup(&ctx);
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    ctx
}

/// The shell's frame at `size`, as `fn_tui --frame` makes it.
fn frame(shell: &mut Shell, size: (u16, u16)) -> Buffer {
    let palette = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
    render_shell(shell, &palette, size)
}

/// The screens to compare: the title screen and its nation picker, embark, a game frame, and a
/// full-screen page of each kind (`reference_goldens.rs`' keys).
fn shells() -> Vec<(&'static str, Shell)> {
    let mut title = Shell::new_title(SIZE, None, 1);
    let mut out = vec![("title", Shell::new_title(SIZE, None, 1))];
    title.play(&fn_ui::script::parse("<enter>").unwrap());
    out.push(("title step two", title));
    let defs = fn_sim::Defs::bundled();
    let embark = Shell::new_embark(
        7,
        Nation::Earth,
        defs.embark.map_sizes[0].clone(),
        fn_sim::embark::SurveyMode::Exact,
        None,
        SIZE,
        None,
    );
    out.push(("embark", embark));
    out.push(("game", game("")));
    for (name, keys) in [
        ("build", "u"),
        ("research", "t"),
        ("nation", "n"),
        ("abilities", "i"),
        ("endeavor", "g"),
        ("menu", "<esc>"),
        ("help", "?"),
        ("tool", "h <enter> <right*7> <up*2>"),
    ] {
        out.push((name, game(keys)));
    }
    out
}

/// The painter's glyph grid is the buffer's text, for every screen and at three sizes.
#[test]
fn the_painters_glyph_grid_is_what_fn_tui_frame_prints() {
    for size in [SIZE, (80, 24), (200, 60)] {
        for (name, mut shell) in shells() {
            let buf = frame(&mut shell, size);
            let layout = CellLayout::new(&buf);
            assert_eq!((layout.cols, layout.rows), size, "{name}");
            assert_eq!(layout.text(), to_text(&buf), "{name} at {size:?}");
            // And it is not vacuous: a screen shows something.
            assert!(layout.glyphs.len() > 100, "{name} at {size:?}");
        }
    }
}

/// The glyphs in the egui shapes are the layout's: one text shape per glyph, and the galleys
/// hold the buffer's symbols in reading order.
#[test]
fn every_glyph_becomes_one_text_shape() {
    let ctx = context();
    let metrics = Metrics::new(&ctx);
    for (name, mut shell) in shells() {
        let buf = frame(&mut shell, SIZE);
        let layout = CellLayout::new(&buf);
        let shapes = ctx.fonts_mut(|f| cells::shapes(f, Pos2::new(10.0, 20.0), &metrics, &layout));
        let drawn: String = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        let wanted: String = layout.glyphs.iter().map(|g| g.symbol).collect();
        assert_eq!(drawn, wanted, "{name}");
        let texts = shapes.iter().filter(|s| matches!(s, Shape::Text(_))).count();
        assert_eq!(texts, layout.glyphs.len(), "{name}");
    }
}

fn cell_buffer() -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, 8, 2));
    let red = Style::new().bg(Color::Rgb(200, 0, 0));
    let blue = Style::new().bg(Color::Rgb(0, 0, 200));
    buf.set_string(0, 0, "ab", red.fg(Color::White));
    buf.set_string(2, 0, "c", red);
    buf.set_string(3, 0, "d", blue.fg(Color::Rgb(0, 255, 0)));
    // A wide glyph covers two cells.
    buf.set_string(5, 0, "世", Style::new());
    buf.set_string(
        0,
        1,
        "x",
        Style::new()
            .fg(Color::Rgb(10, 20, 30))
            .bg(Color::Rgb(1, 2, 3))
            .add_modifier(Modifier::REVERSED),
    );
    buf
}

#[test]
fn equal_backgrounds_in_a_row_are_one_run() {
    let buf = cell_buffer();
    let layout = CellLayout::new(&buf);
    let rows0: Vec<_> = layout.backgrounds.iter().filter(|r| r.row == 0).collect();
    // Three red cells, then one blue; the default background is the base rectangle's.
    assert_eq!(rows0.len(), 2, "{rows0:?}");
    assert_eq!((rows0[0].col, rows0[0].len, rows0[0].color), (0, 3, Color32::from_rgb(200, 0, 0)));
    assert_eq!((rows0[1].col, rows0[1].len, rows0[1].color), (3, 1, Color32::from_rgb(0, 0, 200)));
    assert!(layout.backgrounds.iter().all(|r| r.color != DEFAULT_BG));
}

#[test]
fn reversed_swaps_colours_and_a_wide_glyph_spans_two_cells() {
    let buf = cell_buffer();
    let layout = CellLayout::new(&buf);
    let x = layout.glyphs.iter().find(|g| g.symbol == "x").unwrap();
    assert_eq!(x.fg, Color32::from_rgb(1, 2, 3));
    let run = layout.backgrounds.iter().find(|r| r.row == 1).unwrap();
    assert_eq!(run.color, Color32::from_rgb(10, 20, 30));
    let wide = layout.glyphs.iter().find(|g| g.symbol == "世").unwrap();
    assert_eq!((wide.col, wide.width), (5, 2));
    // Its second cell holds no glyph of its own.
    assert!(!layout.glyphs.iter().any(|g| g.row == 0 && g.col == 6));
}

#[test]
fn metrics_are_whole_pixels_and_fit_a_window() {
    for ppp in [1.0, 1.25, 1.5, 2.0] {
        let ctx = context();
        ctx.set_pixels_per_point(ppp);
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
        let m = Metrics::new(&ctx);
        for v in [m.cell.x, m.cell.y] {
            assert!(((v * ppp).round() - v * ppp).abs() < 1e-3, "{v} at {ppp}");
        }
        let (cols, rows) = m.grid_in(egui::vec2(1600.0, 960.0));
        assert!(cols as f32 * m.cell.x <= 1600.0 && (cols as f32 + 1.0) * m.cell.x > 1600.0);
        assert!(rows as f32 * m.cell.y <= 960.0 && (rows as f32 + 1.0) * m.cell.y > 960.0);
    }
}

/// The cost of the cell paint at the native window's size (1600 by 960 points), with a
/// full-screen page open: layout, shapes, and, on top, egui's tessellation of them. Ignored by
/// default (it is a measurement, and only means something in release):
///
/// `cargo test --release -p fn_gui --test it cell_paint_cost -- --ignored --nocapture`
#[test]
#[ignore = "a measurement: run it in release"]
fn cell_paint_cost() {
    let ctx = context();
    let metrics = Metrics::new(&ctx);
    let (cols, rows) = metrics.grid_in(egui::vec2(1600.0, 960.0));
    println!(
        "cell {:.0} x {:.0} pt, {cols} x {rows} cells at 1600 x 960 (release: {})",
        metrics.cell.x,
        metrics.cell.y,
        !cfg!(debug_assertions)
    );
    let mut cases: Vec<(&str, Buffer)> = Vec::new();
    for (name, keys) in
        [("build", "u"), ("research", "t"), ("nation", "n"), ("help", "?"), ("game", "")]
    {
        let mut shell = game(keys);
        cases.push((name, frame(&mut shell, (cols, rows))));
    }
    // What a native game frame (#876) still paints as cells: the side panel's slot (300 points,
    // 37 columns, down to the hint bar) and, with the menu open, its box. The map's own 10 000
    // glyphs are not in a native frame: `MapView` paints them, and the terminal's are skipped.
    let shell = game("");
    if let fn_tui::shell::Mode::Game(app) = &shell.mode {
        let palette = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
        let slot = Rect::new(0, 0, (300.0 / metrics.cell.x) as u16, rows - 1);
        let mut buf = Buffer::empty(slot);
        fn_tui::panel::draw_panel(&app.session, &app.ui, &palette, slot, &mut buf);
        cases.push(("game, native: panel slot", buf));
        let area = Rect::new(0, 0, fn_tui::menu::SIZE.0, fn_tui::menu::SIZE.1);
        let mut buf = Buffer::empty(area);
        fn_tui::menu::draw(&app.ui, &[], "Day 1", &palette, area, &mut buf);
        cases.push(("game, native: menu box", buf));
    }
    for (name, buf) in cases {
        let runs = CellLayout::new(&buf);
        let (mut layout_ms, mut shapes_ms, mut tess_ms) = (0.0, 0.0, 0.0);
        const N: u32 = 200;
        for i in 0..N + 20 {
            // 20 warm-up rounds: the first rasterises glyphs into egui's atlas.
            let warm = i >= 20;
            let t = Instant::now();
            let layout = CellLayout::new(&buf);
            let a = t.elapsed();
            let t = Instant::now();
            let shapes = ctx.fonts_mut(|f| cells::shapes(f, Pos2::ZERO, &metrics, &layout));
            let b = t.elapsed();
            let t = Instant::now();
            let clipped: Vec<_> = shapes
                .into_iter()
                .map(|shape| egui::epaint::ClippedShape {
                    clip_rect: egui::Rect::EVERYTHING,
                    shape,
                    transform_after_tessellation: egui::emath::TSTransform::IDENTITY,
                })
                .collect();
            let meshes = ctx.tessellate(clipped, 1.0);
            let c = t.elapsed();
            std::hint::black_box(meshes);
            if warm {
                layout_ms += a.as_secs_f64() * 1000.0;
                shapes_ms += b.as_secs_f64() * 1000.0;
                tess_ms += c.as_secs_f64() * 1000.0;
            }
        }
        let n = f64::from(N);
        println!(
            "{name:26} {:5} glyphs {:4} bg runs: layout {:.3} ms + shapes {:.3} ms = {:.3} ms \
             (egui tessellation on top: {:.3} ms)",
            runs.glyphs.len(),
            runs.backgrounds.len(),
            layout_ms / n,
            shapes_ms / n,
            (layout_ms + shapes_ms) / n,
            tess_ms / n
        );
    }
}
