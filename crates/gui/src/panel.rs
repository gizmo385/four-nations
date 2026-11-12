//! The one section converted to real widgets (#842): the side panel's header (run state, speed,
//! the clock) and its People rows (count and idle, need flags, beasts), plus the settlers
//! themselves as a list, all from `fn_ui` view models. Clicking a settler selects it, as `Enter`
//! on its tile would. This is the cost sample for converting a screen: compare it with
//! `fn_tui::panel`'s `header`, `people_row` and `need_flags_row`.

use eframe::egui::{self, Color32, RichText, Ui};
use fn_ui::app::App;
use fn_ui::view::{Activity, activity_counts, beast_count, need_flags, side_view, unit_view};

const WARN: Color32 = Color32::from_rgb(230, 180, 60);
const BAD: Color32 = Color32::from_rgb(230, 80, 70);
const DIM: Color32 = Color32::from_rgb(140, 140, 150);

/// The header and People rows of `app`, as egui widgets.
pub fn show(ui: &mut Ui, app: &mut App) {
    let session = &app.session;
    let view = side_view(session, app.ui.tool, &app.ui.notice, app.ui.choice, &app.ui.alerts);

    ui.heading(view.nation.to_uppercase());
    ui.horizontal(|ui| {
        let (icon, word, color) = if view.paused {
            ("‖", "Paused", WARN)
        } else {
            ("▶", "Running", Color32::from_rgb(120, 200, 120))
        };
        ui.label(RichText::new(format!("{icon} {word}")).color(color).strong());
        ui.label(format!("x{}", view.speed));
    });
    ui.label(&view.clock);
    ui.separator();

    ui.label(RichText::new("SETTLEMENT").color(DIM).small());
    let idle = activity_counts(session)
        .into_iter()
        .find(|&(a, _)| a == Activity::Idle)
        .map_or(0, |(_, n)| n);
    ui.horizontal(|ui| {
        let people = if view.population.len() <= 1 {
            view.people.to_string()
        } else {
            let by: Vec<String> = view.population.iter().map(|(n, c)| format!("{n} {c}")).collect();
            format!("{} ({})", view.people, by.join(", "))
        };
        ui.label(format!("People {people}"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("Idle {idle}"));
        });
    });
    let flags = need_flags(session);
    if !flags.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (word, n) in &flags {
                let bad = matches!(word.as_str(), "Starving" | "Parched" | "Exhausted" | "Dying");
                ui.label(RichText::new(format!("{word} {n}")).color(if bad { BAD } else { WARN }));
            }
        });
    }
    let beasts = beast_count(session);
    if beasts.total > 0 {
        ui.horizontal(|ui| {
            ui.label(format!("Beasts {}", beasts.total));
            if beasts.hungry > 0 {
                ui.label(RichText::new(format!("Hungry {}", beasts.hungry)).color(WARN));
            }
        });
    }
    ui.separator();

    // The settlers themselves: a list the terminal has no room for in the panel (it is the
    // Nation screen's roster, `N`). A click selects the settler and puts the cursor on it.
    let rows: Vec<_> = session
        .world
        .units
        .iter()
        .filter(|u| !u.is_beast())
        .map(|u| (u.id, u.pos, unit_view(session, u)))
        .collect();
    let mut pick = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("people").striped(true).num_columns(2).show(ui, |ui| {
            for (id, pos, v) in &rows {
                let selected = app.ui.selected == Some(*id);
                if ui.selectable_label(selected, &v.name).clicked() {
                    pick = Some((*id, *pos));
                }
                ui.label(RichText::new(&v.task).color(DIM));
                ui.end_row();
            }
        });
    });
    if let Some((id, pos)) = pick {
        app.ui.point_at(pos);
        app.ui.select(Some(id));
        app.dirty = true;
    }
}
