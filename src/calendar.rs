//! The Calendar: a week of days, each listing the items made or changed that day, with a bar that
//! shows how much was done. Chosen days can mark or filter the board.

use crate::board::Id;
use crate::dates::{DayMode, day_of};
use crate::editor::{Editor, plural};
use crate::theme::{self, UiExt, pal};
use chrono::{Datelike, Duration, Local, NaiveDate};
use egui::{Align2, CornerRadius, FontId, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

/// Monday of the week `offset` weeks from this one.
fn week_start(offset: i64) -> NaiveDate {
    let today = Local::now().date_naive();
    today - Duration::days(today.weekday().num_days_from_monday() as i64) + Duration::weeks(offset)
}

/// One item on one day.
struct Entry {
    id: Id,
    /// Made that day (rather than changed).
    new: bool,
    /// When it was made, for sorting.
    created: String,
}

pub fn calendar_ui(ui: &mut Ui, ed: &mut Editor) {
    let p = pal(ui.ctx());
    let start = week_start(ed.cal_week);
    let days: Vec<NaiveDate> = (0..7).map(|i| start + Duration::days(i)).collect();
    let keys: Vec<String> = days.iter().map(|d| d.format("%Y-%m-%d").to_string()).collect();
    let today = Local::now().date_naive();

    // ── header: the week, then what the chosen days do ──
    ui.horizontal(|ui| {
        if ui.button("‹").on_hover_text("Previous week").clicked() {
            ed.cal_week -= 1;
        }
        if ui.add_enabled(ed.cal_week != 0, egui::Button::new("This week")).clicked() {
            ed.cal_week = 0;
        }
        if ui.button("›").on_hover_text("Next week").clicked() {
            ed.cal_week += 1;
        }
        let end = days[6];
        let range = if start.month() == end.month() {
            format!("{} – {}", start.format("%-d"), end.format("%-d %b %Y"))
        } else {
            format!("{} – {}", start.format("%-d %b"), end.format("%-d %b %Y"))
        };
        ui.label(theme::semibold(range).size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.allocate_ui(vec2(300.0, 26.0), |ui| {
                let mut mode = ed.day_mode;
                if ui.segmented(&mut mode, &[(DayMode::All, "Show all"), (DayMode::Highlight, "Highlight"), (DayMode::Only, "Only these days")]) {
                    ed.day_mode = mode;
                }
            });
        });
    });
    let hint = if ed.cal_days.is_empty() {
        "Click a day to choose it, Ctrl-click to add more. Then highlight its items on the board or show only them.".to_owned()
    } else {
        let mut chosen: Vec<String> = ed.cal_days.iter().filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).map(|d| d.format("%a %-d %b").to_string()).collect();
        chosen.truncate(4);
        format!("Chosen: {}{}. Click a chosen day again to clear it.", chosen.join(", "), if ed.cal_days.len() > 4 { "…" } else { "" })
    };
    ui.label(theme::secondary(ui, hint));
    ui.add_space(4.0);

    // ── each day's items ──
    let inner = ed.board.sketch_inner();
    let mut buckets: Vec<Vec<Entry>> = (0..7).map(|_| Vec::new()).collect();
    for o in ed.board.objects.iter().filter(|o| !inner.contains(&o.id)) {
        for d in &o.days {
            if let Some(k) = keys.iter().position(|k| k == d) {
                buckets[k].push(Entry { id: o.id, new: day_of(&o.created) == d, created: o.created.clone() });
            }
        }
    }
    for b in &mut buckets {
        b.sort_by(|a, b| a.created.cmp(&b.created));
    }
    let most = buckets.iter().map(|b| b.len()).max().unwrap_or(0).max(1);

    let gap = 6.0;
    let area = ui.available_rect_before_wrap();
    let col_w = ((area.width() - gap * 6.0) / 7.0).max(60.0);
    let mut pick: Option<Id> = None;
    let mut toggle: Option<(String, bool)> = None;
    for (k, day) in days.iter().enumerate() {
        let rect = Rect::from_min_size(pos2(area.left() + k as f32 * (col_w + gap), area.top()), vec2(col_w, area.height().max(80.0)));
        let key = &keys[k];
        let chosen = ed.cal_days.contains(key);
        let is_today = *day == today;
        let count = buckets[k].len();

        // The card, tinted when chosen and outlined in blue for today.
        let fill = if chosen { p.accent.gamma_multiply(0.16) } else { p.inner_card_fill };
        let stroke = if chosen || is_today { Stroke::new(if chosen { 1.5 } else { 1.0 }, p.accent) } else { Stroke::new(1.0, p.inner_card_stroke) };
        ui.painter().rect(rect, CornerRadius::same(10), fill, stroke, StrokeKind::Inside);

        // Header: day, date, count and the highlighter bar. Clicking it chooses the day.
        let head = Rect::from_min_size(rect.min, vec2(rect.width(), 52.0));
        let resp = ui.interact(head, ui.id().with(("cal-day", key)), Sense::click());
        if resp.hovered() && !chosen {
            ui.painter().rect_filled(head.shrink(1.0), CornerRadius { nw: 9, ne: 9, sw: 0, se: 0 }, p.bg_control_hover);
        }
        if resp.clicked() {
            toggle = Some((key.clone(), ui.input(|i| i.modifiers.command || i.modifiers.shift)));
        }
        let painter = ui.painter();
        let number = if count == 0 { p.text_secondary } else { p.text };
        painter.text(head.min + vec2(10.0, 8.0), Align2::LEFT_TOP, day.format("%a").to_string().to_uppercase(), FontId::proportional(10.5), if is_today { p.accent } else { p.text_secondary });
        painter.text(head.min + vec2(10.0, 21.0), Align2::LEFT_TOP, day.format("%-d").to_string(), FontId::new(17.0, theme::semibold_family()), number);
        if count > 0 {
            painter.text(pos2(head.right() - 10.0, head.top() + 23.0), Align2::RIGHT_TOP, count.to_string(), FontId::proportional(11.0), p.text_secondary);
        }
        // The highlighter: a bar as long as that day's share of the week's busiest day.
        let track = Rect::from_min_size(pos2(head.left() + 10.0, head.bottom() - 8.0), vec2(head.width() - 20.0, 4.0));
        painter.rect_filled(track, CornerRadius::same(2), p.hairline.gamma_multiply(0.6));
        if count > 0 {
            let w = track.width() * count as f32 / most as f32;
            painter.rect_filled(Rect::from_min_size(track.min, vec2(w.max(4.0), track.height())), CornerRadius::same(2), p.accent);
        }
        resp.on_hover_text(if count == 0 { "Nothing on this day".to_owned() } else { format!("{} made or changed", plural(count, "item", "items")) });

        // The items, oldest first. Clicking one selects it and brings it into view.
        let list = Rect::from_min_max(pos2(rect.left() + 4.0, head.bottom() + 4.0), rect.max - vec2(4.0, 6.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list).id_salt(("cal-list", key)));
        ScrollArea::vertical().id_salt(("cal-scroll", key)).auto_shrink([false, false]).show(&mut child, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
            for e in &buckets[k] {
                let sel = ed.selection.contains(&e.id);
                let text = RichText::new(ed.board.display_name(e.id)).size(12.0).color(if e.new { p.text } else { p.text_secondary });
                let row = ui.add(egui::Button::selectable(sel, text).frame_when_inactive(false).min_size(vec2(ui.available_width(), 20.0)));
                // A small dot marks items made that day; changed ones have none.
                if e.new {
                    ui.painter().circle_filled(pos2(row.rect.left() + 3.0, row.rect.center().y), 2.2, p.accent);
                }
                let tip = if e.new { format!("Made at {}", e.created.get(11..16).unwrap_or("")) } else { "Changed".to_owned() };
                if row.on_hover_text(tip).clicked() {
                    pick = Some(e.id);
                }
            }
        });
    }
    ui.allocate_rect(area, Sense::hover());

    if let Some((key, add)) = toggle {
        if ed.cal_days.contains(&key) {
            ed.cal_days.retain(|d| *d != key);
        } else if add {
            ed.cal_days.push(key);
        } else {
            ed.cal_days = vec![key];
        }
        // Choosing a day starts highlighting it, unless a way of showing it is already picked.
        if !ed.cal_days.is_empty() && ed.day_mode == DayMode::All {
            ed.day_mode = DayMode::Highlight;
        }
    }
    if let Some(id) = pick {
        ed.go_to(id);
    }
}
