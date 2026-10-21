//! The Overview: a small floating window with the whole board and a frame for what the canvas shows.
//! Click or drag in it to move the view; scroll over it to zoom.

use crate::board::{self, Kind};
use crate::canvas::{paint_shape, paint_stroke};
use crate::editor::Editor;
use crate::theme::{self, pal};
use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, vec2};

const DEFAULT_SIZE: egui::Vec2 = vec2(240.0, 160.0);

/// Show the Overview window over the canvas while it is switched on.
pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    let canvas = ed.canvas_rect;
    let mut open = ed.show_overview;
    // Only over the board, not over a note open in its place.
    if !open || !ed.canvas_visible || canvas.width() < 120.0 || canvas.height() < 120.0 {
        return;
    }
    let p = pal(ctx);
    let frame = egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::same(8));
    egui::Window::new(theme::semibold("Overview").size(12.0))
        .id(egui::Id::new("overview-window"))
        .open(&mut open)
        .frame(frame)
        .default_pos(canvas.right_bottom() - DEFAULT_SIZE - vec2(28.0, 56.0))
        .default_size(DEFAULT_SIZE)
        .min_size(vec2(140.0, 90.0))
        .resizable(true)
        .collapsible(true)
        .constrain_to(canvas)
        .show(ctx, |ui| {
            let size = ui.available_size().max(vec2(140.0, 90.0));
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let view = Rect::from_min_max(ed.camera.to_world(canvas.min, canvas.min), ed.camera.to_world(canvas.min, canvas.max));

            // While dragging, keep the mapping still so the map does not slide under the pointer.
            let bounds_id = resp.id.with("bounds");
            let live = ed.board.bounds().map(|b| b.union(view)).unwrap_or(view).expand(60.0);
            let bounds = if resp.dragged() { ui.data(|d| d.get_temp::<Rect>(bounds_id)).unwrap_or(live) } else { live };
            if resp.drag_started() || !resp.dragged() {
                ui.data_mut(|d| d.insert_temp(bounds_id, live));
            }
            let scale = (rect.width() / bounds.width()).min(rect.height() / bounds.height());
            let to_mini = |w: Pos2| rect.center() + (w - bounds.center()) * scale;
            let to_world = |s: Pos2| bounds.center() + (s - rect.center()) / scale;

            // Navigate.
            if resp.clicked() || resp.dragged() {
                if let Some(s) = resp.interact_pointer_pos() {
                    ed.camera.look_at(canvas, to_world(s));
                }
            }
            if resp.hovered() {
                let (scroll, zd) = ui.input(|i| (i.smooth_scroll_delta().y, i.zoom_delta()));
                let factor = zd * (scroll * 0.0018).exp();
                if (factor - 1.0).abs() > 1e-4 {
                    ed.camera.zoom_at(canvas.min, canvas.center(), factor);
                }
            }
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);

            let painter = ui.painter_at(rect);
            painter.rect(rect, CornerRadius::same(6), p.canvas, Stroke::new(1.0, p.hairline), StrokeKind::Inside);
            if ed.board.objects.is_empty() {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, "The board is empty.", egui::FontId::proportional(11.0), p.text_secondary);
            }

            let look = ed.board.lookup();
            for k in 0..ed.board.objects.len() {
                if look.hidden(ed.board.objects[k].id) {
                    continue;
                }
                let image = if let Kind::Image { image, .. } = ed.board.objects[k].kind { Some(image) } else { None };
                let tex = image.and_then(|img| ed.texture(ctx, img));
                let o = &ed.board.objects[k];
                let r = Rect::from_min_max(to_mini(o.rect().min), to_mini(o.rect().max));
                let ink = ed.board.ink_of(o).color(p);
                match &o.kind {
                    Kind::Group { sketch: true, .. } => {}
                    Kind::Group { .. } => {
                        let c = if o.style.ink == board::Ink::Auto { p.text_secondary } else { ink };
                        painter.rect(r, CornerRadius::same(3), c.gamma_multiply(0.08), Stroke::new(1.0, c.gamma_multiply(0.8)), StrokeKind::Inside);
                    }
                    Kind::Image { .. } => match tex {
                        Some(t) => {
                            painter.image(t, r, o.image_uv(), Color32::WHITE);
                        }
                        None => {
                            painter.rect_filled(r, 0.0, p.faint_bg);
                        }
                    },
                    Kind::Path { brush, .. } => {
                        let pts = o.line_points();
                        // Thin out long strokes; the map is small.
                        let step = (pts.len() / 48).max(1);
                        let mut mini: Vec<Pos2> = pts.iter().step_by(step).map(|q| to_mini(*q)).collect();
                        if let Some(last) = pts.last() {
                            mini.push(to_mini(*last));
                        }
                        let w = (o.style.width * scale).max(0.6);
                        paint_stroke(&painter, &mini, Stroke::new(w, ink.gamma_multiply(brush.alpha())), *brush);
                    }
                    // A few pixels across, a shape looks the same as a filled box and is far cheaper.
                    Kind::Shape { .. } if r.width().max(r.height()) < 5.0 => {
                        painter.rect_filled(r, 0.0, ink.gamma_multiply(if o.style.fill { 0.6 } else { 0.4 }));
                    }
                    Kind::Shape { shape } => {
                        let fill = if o.style.fill { ink.gamma_multiply(0.25) } else { Color32::TRANSPARENT };
                        paint_shape(&painter, *shape, r, scale, fill, Stroke::new(1.0, ink));
                    }
                    Kind::Frame { .. } => {
                        painter.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.0, p.hairline), StrokeKind::Inside);
                    }
                    Kind::Note { .. } => {
                        painter.rect(r, CornerRadius::same(2), p.window_fill, Stroke::new(1.0, p.window_stroke), StrokeKind::Inside);
                    }
                    Kind::Text => {
                        painter.rect_filled(r, 1.0, ink.gamma_multiply(0.35));
                    }
                }
            }
            for w in &ed.board.wires {
                if let Some(pts) = look.wire_points(&ed.board, w) {
                    let step = (pts.len() / 8).max(1);
                    let mut mini: Vec<Pos2> = pts.iter().step_by(step).map(|q| to_mini(*q)).collect();
                    mini.push(to_mini(pts[pts.len() - 1]));
                    painter.add(egui::Shape::line(mini, Stroke::new(1.0, p.text_secondary.gamma_multiply(0.7))));
                }
            }

            // What the canvas shows right now.
            let vr = Rect::from_min_max(to_mini(view.min), to_mini(view.max)).intersect(rect.shrink(1.0));
            painter.rect(vr, CornerRadius::same(2), p.accent.gamma_multiply(0.10), Stroke::new(1.5, p.accent), StrokeKind::Inside);
        });
    // The window's × switches it off.
    ed.show_overview = open;
}
