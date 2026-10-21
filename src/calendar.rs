//! The calendar, in the left column under the notes: a month like the Windows calendar flyout. Days
//! with something made or changed have a dot; clicking days highlights their items on the board.

use crate::editor::{Editor, plural};
use crate::theme::{self, pal};
use chrono::{Datelike, Duration, Local, NaiveDate};
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use std::collections::HashMap;

/// The first of the month that `d` is in.
fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

/// The first of the month `n` months after (or before) `d`'s month.
fn add_months(d: NaiveDate, n: i32) -> NaiveDate {
    let m = d.year() * 12 + d.month0() as i32 + n;
    NaiveDate::from_ymd_opt(m.div_euclid(12), m.rem_euclid(12) as u32 + 1, 1).unwrap_or(d)
}

pub fn calendar_ui(ui: &mut Ui, ed: &mut Editor) {
    let p = pal(ui.ctx());
    let today = Local::now().date_naive();
    let shown = ed.cal_month.unwrap_or_else(|| month_start(today));

    // How much happened each day.
    let inner = ed.board.sketch_inner();
    let mut count: HashMap<&str, usize> = HashMap::new();
    for o in ed.board.objects.iter().filter(|o| !inner.contains(&o.id)) {
        for d in &o.days {
            *count.entry(d.as_str()).or_default() += 1;
        }
    }

    // Today, written out, as at the top of the Windows flyout.
    ui.add_space(4.0);
    ui.label(RichText::new(today.format("%A, %B %-d").to_string()).size(14.0).color(p.text));
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    // The month and its arrows.
    let mut month_step = 0;
    ui.horizontal(|ui| {
        ui.label(theme::semibold(shown.format("%B %Y").to_string()).size(14.0).color(p.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let arrow = |ui: &mut Ui, up: bool, tip: &str| -> bool {
                let (r, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
                let c = if resp.hovered() { p.text } else { p.text_secondary };
                let m = r.center();
                let pts = if up { vec![m + vec2(-4.5, 2.5), m + vec2(0.0, -2.5), m + vec2(4.5, 2.5)] } else { vec![m + vec2(-4.5, -2.5), m + vec2(0.0, 2.5), m + vec2(4.5, -2.5)] };
                ui.painter().add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
                resp.on_hover_text(tip).clicked()
            };
            if arrow(ui, false, "Next month") {
                month_step = 1;
            }
            if arrow(ui, true, "Previous month") {
                month_step = -1;
            }
            if shown != month_start(today) && ui.add(egui::Button::new(RichText::new("Today").size(11.5)).frame(false)).clicked() {
                ed.cal_month = None;
            }
        });
    });
    ui.add_space(6.0);

    // The grid: weekday names, then six weeks starting on a Sunday.
    let w = ui.available_width();
    let cell = (w / 7.0).floor();
    let rh = cell.clamp(26.0, 40.0);
    let (head, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    for (k, name) in ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"].iter().enumerate() {
        ui.painter().text(pos2(head.left() + cell * (k as f32 + 0.5), head.center().y), Align2::CENTER_CENTER, *name, FontId::new(11.5, theme::semibold_family()), p.text);
    }
    let (grid, gresp) = ui.allocate_exact_size(vec2(w, rh * 6.0), Sense::hover());
    // The mouse wheel turns the months, as in Windows.
    if gresp.hovered() {
        let dy = ui.input(|i| i.smooth_scroll_delta().y);
        let acc_id = ui.id().with("cal-wheel");
        let mut acc: f32 = ui.data(|d| d.get_temp(acc_id)).unwrap_or(0.0) + dy;
        if acc.abs() >= 40.0 {
            month_step = if acc > 0.0 { -1 } else { 1 };
            acc = 0.0;
        }
        ui.data_mut(|d| d.insert_temp(acc_id, acc));
    }
    let first = shown - Duration::days(shown.weekday().num_days_from_sunday() as i64);
    let mut toggle: Option<(NaiveDate, bool)> = None;
    for k in 0..42 {
        let day = first + Duration::days(k);
        let (row, col) = (k / 7, k % 7);
        let r = Rect::from_min_size(pos2(grid.left() + col as f32 * cell, grid.top() + row as f32 * rh), vec2(cell, rh));
        let key = day.format("%Y-%m-%d").to_string();
        let n = count.get(key.as_str()).copied().unwrap_or(0);
        let resp = ui.interact(r, ui.id().with(("cal-day", k)), Sense::click());
        let chosen = ed.cal_days.contains(&key);
        let in_month = day.month() == shown.month();
        let is_today = day == today;
        let c = r.center();
        let radius = (rh.min(cell) * 0.5 - 2.0).max(8.0);
        let painter = ui.painter();
        if is_today {
            painter.circle_filled(c, radius, p.accent);
        } else if chosen {
            painter.circle_filled(c, radius, p.accent.gamma_multiply(0.22));
        } else if resp.hovered() {
            painter.circle_filled(c, radius, p.bg_control_hover);
        }
        if chosen {
            painter.circle_stroke(c, radius, Stroke::new(1.5, if is_today { Color32::WHITE } else { p.accent }));
        }
        let color = if is_today { Color32::WHITE } else if in_month { p.text } else { p.text_secondary.gamma_multiply(0.7) };
        painter.text(c - vec2(0.0, if n > 0 { 2.0 } else { 0.0 }), Align2::CENTER_CENTER, day.day().to_string(), FontId::proportional(13.0), color);
        // A dot for a day with something made or changed.
        if n > 0 {
            painter.circle_filled(c + vec2(0.0, radius * 0.55), 1.8, if is_today { Color32::WHITE } else { p.accent });
        }
        let tip = format!("{}: {}", day.format("%A %-d %B"), if n == 0 { "nothing made or changed".to_owned() } else { format!("{} made or changed", plural(n, "item", "items")) });
        if resp.on_hover_text(tip).clicked() {
            toggle = Some((day, ui.input(|i| i.modifiers.command || i.modifiers.shift)));
        }
    }
    ui.add_space(8.0);
    ui.separator();

    // What is highlighted, and the way out of it.
    ui.horizontal(|ui| {
        if ed.cal_days.is_empty() {
            ui.label(theme::secondary(ui, "Click a day to highlight what was made or changed on it. Ctrl-click for more days."));
        } else {
            let n = ed.day_direct.len();
            let days = if ed.cal_days.len() == 1 {
                NaiveDate::parse_from_str(&ed.cal_days[0], "%Y-%m-%d").map(|d| d.format("%a %-d %b").to_string()).unwrap_or_default()
            } else {
                plural(ed.cal_days.len(), "day", "days")
            };
            ui.label(RichText::new(format!("{}, {days}", plural(n, "item", "items"))).size(12.5).color(p.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear").clicked() {
                    ed.cal_days.clear();
                }
            });
        }
    });

    if month_step != 0 {
        ed.cal_month = Some(add_months(shown, month_step));
    }
    if let Some((day, add)) = toggle {
        let key = day.format("%Y-%m-%d").to_string();
        if ed.cal_days.contains(&key) {
            ed.cal_days.retain(|d| *d != key);
        } else if add {
            ed.cal_days.push(key);
        } else {
            ed.cal_days = vec![key];
        }
        // A day of another month brings that month into view.
        if day.month() != shown.month() {
            ed.cal_month = Some(month_start(day));
        }
    }
}
