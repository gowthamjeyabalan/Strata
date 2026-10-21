//! The date roller under the board: one row of days, each with a bar for how much was made or
//! changed that day. Clicking days highlights their items on the board.

use crate::editor::{Editor, plural};
use chrono::{Datelike, Duration, Local, NaiveDate};
use egui::{Align2, CornerRadius, FontId, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use std::collections::HashMap;

/// Height of the roller.
pub const ROLLER: f32 = 46.0;
const CHIP: f32 = 38.0;

pub fn date_roller(ui: &mut Ui, ed: &mut Editor, rect: Rect) {
    let p = crate::theme::pal(ui.ctx());
    ui.painter().rect_filled(rect, CornerRadius { nw: 0, ne: 0, sw: 10, se: 10 }, p.bg);
    let today = Local::now().date_naive();

    // How much happened each day, and from which day there is anything at all.
    let inner = ed.board.sketch_inner();
    let mut count: HashMap<&str, usize> = HashMap::new();
    for o in ed.board.objects.iter().filter(|o| !inner.contains(&o.id)) {
        for d in &o.days {
            *count.entry(d.as_str()).or_default() += 1;
        }
    }
    let first = count.keys().filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).min().unwrap_or(today);
    let start = (first - Duration::days(3)).max(today - Duration::days(400)).min(today - Duration::days(21));
    let end = today + Duration::days(3);
    let most = count.values().copied().max().unwrap_or(1).max(1);

    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(8.0, 4.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let ui = &mut child;
    let mut toggle: Option<(String, bool)> = None;
    if !ed.cal_days.is_empty() {
        if ui.add(egui::Button::new(egui::RichText::new("Clear").size(11.5)).frame(false)).on_hover_text("Stop highlighting days").clicked() {
            ed.cal_days.clear();
        }
    }
    // The newest days are on the right, and the roller starts scrolled to them.
    ScrollArea::horizontal().id_salt("date-roller").stick_to_right(true).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
        ui.horizontal(|ui| {
            let mut day = start;
            while day <= end {
                let key = day.format("%Y-%m-%d").to_string();
                let n = count.get(key.as_str()).copied().unwrap_or(0);
                let chosen = ed.cal_days.contains(&key);
                let (r, resp) = ui.allocate_exact_size(vec2(CHIP, ROLLER - 10.0), Sense::click());
                let painter = ui.painter();
                let fill = if chosen { p.accent.gamma_multiply(0.22) } else if resp.hovered() { p.bg_control_hover } else { p.inner_card_fill };
                let stroke = if chosen { Stroke::new(1.5, p.accent) } else if day == today { Stroke::new(1.0, p.accent.gamma_multiply(0.7)) } else { Stroke::new(1.0, p.inner_card_stroke) };
                painter.rect(r, CornerRadius::same(7), fill, stroke, StrokeKind::Inside);
                // The month on its first day, the weekday otherwise.
                let top = if day.day() == 1 { day.format("%b").to_string() } else { day.format("%a").to_string().chars().take(2).collect() };
                let dim = if n == 0 { p.text_secondary } else { p.text };
                painter.text(pos2(r.center().x, r.top() + 4.0), Align2::CENTER_TOP, top, FontId::proportional(9.5), if day == today { p.accent } else { p.text_secondary });
                painter.text(pos2(r.center().x, r.top() + 15.0), Align2::CENTER_TOP, day.day().to_string(), FontId::new(12.5, crate::theme::semibold_family()), dim);
                // The bar: that day's share of the busiest day.
                if n > 0 {
                    let w = (r.width() - 12.0) * (n as f32 / most as f32).max(0.15);
                    painter.rect_filled(Rect::from_min_size(pos2(r.center().x - w * 0.5, r.bottom() - 6.0), vec2(w, 3.0)), CornerRadius::same(2), p.accent);
                }
                let tip = format!("{}: {}", day.format("%a %-d %b %Y"), if n == 0 { "nothing made or changed".to_owned() } else { format!("{} made or changed", plural(n, "item", "items")) });
                if resp.on_hover_text(tip).clicked() {
                    toggle = Some((key, ui.input(|i| i.modifiers.command || i.modifiers.shift)));
                }
                day += Duration::days(1);
            }
        });
    });
    if let Some((key, add)) = toggle {
        if ed.cal_days.contains(&key) {
            ed.cal_days.retain(|d| *d != key);
        } else if add {
            ed.cal_days.push(key);
        } else {
            ed.cal_days = vec![key];
        }
    }
}
