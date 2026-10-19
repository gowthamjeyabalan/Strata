//! The insert list: press "/" or drag a connection into empty space, type to filter, pick an item.
//! With a node selected (or a wire being dragged) the new item is connected to it.

use crate::board::{Id, Kind, Shape, Side, Style};
use crate::canvas::paint_shape;
use crate::editor::Editor;
use crate::theme::{self, pal};
use crate::tools::{self, Tool};
use egui::{Color32, CornerRadius, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2, pos2, vec2};

/// Where the new item goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// Beside the source node, to its right (after "/" with a node selected).
    RightOf,
    /// At a world point: where a wire was dropped, or the pointer.
    At(Pos2),
}

#[derive(Clone, Debug)]
pub struct InsertMenu {
    /// Node (and port) the new item is connected from.
    pub from: Option<(Id, Option<Side>)>,
    pub place: Place,
    pub query: String,
    pub index: usize,
    pub(crate) focus: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    Shape(Shape),
    Text,
    Image,
}

/// Every item, with words to find it by.
pub fn items() -> Vec<(Item, &'static str, &'static str)> {
    vec![
        (Item::Shape(Shape::Process), "Process", "step box action task"),
        (Item::Shape(Shape::Decision), "Decision", "if choice yes no branch diamond question"),
        (Item::Shape(Shape::Node), "Empty node", "blank box"),
        (Item::Text, "Text", "label words type write"),
        (Item::Shape(Shape::Note), "Note", "sticky comment idea"),
        (Item::Shape(Shape::Terminal), "Start or end", "terminal begin finish stop pill"),
        (Item::Shape(Shape::Data), "Input or output", "data io parallelogram"),
        (Item::Shape(Shape::Document), "Document", "file paper page report"),
        (Item::Shape(Shape::Database), "Database", "storage db cylinder store"),
        (Item::Shape(Shape::Subprocess), "Subprocess", "predefined routine function"),
        (Item::Shape(Shape::ManualInput), "Manual input", "keyboard enter form"),
        (Item::Shape(Shape::Connector), "Connector", "circle jump link reference"),
        (Item::Shape(Shape::Rectangle), "Rectangle", "square box shape"),
        (Item::Shape(Shape::Ellipse), "Ellipse", "circle oval round shape"),
        (Item::Image, "Image…", "picture photo screenshot file"),
    ]
}

/// Items matching `query`: every word must start a word of the name or keywords.
pub fn filter(query: &str) -> Vec<(Item, &'static str)> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_owned).collect();
    items()
        .into_iter()
        .filter(|(_, name, keys)| {
            let hay: Vec<String> = format!("{name} {keys}").to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect();
            words.iter().all(|q| hay.iter().any(|h| h.starts_with(q.as_str())))
        })
        .map(|(i, n, _)| (i, n))
        .collect()
}

/// Gap between a node and an item added beside it, in world units.
const GAP: f32 = 80.0;

fn opposite(s: Side) -> Side {
    match s {
        Side::Left => Side::Right,
        Side::Right => Side::Left,
        Side::Top => Side::Bottom,
        Side::Bottom => Side::Top,
    }
}

impl Editor {
    /// Open the list from the keyboard: connected to the selected node, or at the pointer.
    pub fn open_insert_here(&mut self) {
        self.finish_edit();
        let from = (self.selection.len() == 1).then(|| self.selection[0]).filter(|id| self.board.get(*id).is_some() && !self.board.hidden(*id));
        let place = match from {
            Some(_) => Place::RightOf,
            None => Place::At(self.paste_point()),
        };
        self.insert = Some(InsertMenu { from: from.map(|id| (id, Some(Side::Right))), place, query: String::new(), index: 0, focus: true });
    }

    /// Open the list where a connection was dropped on empty board.
    pub fn open_insert_from_wire(&mut self, from: Id, side: Option<Side>, at: Pos2) {
        self.insert = Some(InsertMenu { from: Some((from, side)), place: Place::At(at), query: String::new(), index: 0, focus: true });
    }

    /// World rect for a new item of `size`, and which of its sides the connection arrives at.
    fn insert_rect(&self, menu: &InsertMenu, size: Vec2) -> (Rect, Option<Side>) {
        let src = menu.from.and_then(|(id, _)| self.board.get(id));
        match (menu.place, src) {
            (Place::RightOf, Some(s)) => {
                // Beside the node; step down past anything already there.
                let sr = s.rect();
                let mut r = Rect::from_min_size(pos2(sr.right() + GAP, sr.center().y - size.y * 0.5), size);
                let hidden = self.board.hidden_set();
                for _ in 0..40 {
                    let busy = self.board.objects.iter().any(|o| !o.is_group() && !hidden.contains(&o.id) && o.rect().expand(12.0).intersects(r));
                    if !busy {
                        break;
                    }
                    r = r.translate(vec2(0.0, size.y + 24.0));
                }
                (r, Some(Side::Left))
            }
            (Place::At(p), Some(s)) => {
                // The new item's facing port sits where the wire was dropped.
                let from_port = menu.from.and_then(|(_, side)| side).map(|side| s.port(side)).unwrap_or(s.rect().center());
                let d = p - from_port;
                let side = if d.x.abs() >= d.y.abs() { if d.x >= 0.0 { Side::Left } else { Side::Right } } else if d.y >= 0.0 { Side::Top } else { Side::Bottom };
                let r = match side {
                    Side::Left => Rect::from_min_size(pos2(p.x, p.y - size.y * 0.5), size),
                    Side::Right => Rect::from_min_size(pos2(p.x - size.x, p.y - size.y * 0.5), size),
                    Side::Top => Rect::from_min_size(pos2(p.x - size.x * 0.5, p.y), size),
                    Side::Bottom => Rect::from_min_size(pos2(p.x - size.x * 0.5, p.y - size.y), size),
                };
                (r, Some(side))
            }
            (Place::At(p), None) => (Rect::from_center_size(p, size), None),
            (Place::RightOf, None) => (Rect::from_center_size(self.paste_point(), size), None),
        }
    }

    /// Create the picked item, connect it, and start typing its label.
    pub fn insert_item(&mut self, item: Item) {
        let Some(menu) = self.insert.take() else { return };
        if item == Item::Image {
            let before = self.board.objects.len();
            let at = match menu.place {
                Place::At(p) => p,
                Place::RightOf => self.insert_rect(&menu, vec2(200.0, 150.0)).0.center(),
            };
            self.pick_images_at(at);
            if let (Some((src, side)), true) = (menu.from, self.board.objects.len() > before) {
                if let Some(img) = self.selection.first().copied() {
                    let to_side = side.map(opposite).unwrap_or(Side::Left);
                    let from_side = side.unwrap_or(Side::Right);
                    self.board.connect(src, from_side, img, to_side, self.wire_style);
                    self.mark_changed();
                }
            }
            return;
        }
        let (kind, size, label) = match item {
            Item::Shape(s) => (Kind::Shape { shape: s }, s.default_size(), s.takes_label()),
            Item::Text => (Kind::Text, vec2(240.0, self.style.font_size * 1.4), true),
            Item::Image => unreachable!(),
        };
        let (rect, to_side) = self.insert_rect(&menu, size);
        let mut style = self.style;
        if matches!(item, Item::Shape(s) if !matches!(s, Shape::Rectangle | Shape::Ellipse)) {
            style.fill = true;
        }
        if item == Item::Text {
            style = Style { ink: self.style.ink, ..Style::default() };
        }
        let id = self.board.add(rect, kind, style);
        if let (Some((src, side)), Some(to)) = (menu.from, to_side) {
            let from_side = side.unwrap_or_else(|| Side::facing(self.board.get(src).map(|o| o.rect()).unwrap_or(rect), rect.center()));
            self.board.connect(src, from_side, id, to, self.wire_style);
        }
        self.mark_changed();
        self.selection = vec![id];
        self.selected_wire = None;
        if label {
            self.start_edit(id);
        }
    }

    /// Where the list appears on screen.
    fn insert_anchor(&self, menu: &InsertMenu) -> Pos2 {
        let origin = self.canvas_rect.min;
        match (menu.place, menu.from.and_then(|(id, _)| self.board.get(id))) {
            (Place::RightOf, Some(s)) => self.camera.to_screen(origin, s.rect().right_center()) + vec2(16.0, -18.0),
            (Place::At(p), _) => self.camera.to_screen(origin, p) + vec2(8.0, 8.0),
            _ => self.canvas_rect.center(),
        }
    }
}

/// Draw the list and handle typing, arrow keys, Enter and Escape.
pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    let Some(menu) = ed.insert.clone() else { return };
    let p = pal(ctx);
    let list = filter(&menu.query);
    let mut index = menu.index.min(list.len().saturating_sub(1));
    let mut picked = None;
    let mut close = false;

    // Keys first, so the search box does not also see them.
    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) && !list.is_empty() {
            index = (index + 1) % list.len();
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowUp) && !list.is_empty() {
            index = (index + list.len() - 1) % list.len();
        }
        if i.consume_key(Modifiers::NONE, Key::Enter) {
            picked = list.get(index).map(|(item, _)| *item);
        }
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            close = true;
        }
    });

    // Keep it on the board.
    let w = 230.0;
    let mut at = ed.insert_anchor(&menu);
    let canvas = ed.canvas_rect;
    at.x = at.x.min(canvas.right() - w - 8.0).max(canvas.left() + 8.0);
    at.y = at.y.min(canvas.bottom() - 360.0).max(canvas.top() + 8.0);

    let mut query = menu.query.clone();
    let area = egui::Area::new(egui::Id::new("insert-menu")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
        egui::Frame::new()
            .fill(p.window_fill)
            .stroke(Stroke::new(1.0, p.window_stroke))
            .corner_radius(10)
            .inner_margin(egui::Margin::same(6))
            .shadow(ctx.global_style().visuals.popup_shadow)
            .show(ui, |ui| {
                ui.set_width(w);
                let title = if menu.from.is_some() { "Add and connect" } else { "Add" };
                ui.label(theme::secondary(ui, title).small());
                let edit = ui.add(egui::TextEdit::singleline(&mut query).hint_text("Search").desired_width(f32::INFINITY));
                if menu.focus {
                    edit.request_focus();
                }
                ui.add_space(4.0);
                if list.is_empty() {
                    ui.label(theme::secondary(ui, "Nothing matches"));
                }
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for (k, (item, name)) in list.iter().enumerate() {
                        let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
                        if resp.hovered() {
                            index = k;
                        }
                        if k == index {
                            ui.painter().rect_filled(row, CornerRadius::same(6), p.bg_control_hover);
                            if resp.rect.height() > 0.0 && menu.index != index {
                                resp.scroll_to_me(None);
                            }
                        }
                        let icon = Rect::from_min_size(row.min + vec2(6.0, 5.0), vec2(18.0, 18.0));
                        paint_item_icon(ui.painter(), icon, *item, p.text, p.accent);
                        ui.painter().text(pos2(icon.right() + 10.0, row.center().y), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(13.0), p.text);
                        if resp.clicked() {
                            picked = Some(*item);
                        }
                    }
                });
            });
    });

    // A click anywhere else closes it.
    let clicked_outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|q| !area.response.rect.contains(q)));
    if close || clicked_outside {
        ed.insert = None;
        return;
    }
    if let Some(item) = picked {
        ed.insert_item(item);
        return;
    }
    if let Some(m) = ed.insert.as_mut() {
        if query != m.query {
            index = 0;
        }
        m.query = query;
        m.index = index;
        m.focus = false;
    }
}

pub(crate) fn paint_item_icon(painter: &egui::Painter, r: Rect, item: Item, c: Color32, dot: Color32) {
    match item {
        Item::Shape(s) => {
            let r = if matches!(s, Shape::Connector) { Rect::from_center_size(r.center(), vec2(r.height(), r.height()) * 0.85) } else { r.shrink2(vec2(0.0, r.height() * 0.18)) };
            paint_shape(painter, s, r, 0.3, Color32::TRANSPARENT, Stroke::new(1.3, c));
        }
        Item::Text => tools::paint_icon(painter, r, Tool::Text, c, dot),
        Item::Image => tools::paint_image_icon(painter, r, c, dot),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Kind;

    #[test]
    fn filter_finds_by_name_and_keywords() {
        assert_eq!(filter("").len(), items().len());
        assert_eq!(filter("dec")[0].1, "Decision");
        assert!(filter("yes").iter().any(|(_, n)| *n == "Decision"));
        assert!(filter("sticky").iter().any(|(_, n)| *n == "Note"));
        assert!(filter("zzz").is_empty());
    }

    #[test]
    fn slash_adds_a_connected_item_to_the_right_and_steps_down() {
        let mut ed = Editor::default();
        let a = ed.board.add(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 60.0)), Kind::Shape { shape: Shape::Process }, Style::default());
        ed.selection = vec![a];
        ed.open_insert_here();
        ed.insert_item(Item::Shape(Shape::Process));
        let b = ed.selection[0];
        let rb = ed.board.get(b).unwrap().rect();
        assert!(rb.left() >= 100.0 + GAP - 0.1);
        assert!(ed.board.wires.iter().any(|w| w.from == a && w.to == b && w.from_side == Side::Right && w.to_side == Side::Left));
        // Again from the same node: the second child goes below the first, not on top of it.
        ed.finish_edit();
        ed.selection = vec![a];
        ed.open_insert_here();
        ed.insert_item(Item::Shape(Shape::Decision));
        let c = ed.selection[0];
        assert!(ed.board.get(c).unwrap().rect().top() >= rb.bottom());
    }

    #[test]
    fn dropping_a_wire_places_the_item_at_the_drop_point() {
        let mut ed = Editor::default();
        let a = ed.board.add(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 60.0)), Kind::Shape { shape: Shape::Process }, Style::default());
        ed.open_insert_from_wire(a, Some(Side::Bottom), pos2(50.0, 300.0));
        ed.insert_item(Item::Shape(Shape::Node));
        let b = ed.selection[0];
        let r = ed.board.get(b).unwrap().rect();
        // Dropped below: the new item hangs from the drop point by its top port.
        assert_eq!(r.center_top(), pos2(50.0, 300.0));
        assert!(ed.board.wires.iter().any(|w| w.from == a && w.to == b && w.to_side == Side::Top));
    }
}
