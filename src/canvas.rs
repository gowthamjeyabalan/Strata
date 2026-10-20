//! The infinite board: camera, pointer gestures for each tool, and drawing.

use crate::board::{self, Brush, Id, Kind, Object, Shape, Side, Style};
use crate::editor::{Editor, Pick};
use crate::shapes;
use crate::theme::{self, Pal, pal};
use crate::tools::Tool;
use egui::{
    Align, Color32, CornerRadius, CursorIcon, FontId, Key, Modifiers, Painter, PointerButton, Pos2, Rect, Sense, Stroke,
    StrokeKind, TextureOptions, Ui, Vec2, pos2, vec2,
};
use serde::{Deserialize, Serialize};

const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 16.0;
/// Pointer tolerance for hits, in screen pixels.
const HIT_PX: f32 = 6.0;
const PORT_PX: f32 = 4.5;
const HANDLE_PX: f32 = 9.0;
/// How near the first point of a polyline a click closes it, in screen pixels.
const CLOSE_PX: f32 = 8.0;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Camera {
    /// Screen offset of the world origin from the canvas' top-left corner.
    pub pan: [f32; 2],
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { pan: [400.0, 300.0], zoom: 1.0 }
    }
}

impl Camera {
    fn pan_v(&self) -> Vec2 {
        vec2(self.pan[0], self.pan[1])
    }
    pub fn to_screen(&self, origin: Pos2, w: Pos2) -> Pos2 {
        origin + self.pan_v() + w.to_vec2() * self.zoom
    }
    pub fn to_world(&self, origin: Pos2, s: Pos2) -> Pos2 {
        ((s - origin - self.pan_v()) / self.zoom).to_pos2()
    }
    pub fn rect_to_screen(&self, origin: Pos2, r: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(origin, r.min), self.to_screen(origin, r.max))
    }
    pub fn pan_by(&mut self, d: Vec2) {
        self.pan[0] += d.x;
        self.pan[1] += d.y;
    }
    /// Zoom by `factor`, keeping the world point under `screen` fixed.
    pub fn zoom_at(&mut self, origin: Pos2, screen: Pos2, factor: f32) {
        let w = self.to_world(origin, screen);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let p = screen - origin - w.to_vec2() * self.zoom;
        self.pan = [p.x, p.y];
    }
    /// Put world point `w` at the centre of `view`.
    pub fn look_at(&mut self, view: Rect, w: Pos2) {
        let p = view.center() - view.min - w.to_vec2() * self.zoom;
        self.pan = [p.x, p.y];
    }
    pub fn fit(&mut self, view: Rect, world: Rect) {
        let z = (view.width() / world.width().max(1.0)).min(view.height() / world.height().max(1.0));
        self.zoom = z.clamp(MIN_ZOOM, 2.0);
        self.look_at(view, world.center());
    }
    pub fn centered(view: Rect) -> Self {
        let mut c = Camera { pan: [0.0, 0.0], zoom: 1.0 };
        c.look_at(view, Pos2::ZERO);
        c
    }
}

/// Which resize handle: the corners and the middle of each side.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Handle {
    NW,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
}

impl Handle {
    fn is_corner(self) -> bool {
        matches!(self, Handle::NW | Handle::NE | Handle::SE | Handle::SW)
    }
    fn cursor(self) -> CursorIcon {
        match self {
            Handle::NW | Handle::SE => CursorIcon::ResizeNwSe,
            Handle::NE | Handle::SW => CursorIcon::ResizeNeSw,
            Handle::N | Handle::S => CursorIcon::ResizeVertical,
            Handle::E | Handle::W => CursorIcon::ResizeHorizontal,
        }
    }
    /// Move this handle's edges of `r` to `w`, keeping the opposite side fixed.
    fn apply(self, r: Rect, w: Pos2, min: f32) -> Rect {
        let (mut l, mut t, mut rr, mut b) = (r.left(), r.top(), r.right(), r.bottom());
        if matches!(self, Handle::NW | Handle::W | Handle::SW) {
            l = w.x.min(rr - min);
        }
        if matches!(self, Handle::NE | Handle::E | Handle::SE) {
            rr = w.x.max(l + min);
        }
        if matches!(self, Handle::NW | Handle::N | Handle::NE) {
            t = w.y.min(b - min);
        }
        if matches!(self, Handle::SW | Handle::S | Handle::SE) {
            b = w.y.max(t + min);
        }
        Rect::from_min_max(pos2(l, t), pos2(rr, b))
    }
}

/// A small control drawn on an item.
#[derive(Clone, Copy)]
enum Control {
    Comments(Id),
    Collapse(Id),
}

/// The gesture in progress. Points are in world space.
#[derive(Default)]
pub enum Drag {
    #[default]
    None,
    Pan,
    /// `bbox` is what is being moved, as it was at the start; `exclude` is it plus the groups around
    /// it, which snapping ignores.
    Move { start: Pos2, orig: Vec<(Id, Pos2)>, moved: bool, bbox: Rect, exclude: Vec<Id> },
    Resize { id: Id, orig: Rect, handle: Handle, aspect: Option<f32> },
    /// Dragging the corner-radius diamond.
    Radius { id: Id },
    Marquee { start: Pos2, end: Pos2, add: bool },
    Draw { points: Vec<Pos2> },
    Segment { start: Pos2, end: Pos2, arrow: bool },
    Box { start: Pos2, end: Pos2 },
    /// A new connection. `from_side` is `None` when it started on the body rather than a port.
    Wire { from: Id, from_side: Option<Side>, end: Pos2 },
    Erase { erased: bool },
}

fn snap45(start: Pos2, end: Pos2) -> Pos2 {
    let d = end - start;
    let a = (d.y.atan2(d.x) / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
    start + vec2(a.cos(), a.sin()) * d.length()
}

fn box_rect(start: Pos2, end: Pos2, square: bool) -> Rect {
    let mut end = end;
    if square {
        let d = end - start;
        let s = d.x.abs().max(d.y.abs());
        end = start + vec2(s * d.x.signum(), s * d.y.signum());
    }
    Rect::from_two_pos(start, end)
}

const TEXT_WIDTH: f32 = 240.0;

fn quantize_font(px: f32) -> f32 {
    ((px * 2.0).round() / 2.0).max(1.0)
}

impl Editor {
    fn tol(&self) -> f32 {
        HIT_PX / self.camera.zoom
    }

    /// The port under a world point, topmost object first.
    fn port_hit(&self, w: Pos2, except: Option<Id>) -> Option<(Id, Side)> {
        let r = (PORT_PX + 5.0) / self.camera.zoom;
        let hidden = self.board.hidden_set();
        let inner = self.board.sketch_inner();
        self.board
            .objects
            .iter()
            .rev()
            .filter(|o| Some(o.id) != except && !hidden.contains(&o.id) && !inner.contains(&o.id))
            .find_map(|o| Side::ALL.into_iter().find(|s| o.port(*s).distance(w) <= r).map(|s| (o.id, s)))
    }

    /// Screen centres of the eight resize handles, just outside the item so they sit clear of its ports.
    fn handles(&self, o: &Object) -> [(Handle, Pos2); 8] {
        let r = self.camera.rect_to_screen(self.canvas_rect.min, o.rect()).expand(6.0);
        let c = r.center();
        [
            (Handle::NW, r.left_top()),
            (Handle::N, pos2(c.x, r.top())),
            (Handle::NE, r.right_top()),
            (Handle::E, pos2(r.right(), c.y)),
            (Handle::SE, r.right_bottom()),
            (Handle::S, pos2(c.x, r.bottom())),
            (Handle::SW, r.left_bottom()),
            (Handle::W, pos2(r.left(), c.y)),
        ]
    }

    fn resizable(o: &Object) -> bool {
        !o.locked && !matches!(o.kind, Kind::Text | Kind::Group { sketch: false, .. })
    }

    /// The single selected shape whose corners can be rounded, with its corner radius in world units.
    fn radius_target(&self) -> Option<(Id, Shape, f32)> {
        if self.selection.len() != 1 {
            return None;
        }
        let o = self.board.get(self.selection[0]).filter(|o| !o.locked)?;
        let Kind::Shape { shape } = o.kind else { return None };
        if !shapes::has_corners(shape) {
            return None;
        }
        let r = o.rect();
        let rad = o.style.radius.unwrap_or_else(|| shapes::default_radius(shape)).min(r.width().min(r.height()) * 0.5);
        Some((o.id, shape, rad))
    }

    /// Screen position of the corner-radius diamond: inside the top-left corner, as far in as the radius.
    fn radius_handle(&self) -> Option<(Id, Pos2)> {
        let (id, _, rad) = self.radius_target()?;
        let r = self.camera.rect_to_screen(self.canvas_rect.min, self.board.get(id)?.rect());
        let along = (rad * self.camera.zoom).max(12.0).min(r.width() * 0.5);
        Some((id, pos2(r.left() + along, r.top() + 10.0)))
    }

    // ── small controls drawn on items: the comment bubble and a group's collapse arrow ──

    /// Screen rect of an item's comment bubble, just outside its top-right corner.
    fn comment_rect(&self, o: &Object) -> Rect {
        let tr = self.camera.to_screen(self.canvas_rect.min, o.rect().right_top());
        Rect::from_center_size(tr + vec2(24.0, -14.0), vec2(20.0, 17.0))
    }

    /// The bubble shows when an item has comments, or when it is hovered or selected (to add one).
    fn shows_comment_icon(&self, o: &Object, hovered: Option<Id>) -> bool {
        !o.comments.is_empty() || hovered == Some(o.id) || (self.selection.len() == 1 && self.selection[0] == o.id)
    }

    /// Screen rect of a group's collapse arrow, left of its name.
    fn chevron_rect(&self, o: &Object) -> Rect {
        let r = self.camera.rect_to_screen(self.canvas_rect.min, o.rect());
        let z = self.camera.zoom;
        let size = (o.style.font_size * z).clamp(10.0, 40.0);
        Rect::from_min_size(r.min + vec2(10.0, 9.0) * z, vec2(size, size))
    }

    /// A click on a comment bubble or a collapse arrow (screen position).
    fn control_hit(&self, s: Pos2) -> Option<Control> {
        let hidden = self.board.hidden_set();
        let inner = self.board.sketch_inner();
        let hovered = self.hover_world.and_then(|w| self.board.hit(w, self.tol()));
        for o in self.board.objects.iter().rev().filter(|o| !hidden.contains(&o.id) && !inner.contains(&o.id)) {
            if self.shows_comment_icon(o, hovered) && self.comment_rect(o).expand(2.0).contains(s) {
                return Some(Control::Comments(o.id));
            }
        }
        for o in self.board.objects.iter().rev().filter(|o| o.is_group() && !o.is_sketch() && !hidden.contains(&o.id)) {
            if self.chevron_rect(o).expand(3.0).contains(s) {
                return Some(Control::Collapse(o.id));
            }
        }
        None
    }

    /// A resize handle under a world point (only for a single selected item).
    fn resize_handle_hit(&self, w: Pos2) -> Option<(Id, Handle)> {
        if self.selection.len() != 1 || self.is_editing() {
            return None;
        }
        let o = self.board.get(self.selection[0]).filter(|o| Self::resizable(o))?;
        let s = self.camera.to_screen(self.canvas_rect.min, w);
        self.handles(o).into_iter().find(|(_, p)| (*p - s).abs().max_elem() <= HANDLE_PX * 0.5 + 2.0).map(|(h, _)| (o.id, h))
    }

    fn radius_handle_hit(&self, w: Pos2) -> Option<Id> {
        let s = self.camera.to_screen(self.canvas_rect.min, w);
        self.radius_handle().filter(|(_, p)| p.distance(s) <= 7.0).map(|(id, _)| id)
    }

    fn wire_hit(&self, w: Pos2) -> Option<Id> {
        let tol = self.tol();
        self.board.wires.iter().rev().find(|wire| {
            self.board.wire_points(wire).is_some_and(|pts| pts.windows(2).any(|s| board::dist_to_segment(w, s[0], s[1]) <= tol))
        }).map(|w| w.id)
    }

    /// Where a connection dropped at `end` would land: an exact port, or the side of the item under it
    /// that faces the start.
    fn wire_target(&self, from: Id, from_side: Option<Side>, end: Pos2) -> Option<(Id, Side)> {
        if let Some(hit) = self.port_hit(end, Some(from)) {
            return Some(hit);
        }
        let to = self.board.hit(end, self.tol()).filter(|t| *t != from)?;
        let a = self.board.get(from)?;
        let toward = from_side.map(|s| a.port(s)).unwrap_or(a.rect().center());
        Some((to, Side::facing(self.board.get(to)?.rect(), toward)))
    }

    /// The eyedropper: take a colour from what is under `w` (an image's pixel, or an item's colour) and
    /// give it to the selection's fill or line.
    fn pick_color(&mut self, w: Pos2) {
        let Some(target) = self.eyedropper.take() else { return };
        let Some(o) = self.board.hit(w, self.tol()).and_then(|id| self.board.get(id)).cloned() else {
            self.say("Nothing there to take a colour from");
            return;
        };
        let ink = match &o.kind {
            Kind::Image { image } => {
                let px = self.board.images.get(image).and_then(|a| image::load_from_memory(&a.bytes).ok()).map(|img| {
                    let r = o.rect();
                    let u = ((w.x - r.left()) / r.width()).clamp(0.0, 0.999);
                    let v = ((w.y - r.top()) / r.height()).clamp(0.0, 0.999);
                    let p = img.to_rgb8().get_pixel((u * img.width() as f32) as u32, (v * img.height() as f32) as u32).0;
                    board::Ink::Rgb(p)
                });
                match px {
                    Some(i) => i,
                    None => return,
                }
            }
            Kind::Shape { .. } if target == Pick::Fill && o.style.fill => o.style.fill_color.unwrap_or(o.style.ink),
            _ => o.style.ink,
        };
        let sel: Vec<Id> = self.selection.iter().copied().filter(|id| *id != o.id || self.selection.len() == 1).collect();
        if sel.is_empty() {
            return;
        }
        for id in sel {
            if let Some(x) = self.board.get_mut(id) {
                match target {
                    Pick::Fill => {
                        x.style.fill = true;
                        x.style.fill_color = Some(ink);
                    }
                    Pick::Line => {
                        x.style.ink = ink;
                        x.style.line = true;
                    }
                }
            }
        }
        self.mark_changed();
        self.say(if target == Pick::Fill { "Fill colour picked" } else { "Line colour picked" });
    }

    /// Remove strokes and lines under `w`. Returns true if anything was erased.
    fn erase_at(&mut self, w: Pos2) -> bool {
        let tol = 8.0 / self.camera.zoom;
        let hidden = self.board.hidden_set();
        let ids: Vec<Id> = self.board.objects.iter().filter(|o| matches!(o.kind, Kind::Path { .. }) && !o.locked && !hidden.contains(&o.id) && o.hit(w, tol)).map(|o| o.id).collect();
        if ids.is_empty() {
            return false;
        }
        self.board.remove(&ids);
        self.selection.retain(|id| !ids.contains(id));
        true
    }

    /// The shape the current tool makes, if it makes one.
    fn tool_shape(&self) -> Option<Shape> {
        match self.tool {
            Tool::Rectangle => Some(Shape::Rectangle),
            Tool::Ellipse => Some(Shape::Ellipse),
            Tool::Node => Some(self.node_shape),
            _ => None,
        }
    }

    fn create_box(&mut self, rect: Rect) {
        self.mark_changed();
        let mut style = self.style;
        let (kind, edit) = match self.tool_shape() {
            Some(shape) => {
                if self.tool == Tool::Node {
                    style.fill = true;
                }
                (Kind::Shape { shape }, shape.takes_label())
            }
            None => (Kind::Text, true),
        };
        let id = self.board.add(rect, kind, style);
        self.selection = vec![id];
        self.selected_wire = None;
        if edit {
            self.start_edit(id);
        }
    }

    /// Add a stroke or line. Freehand strokes (`brush` set) are smoothed and use the brush's own width.
    fn create_path(&mut self, mut points: Vec<Pos2>, arrow: bool, brush: Option<Brush>) {
        if points.len() == 1 {
            points.push(points[0]);
        }
        let mut style = self.style;
        if let Some(b) = brush {
            style.width = self.brush_widths[b.index()];
            if points.len() > 3 {
                points = board::smooth(&points, 2);
                // Smoothing quadruples the points; drop the ones within a quarter pixel of the line.
                points = board::simplify(&points, 0.25 / self.camera.zoom);
            }
        }
        let b = Rect::from_points(&points);
        let local = points.iter().map(|p| [p.x - b.min.x, p.y - b.min.y]).collect();
        self.mark_changed();
        self.board.add(b, Kind::Path { points: local, base: [b.width(), b.height()], arrow, brush: brush.unwrap_or_default(), closed: false, curved: false }, style);
    }

    // ── polyline: click to add corners, double-click or Enter to finish, click the first point to close ──

    fn poly_click(&mut self, w: Pos2, mods: Modifiers) {
        let w = match self.poly.last() {
            Some(last) if mods.shift => snap45(*last, w),
            _ => w,
        };
        if self.poly.len() >= 3 && self.poly[0].distance(w) * self.camera.zoom <= CLOSE_PX {
            self.finish_poly(true);
        } else {
            self.poly.push(w);
        }
    }

    /// Turn the points clicked so far into a line (a shape when `closed`).
    pub fn finish_poly(&mut self, closed: bool) {
        let mut pts = std::mem::take(&mut self.poly);
        let z = self.camera.zoom;
        // A double-click adds the same point twice.
        pts.dedup_by(|b, a| a.distance(*b) * z < 2.0);
        if pts.len() < 2 || (closed && pts.len() < 3) {
            return;
        }
        let b = Rect::from_points(&pts);
        let local = pts.iter().map(|p| [p.x - b.min.x, p.y - b.min.y]).collect();
        self.mark_changed();
        let kind = Kind::Path { points: local, base: [b.width(), b.height()], arrow: false, brush: Brush::Pen, closed, curved: self.poly_curved };
        let id = self.board.add(b, kind, self.style);
        self.selection = vec![id];
        self.selected_wire = None;
    }

    /// Enter finishes the polyline, Esc drops it, Backspace takes back the last point. Returns true if
    /// a key was used.
    pub fn poly_keys(&mut self, ctx: &egui::Context) -> bool {
        if self.tool != Tool::Polyline || self.poly.is_empty() {
            return false;
        }
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                self.finish_poly(false);
            } else if i.consume_key(Modifiers::NONE, Key::Escape) {
                self.poly.clear();
            } else if i.consume_key(Modifiers::NONE, Key::Backspace) || i.consume_key(Modifiers::NONE, Key::Delete) {
                self.poly.pop();
            }
        });
        true
    }

    fn begin_drag(&mut self, w: Pos2, mods: Modifiers, pan: bool) {
        if pan || self.tool == Tool::Hand {
            self.drag = Drag::Pan;
            return;
        }
        let tol = self.tol();
        self.drag = match self.tool {
            _ if self.eyedropper.is_some() => Drag::None,
            Tool::Select => {
                if let Some(id) = self.radius_handle_hit(w) {
                    Drag::Radius { id }
                } else if let Some((id, handle)) = self.resize_handle_hit(w) {
                    let o = self.board.get(id).unwrap();
                    // Images keep their shape when pulled by a corner (Shift frees them).
                    let aspect = (matches!(o.kind, Kind::Image { .. }) && handle.is_corner()).then(|| o.rect().aspect_ratio());
                    Drag::Resize { id, orig: o.rect(), handle, aspect }
                } else if let Some((id, side)) = self.port_hit(w, None) {
                    Drag::Wire { from: id, from_side: Some(side), end: w }
                } else if let Some(id) = self.board.hit(w, tol) {
                    if !self.selection.contains(&id) {
                        if mods.shift {
                            self.selection.push(id);
                        } else {
                            self.selection = vec![id];
                        }
                    }
                    self.selected_wire = None;
                    // Moving a group moves everything in it. Locked items stay where they are.
                    let unlocked: Vec<Id> = self.selection.iter().copied().filter(|id| !self.is_locked(*id)).collect();
                    let ids = self.board.with_descendants(&unlocked);
                    let orig: Vec<(Id, Pos2)> = ids.iter().filter(|id| !self.is_locked(**id)).filter_map(|id| self.board.get(*id).map(|o| (*id, o.rect().min))).collect();
                    if orig.is_empty() {
                        self.say("Locked items cannot be moved. Unlock them first (Ctrl+L).");
                    }
                    let bbox = unlocked.iter().filter_map(|id| self.board.get(*id)).map(|o| o.rect()).reduce(|a, b| a.union(b)).unwrap_or(Rect::NOTHING);
                    let exclude = self.snap_exclude(&unlocked);
                    Drag::Move { start: w, orig, moved: false, bbox, exclude }
                } else {
                    if !mods.shift {
                        self.clear_selection();
                    }
                    Drag::Marquee { start: w, end: w, add: mods.shift }
                }
            }
            Tool::Hand => Drag::Pan,
            Tool::Pen | Tool::Pencil | Tool::Marker | Tool::Highlighter => Drag::Draw { points: vec![w] },
            Tool::Eraser => Drag::Erase { erased: self.erase_at(w) },
            Tool::Line | Tool::Arrow => Drag::Segment { start: w, end: w, arrow: self.tool == Tool::Arrow },
            Tool::Polyline => Drag::None,
            Tool::Rectangle | Tool::Ellipse | Tool::Node | Tool::Text => Drag::Box { start: w, end: w },
            Tool::Connect => match self.port_hit(w, None) {
                Some((id, side)) => Drag::Wire { from: id, from_side: Some(side), end: w },
                None => match self.board.hit(w, tol) {
                    Some(id) => Drag::Wire { from: id, from_side: None, end: w },
                    None => Drag::None,
                },
            },
        };
    }

    /// Moving items, plus every group around them: snapping should not line things up with themselves.
    fn snap_exclude(&self, ids: &[Id]) -> Vec<Id> {
        let mut out = self.board.with_descendants(ids);
        for id in ids {
            let mut cur = *id;
            while let Some(p) = self.board.parent(cur) {
                out.push(p);
                cur = p;
            }
        }
        out
    }

    /// Rects to snap against: visible items near `around`, other than `exclude`.
    fn snap_targets(&self, exclude: &[Id], around: Rect) -> Vec<Rect> {
        let hidden = self.board.hidden_set();
        let reach = around.expand(1200.0 / self.camera.zoom);
        self.board
            .objects
            .iter()
            .filter(|o| !exclude.contains(&o.id) && !hidden.contains(&o.id) && !matches!(o.kind, Kind::Path { .. }))
            .map(|o| o.rect())
            .filter(|r| reach.intersects(*r))
            .collect()
    }

    fn update_drag(&mut self, w: Pos2, mods: Modifiers, screen_delta: Vec2) {
        let zoom = self.camera.zoom;
        let mut erase = false;
        // Snapping: on unless switched off, and Alt skips it for this drag.
        let snap_on = self.snapping && !mods.alt;
        let tol = 6.0 / zoom;
        let targets = match &self.drag {
            Drag::Move { exclude, bbox, .. } if snap_on => Some(self.snap_targets(exclude, *bbox)),
            Drag::Resize { id, orig, .. } if snap_on => Some(self.snap_targets(&self.snap_exclude(&[*id]), *orig)),
            _ => None,
        };
        self.guides.clear();
        match &mut self.drag {
            Drag::None => {}
            Drag::Pan => self.camera.pan_by(screen_delta),
            Drag::Move { start, orig, moved, bbox, .. } => {
                let mut d = w - *start;
                if mods.shift {
                    if d.x.abs() > d.y.abs() { d.y = 0.0 } else { d.x = 0.0 }
                }
                if let Some(t) = &targets {
                    let (adj, guides) = crate::snap::snap_move(bbox.translate(d), t, tol);
                    // Shift keeps the drag on one axis; only snap along it.
                    let adj = if mods.shift { if d.x == 0.0 { vec2(0.0, adj.y) } else { vec2(adj.x, 0.0) } } else { adj };
                    d += adj;
                    self.guides = guides;
                }
                for (id, o) in orig.iter() {
                    if let Some(obj) = self.board.objects.iter_mut().find(|x| x.id == *id) {
                        obj.set_rect(Rect::from_min_size(*o + d, obj.rect().size()));
                    }
                }
                *moved = true;
            }
            Drag::Resize { id, orig, handle, aspect } => {
                let o0 = *orig;
                let mut r = handle.apply(o0, w, 4.0 / zoom);
                if let Some(a) = aspect.filter(|_| !mods.shift) {
                    let s = r.size();
                    let s = if s.x / s.y > a { vec2(s.y * a, s.y) } else { vec2(s.x, s.x / a) };
                    // Keep the corner opposite the handle where it was.
                    r = match handle {
                        Handle::NW => Rect::from_min_max(o0.max - s, o0.max),
                        Handle::NE => Rect::from_min_size(pos2(o0.min.x, o0.max.y - s.y), s),
                        Handle::SW => Rect::from_min_size(pos2(o0.max.x - s.x, o0.min.y), s),
                        _ => Rect::from_min_size(o0.min, s),
                    };
                } else if let Some(t) = &targets {
                    let h = *handle;
                    let edges = [
                        matches!(h, Handle::NW | Handle::W | Handle::SW),
                        matches!(h, Handle::NE | Handle::E | Handle::SE),
                        matches!(h, Handle::NW | Handle::N | Handle::NE),
                        matches!(h, Handle::SW | Handle::S | Handle::SE),
                    ];
                    let (snapped, guides) = crate::snap::snap_resize(r, edges, t, tol);
                    r = snapped;
                    self.guides = guides;
                }
                let id = *id;
                let before = self.board.get(id).map(|o| o.rect());
                if let Some(o) = self.board.objects.iter_mut().find(|x| x.id == id) {
                    o.set_rect(r);
                }
                // A sketch's strokes scale with it.
                if let Some(old) = before.filter(|_| self.board.get(id).is_some_and(|o| o.is_sketch())) {
                    let inner = self.board.with_descendants(&[id]);
                    let (sx, sy) = (r.width() / old.width().max(1e-3), r.height() / old.height().max(1e-3));
                    let map = |q: Pos2| pos2(r.min.x + (q.x - old.min.x) * sx, r.min.y + (q.y - old.min.y) * sy);
                    for o in self.board.objects.iter_mut().filter(|o| o.id != id && inner.contains(&o.id)) {
                        let m = o.rect();
                        o.set_rect(Rect::from_min_max(map(m.min), map(m.max)));
                    }
                }
            }
            Drag::Radius { id } => {
                if let Some(o) = self.board.objects.iter_mut().find(|x| x.id == *id) {
                    let r = o.rect();
                    o.style.radius = Some((w.x - r.left()).clamp(0.0, r.width().min(r.height()) * 0.5).round());
                }
            }
            Drag::Marquee { end, .. } => *end = w,
            Drag::Draw { points } => {
                if points.last().is_none_or(|l| l.distance(w) * zoom > 1.5) {
                    points.push(w);
                }
            }
            Drag::Segment { start, end, .. } => *end = if mods.shift { snap45(*start, w) } else { w },
            Drag::Box { end, .. } => *end = w,
            Drag::Wire { end, .. } => *end = w,
            Drag::Erase { .. } => erase = true,
        }
        if erase {
            let hit = self.erase_at(w);
            if let Drag::Erase { erased, .. } = &mut self.drag {
                *erased |= hit;
            }
        }
    }

    fn end_drag(&mut self, mods: Modifiers) {
        let zoom = self.camera.zoom;
        self.guides.clear();
        match std::mem::take(&mut self.drag) {
            Drag::None | Drag::Pan => {}
            Drag::Move { moved, orig, .. } => {
                if moved && !orig.is_empty() {
                    self.mark_changed();
                }
            }
            Drag::Resize { .. } | Drag::Radius { .. } => self.mark_changed(),
            Drag::Marquee { start, end, add } => {
                let r = Rect::from_two_pos(start, end);
                let hidden = self.board.hidden_set();
                let inner = self.board.sketch_inner();
                // Groups need to be fully inside the box; anything else (a sketch too) only has to touch it.
                let hits: Vec<Id> = self
                    .board
                    .objects
                    .iter()
                    .filter(|o| !hidden.contains(&o.id) && !inner.contains(&o.id))
                    .filter(|o| if o.is_group() && !o.is_sketch() { r.contains_rect(o.rect()) } else { r.intersects(o.rect().expand(0.5)) })
                    .map(|o| o.id)
                    .collect();
                if !add {
                    self.selection.clear();
                }
                for id in hits {
                    if !self.selection.contains(&id) {
                        self.selection.push(id);
                    }
                }
            }
            Drag::Draw { points } => self.create_path(points, false, self.tool.brush()),
            Drag::Segment { start, end, arrow } => {
                if start.distance(end) * zoom >= 3.0 {
                    self.create_path(vec![start, end], arrow, None);
                }
            }
            Drag::Box { start, end } => {
                let r = box_rect(start, end, mods.shift);
                if r.width() * zoom < 6.0 && r.height() * zoom < 6.0 {
                    self.click(start, mods);
                } else if self.tool == Tool::Text {
                    let h = self.style.font_size * 1.4;
                    self.create_box(Rect::from_min_size(r.min, vec2(r.width().max(40.0), h)));
                } else {
                    self.create_box(r);
                }
            }
            Drag::Wire { from, from_side, end } => {
                if let Some((to, to_side)) = self.wire_target(from, from_side, end) {
                    let from_side = from_side.unwrap_or_else(|| Side::facing(self.board.get(from).unwrap().rect(), self.board.get(to).unwrap().port(to_side)));
                    match self.board.connect(from, from_side, to, to_side, self.wire_style) {
                        Some(wid) => {
                            self.mark_changed();
                            self.selection.clear();
                            self.selected_wire = Some(wid);
                            self.say(format!("Connected {} to {}", self.board.display_name(from), self.board.display_name(to)));
                        }
                        None => self.say("Those ports are already connected, or one is inside the other"),
                    }
                } else if self.board.get(from).is_some_and(|o| !o.rect().expand(20.0 / zoom).contains(end)) {
                    // Dropped on empty board: offer the list of items to create there, connected.
                    let side = from_side.or_else(|| self.board.get(from).map(|o| Side::facing(o.rect(), end)));
                    self.open_insert_from_wire(from, side, end);
                }
            }
            Drag::Erase { erased } => {
                if erased {
                    self.mark_changed();
                }
            }
        }
    }

    fn click(&mut self, w: Pos2, mods: Modifiers) {
        let tol = self.tol();
        match self.tool {
            Tool::Select | Tool::Connect => {
                let s = self.camera.to_screen(self.canvas_rect.min, w);
                let target = self.board.hit(w, tol).and_then(|id| match self.board.get(id).map(|o| &o.kind) {
                    Some(Kind::Note { note }) => self.note_view.borrow().get(&id).and_then(|v| v.1.iter().find(|(r, _)| r.contains(s)).map(|(_, t)| (*note, t.clone()))),
                    _ => None,
                });
                match target {
                    Some((note, crate::markdown::Target::Task(line))) => return self.toggle_note_task(note, line),
                    Some((_, crate::markdown::Target::Link(l))) => return self.follow_link(&l),
                    None => {}
                }
                if let Some(id) = self.board.hit(w, tol) {
                    if mods.shift || mods.command {
                        if let Some(i) = self.selection.iter().position(|x| *x == id) {
                            self.selection.remove(i);
                        } else {
                            self.selection.push(id);
                        }
                    } else {
                        self.selection = vec![id];
                    }
                    self.selected_wire = None;
                } else if let Some(wire) = self.wire_hit(w) {
                    self.selection.clear();
                    self.selected_wire = Some(wire);
                } else {
                    self.clear_selection();
                }
            }
            Tool::Pen | Tool::Pencil | Tool::Marker | Tool::Highlighter => self.create_path(vec![w], false, self.tool.brush()),
            Tool::Eraser => {
                if self.erase_at(w) {
                    self.mark_changed();
                }
            }
            Tool::Text => {
                if let Some(id) = self.board.hit(w, tol).filter(|id| matches!(self.board.get(*id).map(|o| &o.kind), Some(Kind::Text))) {
                    self.start_edit(id);
                } else {
                    let s = vec2(TEXT_WIDTH, self.style.font_size * 1.4);
                    self.create_box(Rect::from_min_size(w - vec2(0.0, s.y * 0.5), s));
                }
            }
            Tool::Rectangle | Tool::Ellipse | Tool::Node => {
                let size = self.tool_shape().map(|s| s.default_size()).unwrap_or(vec2(140.0, 90.0));
                self.create_box(Rect::from_center_size(w, size));
            }
            Tool::Polyline => self.poly_click(w, mods),
            Tool::Hand | Tool::Line | Tool::Arrow => {}
        }
    }

    fn double_click(&mut self, w: Pos2) {
        if self.tool == Tool::Polyline {
            self.finish_poly(false);
            return;
        }
        if let Some(Kind::Note { note }) = self.board.hit(w, self.tol()).and_then(|id| self.board.get(id)).map(|o| o.kind.clone()) {
            self.open_note = Some(note);
            return;
        }
        if self.tool != Tool::Select {
            return;
        }
        if let Some(id) = self.board.hit(w, self.tol()) {
            self.start_edit(id);
        } else if let Some(wid) = self.wire_hit(w) {
            self.start_wire_edit(wid);
        } else {
            self.mark_changed();
            let s = vec2(TEXT_WIDTH, self.style.font_size * 1.4);
            let style = Style { fill: false, ..self.style };
            let id = self.board.add(Rect::from_min_size(w - vec2(0.0, s.y * 0.5), s), Kind::Text, style);
            self.start_edit(id);
        }
    }

    pub(crate) fn texture(&mut self, ctx: &egui::Context, image: Id) -> Option<egui::TextureId> {
        if !self.textures.contains_key(&image) {
            let tex = self.board.images.get(&image).and_then(|a| image::load_from_memory(&a.bytes).ok()).map(|img| {
                let img = if img.width().max(img.height()) > 4096 { img.resize(4096, 4096, image::imageops::FilterType::Triangle) } else { img };
                let rgba = img.to_rgba8();
                let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
                ctx.load_texture(format!("image-{image}"), ci, TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)))
            });
            self.textures.insert(image, tex);
        }
        self.textures.get(&image).and_then(|t| t.as_ref().map(|t| t.id()))
    }

    /// The canvas widget. Fills all the space it is given.
    pub fn canvas_ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let p = pal(&ctx);
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        self.canvas_rect = rect;
        self.canvas_visible = true;
        let origin = rect.min;
        if self.fit_pending {
            self.fit_pending = false;
            self.zoom_to_fit();
        }

        // ── input ──
        let (ptr, primary_down, primary_pressed, mods, space, scroll, zoom_delta) = ui.input(|i| {
            (i.pointer.hover_pos(), i.pointer.primary_down(), i.pointer.primary_pressed(), i.modifiers, i.key_down(Key::Space), i.smooth_scroll_delta(), i.zoom_delta())
        });
        let typing = ctx.egui_wants_keyboard_input();
        let space_pan = space && !typing;
        let over = ptr.filter(|p| rect.contains(*p) && resp.contains_pointer());
        self.hover_world = over.map(|s| self.camera.to_world(origin, s));

        let mut scroll = scroll;
        if let (Some(s), true) = (over, scroll.y != 0.0 && !mods.command) {
            let w = self.camera.to_world(origin, s);
            if let Some(id) = self.board.hit(w, self.tol()).filter(|id| matches!(self.board.get(*id).map(|o| &o.kind), Some(Kind::Note { .. }))) {
                let view_h = self.board.get(id).map(|o| o.rect().height() - 28.0).unwrap_or(0.0);
                let content = self.note_view.borrow().get(&id).map(|v| v.0).unwrap_or(0.0);
                if content > view_h {
                    let cur = self.note_scroll.entry(id).or_insert(0.0);
                    *cur = (*cur - scroll.y / self.camera.zoom).clamp(0.0, content - view_h);
                    scroll.y = 0.0;
                }
            }
        }
        if let Some(s) = over {
            let factor = zoom_delta * (scroll.y * 0.0018).exp();
            if (factor - 1.0).abs() > 1e-4 {
                self.camera.zoom_at(origin, s, factor);
            }
            if scroll.x != 0.0 {
                self.camera.pan_by(vec2(scroll.x, 0.0));
            }
        }
        // Holding the right button on an item opens the radial menu for the selection. On empty board
        // it pans, as the middle button does, unless it is held still for a moment (or just clicked):
        // then the menu opens there.
        let right_pressed = ui.input(|i| i.pointer.button_pressed(PointerButton::Secondary));
        if right_pressed && self.radial.is_none() && !space_pan {
            if let Some(s) = over {
                let w = self.camera.to_world(origin, s);
                if let Some(id) = self.board.hit(w, self.tol()) {
                    self.finish_edit();
                    if !self.selection.contains(&id) {
                        self.selection = vec![id];
                    }
                    self.selected_wire = None;
                    self.radial = Some(crate::radial::Radial::new(s, w, true));
                } else {
                    self.radial_hold = Some((s, w, std::time::Instant::now()));
                }
            }
        }
        if let Some((s, w, t)) = self.radial_hold {
            let down = ui.input(|i| i.pointer.secondary_down());
            if ptr.is_some_and(|q| q.distance(s) > 6.0) {
                self.radial_hold = None;
            } else if !down {
                self.radial_hold = None;
                self.finish_edit();
                self.radial = Some(crate::radial::Radial::new(s, w, false).sticky());
            } else if t.elapsed().as_secs_f32() >= 0.3 {
                self.radial_hold = None;
                self.finish_edit();
                self.radial = Some(crate::radial::Radial::new(s, w, false));
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(40));
            }
        }
        let in_radial = self.radial.is_some();
        // A ring opened from the Notes list is run by the app, so it works whichever tab is showing.
        if !self.radial.as_ref().is_some_and(|r| r.from_list()) {
            crate::radial::update(self, &ctx);
        }
        // Pressing on the board makes Delete act on the board again, not the Notes list.
        if primary_pressed && over.is_some() {
            self.notes_focus = false;
        }
        if resp.dragged_by(PointerButton::Middle) || (resp.dragged_by(PointerButton::Secondary) && !in_radial) {
            self.camera.pan_by(resp.drag_delta());
        }

        // A click outside the text being edited only ends the edit.
        if self.is_editing() && primary_pressed {
            if let (Some(s), Some(er)) = (over, self.edit_rect) {
                if !er.expand(4.0).contains(s) {
                    self.finish_edit();
                    self.swallow = true;
                }
            }
        }
        let swallowing = self.swallow;
        if swallowing && !primary_down {
            self.swallow = false;
        }

        // Comment bubbles and collapse arrows take the click before any tool does.
        let press = ui.input(|i| i.pointer.press_origin()).or(ptr);
        let on_control = !space_pan && press.and_then(|s| self.control_hit(s)).is_some();
        if !swallowing && self.eyedropper.is_some() {
            // The eyedropper takes the next click.
            if resp.clicked() {
                if let Some(s) = ptr {
                    let w = self.camera.to_world(origin, s);
                    self.pick_color(w);
                }
            }
        } else if !swallowing && on_control {
            if resp.clicked() {
                match press.and_then(|s| self.control_hit(s)) {
                    Some(Control::Comments(id)) => {
                        self.comments_open = if self.comments_open == Some(id) { None } else { Some(id) };
                        self.comment_draft.clear();
                    }
                    Some(Control::Collapse(id)) => self.toggle_collapsed(id),
                    None => {}
                }
            }
        } else if !swallowing {
            if resp.drag_started_by(PointerButton::Primary) {
                let start = press.unwrap_or(rect.center());
                let w = self.camera.to_world(origin, start);
                self.begin_drag(w, mods, space_pan);
            }
            if resp.dragged_by(PointerButton::Primary) {
                if let Some(s) = ptr {
                    let w = self.camera.to_world(origin, s);
                    self.update_drag(w, mods, resp.drag_delta());
                }
            }
            if resp.drag_stopped_by(PointerButton::Primary) {
                self.end_drag(mods);
            }
            if resp.clicked() && !space_pan {
                if let Some(s) = ptr {
                    let w = self.camera.to_world(origin, s);
                    self.click(w, mods);
                }
            }
            if resp.double_clicked() && !space_pan {
                if let Some(s) = ptr {
                    let w = self.camera.to_world(origin, s);
                    self.double_click(w);
                }
            }
        }

        if let Some(url) = self.open_url.take() {
            ctx.open_url(egui::OpenUrl::new_tab(url));
        }
        // Groups follow their members.
        self.board.update_groups();
        if self.tool != Tool::Polyline {
            self.poly.clear();
        }

        // ── cursor ──
        if let Some(w) = self.hover_world {
            let icon = if over.is_some_and(|s| self.control_hit(s).is_some()) && !primary_down {
                CursorIcon::PointingHand
            } else if self.eyedropper.is_some() {
                CursorIcon::Crosshair
            } else if space_pan || self.tool == Tool::Hand || matches!(self.drag, Drag::Pan) {
                if primary_down { CursorIcon::Grabbing } else { CursorIcon::Grab }
            } else if let Drag::Resize { handle, .. } = self.drag {
                handle.cursor()
            } else {
                match self.tool {
                    Tool::Select if self.radius_handle_hit(w).is_some() || matches!(self.drag, Drag::Radius { .. }) => CursorIcon::ResizeHorizontal,
                    Tool::Select if self.resize_handle_hit(w).is_some() => self.resize_handle_hit(w).map(|(_, h)| h.cursor()).unwrap_or_default(),
                    Tool::Select if self.port_hit(w, None).is_some() => CursorIcon::Crosshair,
                    Tool::Select if matches!(self.drag, Drag::Move { .. }) => CursorIcon::Move,
                    Tool::Select => CursorIcon::Default,
                    Tool::Text => CursorIcon::Text,
                    _ => CursorIcon::Crosshair,
                }
            };
            ctx.set_cursor_icon(icon);
        }

        // ── draw ──
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius { nw: 0, ne: 0, sw: 10, se: 10 }, p.canvas);
        self.draw_grid(&painter, rect, p);
        let hovered = if matches!(self.tool, Tool::Select | Tool::Connect) && matches!(self.drag, Drag::None) {
            self.hover_world.and_then(|w| self.board.hit(w, self.tol()))
        } else {
            None
        };
        let look = self.board.lookup();
        self.paint_scene(&ctx, &painter, origin, &look);
        self.draw_day_marks(&painter, origin, p, &look);
        self.draw_ports(&painter, origin, p, hovered, &look);
        self.draw_selection(&painter, origin, p, hovered, &look);
        self.draw_controls(&painter, p, hovered, &look);
        self.draw_guides(&painter, origin, p);
        self.draw_preview(&painter, origin, p);

        if self.board.objects.is_empty() && matches!(self.drag, Drag::None) {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop images here or paste a screenshot. Hold the right mouse button for tools and new items.",
                FontId::proportional(13.0),
                p.text_secondary,
            );
        }

        self.edit_overlay(ui, origin, p);
    }

    /// Items and connections, as they appear on the board and in exports. Items inside collapsed groups
    /// are skipped.
    pub(crate) fn paint_scene(&mut self, ctx: &egui::Context, painter: &Painter, origin: Pos2, look: &board::Lookup) {
        let p = pal(ctx);
        // Free the GPU copies of images that are no longer on the board.
        if self.textures.len() > self.board.images.len() {
            let images = &self.board.images;
            self.textures.retain(|k, _| images.contains_key(k));
        }
        let mut text_heights = Vec::new();
        for k in 0..self.board.objects.len() {
            let (id, kind_image) = {
                let o = &self.board.objects[k];
                (o.id, if let Kind::Image { image } = o.kind { Some(image) } else { None })
            };
            if look.hidden(id) {
                continue;
            }
            let tex = kind_image.and_then(|img| self.texture(ctx, img));
            let o = &self.board.objects[k];
            if !painter.clip_rect().intersects(self.camera.rect_to_screen(origin, o.rect()).expand(40.0)) {
                continue;
            }
            if let Some(h) = self.draw_object(painter, origin, o, p, tex, self.editing == Some(id)) {
                text_heights.push((id, h));
            }
        }
        for (id, h) in text_heights {
            if let Some(o) = self.board.get_mut(id) {
                o.rect[3] = h;
            }
        }
        // Wires on top of objects so they are never hidden behind a group or an image.
        self.draw_wires(painter, origin, p, look);
    }

    /// A soft highlighter behind each item from the days chosen in the Calendar.
    fn draw_day_marks(&self, painter: &Painter, origin: Pos2, p: &Pal, look: &board::Lookup) {
        if self.day_direct.is_empty() {
            return;
        }
        for id in &self.day_direct {
            if look.hidden(*id) || look.in_sketch(*id) {
                continue;
            }
            let Some(o) = look.get(&self.board, *id) else { continue };
            let r = self.camera.rect_to_screen(origin, o.rect()).expand(7.0);
            painter.rect(r, 8.0, p.accent.gamma_multiply(0.08), Stroke::new(2.0, p.accent.gamma_multiply(0.7)), StrokeKind::Outside);
        }
    }

    /// Only the ports that have connections, as in an export.
    pub(crate) fn paint_wired_ports(&self, painter: &Painter, origin: Pos2, p: &Pal, look: &board::Lookup) {
        self.draw_ports(painter, origin, p, None, look);
    }

    /// Snapping guides: lines where edges or centres line up, and measured spans for equal gaps.
    fn draw_guides(&self, painter: &Painter, origin: Pos2, p: &Pal) {
        let stroke = Stroke::new(1.0, p.accent);
        for g in &self.guides {
            match *g {
                crate::snap::Guide::Line(a, b) => {
                    let (a, b) = (self.camera.to_screen(origin, a), self.camera.to_screen(origin, b));
                    let dir = (b - a).normalized();
                    painter.line_segment([a - dir * 8.0, b + dir * 8.0], stroke);
                }
                crate::snap::Guide::Gap(a, b) => {
                    let (a, b) = (self.camera.to_screen(origin, a), self.camera.to_screen(origin, b));
                    let n = vec2(-(b - a).normalized().y, (b - a).normalized().x) * 4.0;
                    painter.line_segment([a, b], stroke);
                    painter.line_segment([a - n, a + n], stroke);
                    painter.line_segment([b - n, b + n], stroke);
                }
            }
        }
    }

    /// Comment bubbles, collapse arrows and lock marks.
    fn draw_controls(&self, painter: &Painter, p: &Pal, hovered: Option<Id>, look: &board::Lookup) {
        let ptr = painter.ctx().input(|i| i.pointer.hover_pos());
        for o in self.board.objects.iter().filter(|o| !look.hidden(o.id) && !look.in_sketch(o.id)) {
            let r = self.camera.rect_to_screen(self.canvas_rect.min, o.rect());
            if !painter.clip_rect().intersects(r.expand(30.0)) {
                continue;
            }
            if o.is_group() && !o.is_sketch() {
                let c = self.chevron_rect(o);
                let collapsed = matches!(o.kind, Kind::Group { collapsed: true, .. });
                let color = if ptr.is_some_and(|q| c.expand(3.0).contains(q)) { p.accent } else { group_color(o, p).gamma_multiply(o.opacity) };
                chevron(painter, c, collapsed, color);
            }
            if self.shows_comment_icon(o, hovered) {
                let cr = self.comment_rect(o);
                let hot = ptr.is_some_and(|q| cr.expand(2.0).contains(q)) || self.comments_open == Some(o.id);
                comment_bubble(painter, cr, o.comments.len(), hot, p);
            }
            if o.locked && (hovered == Some(o.id) || self.selection.contains(&o.id)) {
                let tl = r.left_top() + vec2(-10.0, -10.0);
                padlock(painter, Rect::from_center_size(tl, vec2(14.0, 14.0)), p.text_secondary);
            }
        }
    }

    fn draw_grid(&self, painter: &Painter, rect: Rect, p: &Pal) {
        let mut step = 24.0;
        while step * self.camera.zoom < 18.0 {
            step *= 5.0;
        }
        let a = self.camera.to_world(rect.min, rect.min);
        let b = self.camera.to_world(rect.min, rect.max);
        let dot = p.hairline;
        let r = (1.0 * self.camera.zoom.sqrt()).clamp(0.7, 1.3);
        // All dots in one mesh: one shape for the whole grid instead of thousands.
        let mut mesh = egui::Mesh::default();
        let mut x = (a.x / step).floor() * step;
        while x <= b.x {
            let mut y = (a.y / step).floor() * step;
            while y <= b.y {
                let s = self.camera.to_screen(rect.min, pos2(x, y));
                mesh.add_colored_rect(Rect::from_center_size(s, vec2(r * 2.0, r * 2.0)), dot);
                y += step;
            }
            x += step;
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    fn draw_wires(&self, painter: &Painter, origin: Pos2, p: &Pal, look: &board::Lookup) {
        let z = self.camera.zoom;
        let width = (1.6 * z).clamp(1.0, 3.0);
        // Only connections that cross the visible area.
        let clip = painter.clip_rect().expand(20.0);
        let view = Rect::from_min_max(self.camera.to_world(origin, clip.min), self.camera.to_world(origin, clip.max));
        for w in &self.board.wires {
            let Some(pts) = look.wire_points(&self.board, w) else { continue };
            if !view.intersects(Rect::from_points(&pts)) {
                continue;
            }
            let selected = self.selected_wire == Some(w.id);
            let on = selected || self.selection.contains(&w.from) || self.selection.contains(&w.to);
            let color = if on { p.accent } else { p.text_secondary };
            let s = thin(pts.iter().map(|q| self.camera.to_screen(origin, *q)), 1.5);
            let end = s[s.len() - 1];
            painter.add(egui::Shape::line(s, Stroke::new(if selected { width + 1.0 } else { width }, color)));
            // The arrow stops at the port's edge so the port stays visible.
            let n = w.to_side.normal();
            let size = (7.0 * z.sqrt()).clamp(5.0, 10.0);
            arrow_tip(painter, end + n * (PORT_PX + 1.0), -n, size, color);
            if !w.label.is_empty() && self.editing_wire != Some(w.id) {
                let mid = self.camera.to_screen(origin, board::midpoint(&pts));
                let g = painter.layout_no_wrap(w.label.clone(), FontId::proportional(wire_font(z)), if on { p.accent_text } else { p.text });
                let r = Rect::from_center_size(mid, g.size() + vec2(12.0, 6.0));
                painter.rect(r, CornerRadius::same(7), p.canvas, Stroke::new(1.0, if on { p.accent } else { p.hairline }), StrokeKind::Inside);
                painter.galley(r.center() - g.size() * 0.5, g, p.text);
            }
        }
    }

    /// Draws one object. For text objects returns the measured world height so the rect can follow the text.
    fn draw_object(&self, painter: &Painter, origin: Pos2, o: &Object, p: &Pal, tex: Option<egui::TextureId>, editing: bool) -> Option<f32> {
        let z = self.camera.zoom;
        let r = self.camera.rect_to_screen(origin, o.rect());
        // Every colour of the item is faded by its opacity, and further while the Calendar highlights
        // other days' items.
        let faded = !self.day_shown.is_empty() && !self.day_shown.contains(&o.id);
        let alpha = o.opacity.clamp(0.05, 1.0) * if faded { 0.18 } else { 1.0 };
        let fade = |c: Color32| if alpha >= 1.0 { c } else { c.gamma_multiply(alpha) };
        let ink = fade(o.style.ink.color(p));
        let stroke = Stroke::new((o.style.width * z).max(0.75), ink);
        let font = FontId::proportional(quantize_font(o.style.font_size * z));
        match &o.kind {
            Kind::Image { .. } => {
                match tex {
                    Some(t) => {
                        painter.image(t, r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), fade(Color32::WHITE));
                    }
                    None => {
                        painter.rect(r, 6.0, p.faint_bg, Stroke::new(1.0, p.hairline), StrokeKind::Inside);
                        painter.text(r.center(), egui::Align2::CENTER_CENTER, "Image could not be shown", FontId::proportional(12.0), p.text_secondary);
                    }
                }
                self.draw_caption(painter, r, o, fade(p.text_secondary), editing);
            }
            Kind::Path { arrow, brush, closed, .. } => {
                let pts: Vec<Pos2> = o.line_points().into_iter().map(|q| self.camera.to_screen(origin, q)).collect();
                if *closed && o.style.fill {
                    let (look, _) = look_of(o, p, z, fade);
                    if let shapes::Fill::Solid(c) | shapes::Fill::Gradient(c, _) = look.fill {
                        shapes::fill_polygon(painter, &pts, c);
                    }
                }
                let line = Stroke::new(stroke.width, ink.gamma_multiply(brush.alpha()));
                if o.style.dash == board::Dash::Solid {
                    paint_stroke(painter, &pts, line, *brush);
                } else {
                    shapes::stroke_line(painter, &pts, false, line, o.style.dash);
                }
                if *arrow && pts.len() >= 2 {
                    let n = pts.len();
                    let dir = (pts[n - 1] - pts[n - 2]).normalized();
                    let size = (8.0 + o.style.width * 2.5) * z;
                    arrow_head(painter, pts[n - 1], dir, size, stroke);
                }
                self.draw_caption(painter, r, o, fade(p.text_secondary), editing);
            }
            Kind::Shape { shape } => {
                let (look, text) = look_of(o, p, z, fade);
                shapes::paint(painter, *shape, r, z, &look);
                if !editing && !o.label.is_empty() && font.size >= 2.0 {
                    let wrap = (r.width() - 16.0 * z).max(8.0);
                    let mut job = egui::text::LayoutJob::simple(o.label.clone(), font, text, wrap);
                    job.halign = Align::Center;
                    let g = painter.layout_job(job);
                    painter.galley(pos2(r.center().x, r.center().y - g.size().y * 0.5), g, text);
                }
            }
            Kind::Note { note } => {
                // A card: the note, formatted, scrolled inside its frame.
                let fill = match o.style.fill_color {
                    Some(c) if o.style.fill => fade(c.color(p)),
                    _ => fade(p.window_fill),
                };
                let border = if o.style.ink == board::Ink::Auto { fade(p.window_stroke) } else { ink };
                let radius = CornerRadius::same((10.0 * z).clamp(2.0, 24.0) as u8);
                painter.rect(r, radius, fill, Stroke::new(1.0, border), StrokeKind::Inside);
                let pad = 14.0 * z;
                let inner = r.shrink(pad);
                let scroll = self.note_scroll.get(&o.id).copied().unwrap_or(0.0) * z;
                let text_color = fade(if o.style.ink == board::Ink::Auto { p.text } else { ink });
                let (height, hits) = match self.board.note(*note) {
                    Some(n) if inner.width() > 4.0 && inner.height() > 4.0 => {
                        let clipped = painter.with_clip_rect(inner.expand(2.0).intersect(painter.clip_rect()));
                        let blocks = crate::markdown::parse(&n.text);
                        crate::markdown::paint(&clipped, &blocks, inner.min - vec2(0.0, scroll), inner.width(), z, p, text_color)
                    }
                    Some(_) => (0.0, Vec::new()),
                    None => {
                        painter.text(r.center(), egui::Align2::CENTER_CENTER, "This note was deleted", FontId::proportional((12.0 * z).max(1.0)), p.text_secondary);
                        (0.0, Vec::new())
                    }
                };
                // A thin bar shows where the view is when the note is longer than its card.
                if height > inner.height() + 1.0 {
                    let frac = inner.height() / height;
                    let track = Rect::from_min_max(pos2(r.right() - 5.0 * z, inner.top()), pos2(r.right() - 3.0 * z, inner.bottom()));
                    let bar_h = (track.height() * frac).max(12.0 * z);
                    let top = track.top() + (track.height() - bar_h) * (scroll / (height - inner.height()).max(1.0)).clamp(0.0, 1.0);
                    painter.rect_filled(Rect::from_min_size(pos2(track.left(), top), vec2(track.width().max(1.5), bar_h)), CornerRadius::same(2), p.text_secondary.gamma_multiply(0.5));
                }
                let hits = hits.into_iter().filter(|(hr, _)| inner.expand(2.0).contains(hr.center())).collect();
                self.note_view.borrow_mut().insert(o.id, (height / z, hits));
            }
            Kind::Text => {
                if editing {
                    return None;
                }
                let g = painter.layout(o.label.clone(), font, ink, r.width().max(8.0));
                let h = g.size().y / z;
                painter.galley(r.min, g, ink);
                if (h - o.rect[3]).abs() > 0.5 && h > 0.0 {
                    return Some(h);
                }
            }
            // A sketch's strokes draw themselves; it only adds its caption.
            Kind::Group { sketch: true, .. } => self.draw_caption(painter, r, o, fade(p.text_secondary), editing),
            Kind::Group { members, collapsed, .. } => {
                let c = fade(group_color(o, p));
                let gs = Stroke::new((o.style.width * z * 0.75).clamp(1.0, 4.0), c.gamma_multiply(0.85));
                let radius = CornerRadius::same((12.0 * z).clamp(2.0, 30.0) as u8);
                let size = quantize_font(o.style.font_size * z);
                if *collapsed {
                    // A compact node standing in for everything inside.
                    boxed(painter, r, radius, c.gamma_multiply(if o.style.fill { 0.18 } else { 0.06 }), Stroke::new(gs.width.max(1.5), c), StrokeKind::Inside);
                    if size >= 2.0 {
                        let n = self.board.with_descendants(members).iter().filter(|id| self.board.get(**id).is_some_and(|x| !x.is_group())).count();
                        let count = painter.layout_no_wrap(format!("{} inside", n), FontId::proportional(quantize_font(12.0 * z)), fade(p.text_secondary));
                        painter.galley(group_name_pos(r, z, size) + vec2(0.0, size * 1.35), count, p.text_secondary);
                    }
                } else {
                    let gfill = if o.style.fill { c.gamma_multiply(0.08) } else { Color32::TRANSPARENT };
                    boxed(painter, r, radius, gfill, gs, StrokeKind::Inside);
                }
                if !editing && !o.label.is_empty() && size >= 2.0 {
                    let name_font = FontId::new(size, theme::semibold_family());
                    let g = painter.layout(o.label.clone(), name_font, c, (r.width() - 44.0 * z).max(8.0));
                    painter.galley(group_name_pos(r, z, size), g, c);
                }
            }
        }
        None
    }

    /// Images and sketches show their label as a caption below.
    fn draw_caption(&self, painter: &Painter, r: Rect, o: &Object, color: Color32, editing: bool) {
        if editing || o.label.is_empty() {
            return;
        }
        let size = quantize_font(12.0 * self.camera.zoom.sqrt().clamp(0.6, 1.6));
        let mut job = egui::text::LayoutJob::simple(o.label.clone(), FontId::proportional(size), color, r.width().max(120.0));
        job.halign = Align::Center;
        let g = painter.layout_job(job);
        painter.galley(pos2(r.center().x, r.bottom() + 6.0), g, color);
    }

    fn draw_ports(&self, painter: &Painter, origin: Pos2, p: &Pal, hovered: Option<Id>, look: &board::Lookup) {
        let wiring = matches!(self.drag, Drag::Wire { .. });
        let show_all = self.tool == Tool::Connect || wiring;
        let hover_w = self.hover_world;
        let target = match &self.drag {
            Drag::Wire { from, from_side, end } => self.wire_target(*from, *from_side, *end),
            _ => None,
        };
        // Ports in use, with connections to hidden items moved onto the collapsed group showing them.
        let used: std::collections::HashSet<(Id, Side)> = self
            .board
            .wires
            .iter()
            .filter(|w| look.proxy(w.from) != look.proxy(w.to))
            .flat_map(|w| [(look.proxy(w.from), w.from_side), (look.proxy(w.to), w.to_side)])
            .collect();
        let selected: std::collections::HashSet<Id> = self.selection.iter().copied().collect();
        let clip = painter.clip_rect().expand(PORT_PX + 2.0);
        for o in self.board.objects.iter().filter(|o| !look.hidden(o.id) && !look.in_sketch(o.id)) {
            if !clip.intersects(self.camera.rect_to_screen(origin, o.rect())) {
                continue;
            }
            let active = show_all || hovered == Some(o.id) || selected.contains(&o.id);
            for side in Side::ALL {
                let wired = used.contains(&(o.id, side));
                if !(active || wired) {
                    continue;
                }
                let pos = o.port(side);
                let s = self.camera.to_screen(origin, pos);
                if !painter.clip_rect().contains(s) {
                    continue;
                }
                let near = hover_w.is_some_and(|h| h.distance(pos) * self.camera.zoom <= PORT_PX + 5.0);
                let is_target = target == Some((o.id, side));
                let color = if near || is_target { p.accent } else { p.text_secondary };
                let r = if near || is_target { PORT_PX + 1.5 } else { PORT_PX };
                painter.circle(s, r, if wired || is_target { color } else { p.canvas }, Stroke::new(1.5, color));
            }
        }
    }

    fn draw_selection(&self, painter: &Painter, origin: Pos2, p: &Pal, hovered: Option<Id>, look: &board::Lookup) {
        if let Some(id) = hovered.filter(|id| !self.selection.contains(id)) {
            if let Some(o) = self.board.get(id) {
                let r = self.camera.rect_to_screen(origin, o.rect()).expand(4.0);
                painter.rect_stroke(r, 6.0, Stroke::new(1.0, p.accent.gamma_multiply(0.45)), StrokeKind::Middle);
            }
        }
        for id in self.selection.iter().filter(|id| !look.hidden(**id)) {
            let Some(o) = look.get(&self.board, *id) else { continue };
            let r = self.camera.rect_to_screen(origin, o.rect()).expand(4.0);
            painter.rect_stroke(r, 6.0, Stroke::new(1.0, p.accent), StrokeKind::Middle);
        }
        if self.selection.len() == 1 && !self.is_editing() {
            if let Some(o) = self.board.get(self.selection[0]).filter(|o| Self::resizable(o)) {
                for (_, c) in self.handles(o) {
                    painter.rect(Rect::from_center_size(c, vec2(HANDLE_PX, HANDLE_PX)), 2.0, Color32::WHITE, Stroke::new(1.0, p.accent), StrokeKind::Middle);
                }
            }
            // The corner-radius diamond.
            if let Some((_, c)) = self.radius_handle() {
                let s = 5.0;
                let pts = vec![c + vec2(0.0, -s), c + vec2(s, 0.0), c + vec2(0.0, s), c + vec2(-s, 0.0)];
                painter.add(egui::Shape::convex_polygon(pts, Color32::WHITE, Stroke::new(1.0, p.accent)));
            }
        }
    }

    fn draw_preview(&self, painter: &Painter, origin: Pos2, p: &Pal) {
        let z = self.camera.zoom;
        let to_s = |w: Pos2| self.camera.to_screen(origin, w);
        let ink = self.style.ink.color(p);
        let stroke = Stroke::new((self.style.width * z).max(0.75), ink);
        // While the insert list is open for a dropped wire, keep showing that wire.
        if let Some(m) = &self.insert {
            if let (Some((from, side)), crate::insert::Place::At(at)) = (m.from, m.place) {
                if let Some(a) = self.board.get(from) {
                    let fs = side.unwrap_or_else(|| Side::facing(a.rect(), at));
                    let pts = board::route(a.port(fs), fs.normal(), at, None, self.wire_style);
                    painter.extend(egui::Shape::dashed_line(&pts.into_iter().map(to_s).collect::<Vec<_>>(), Stroke::new(2.0, p.accent), 6.0, 4.0));
                    painter.circle_filled(to_s(at), 4.0, p.accent);
                }
            }
        }
        if self.tool == Tool::Polyline && !self.poly.is_empty() {
            let mut pts = self.poly.clone();
            let mods = painter.ctx().input(|i| i.modifiers);
            let closing = self.hover_world.is_some_and(|h| pts.len() >= 3 && pts[0].distance(h) * z <= CLOSE_PX);
            if let Some(h) = self.hover_world.filter(|_| !closing) {
                pts.push(if mods.shift { snap45(pts[pts.len() - 1], h) } else { h });
            }
            let line = if self.poly_curved { board::catmull_rom(&pts, closing) } else if closing { pts.iter().copied().chain([pts[0]]).collect() } else { pts.clone() };
            let s: Vec<Pos2> = line.into_iter().map(to_s).collect();
            paint_stroke(painter, &s, stroke, Brush::Pen);
            for (k, q) in self.poly.iter().enumerate() {
                let r = if k == 0 && closing { 6.0 } else { 3.5 };
                painter.circle(to_s(*q), r, p.canvas, Stroke::new(1.5, p.accent));
            }
        }
        match &self.drag {
            Drag::Marquee { start, end, .. } => {
                let r = Rect::from_two_pos(to_s(*start), to_s(*end));
                painter.rect(r, 2.0, p.accent.gamma_multiply(0.12), Stroke::new(1.0, p.accent), StrokeKind::Middle);
            }
            Drag::Draw { points } => {
                if let Some(b) = self.tool.brush() {
                    let w = (self.brush_widths[b.index()] * z).max(0.5);
                    let pts: Vec<Pos2> = points.iter().map(|q| to_s(*q)).collect();
                    paint_stroke(painter, &pts, Stroke::new(w, ink.gamma_multiply(b.alpha())), b);
                }
            }
            Drag::Segment { start, end, arrow } => {
                let (a, b) = (to_s(*start), to_s(*end));
                painter.line_segment([a, b], stroke);
                if *arrow && a != b {
                    arrow_head(painter, b, (b - a).normalized(), (8.0 + self.style.width * 2.5) * z, stroke);
                }
            }
            Drag::Box { start, end } => {
                let mods = painter.ctx().input(|i| i.modifiers);
                let wr = box_rect(*start, *end, mods.shift);
                let r = Rect::from_two_pos(to_s(wr.min), to_s(wr.max));
                match self.tool_shape() {
                    Some(shape) => {
                        let f = if self.tool == Tool::Node { ink.gamma_multiply(0.16) } else { Color32::TRANSPARENT };
                        paint_shape(painter, shape, r, z, f, stroke);
                    }
                    None => {
                        painter.rect_stroke(r, 2.0, Stroke::new(1.0, p.accent), StrokeKind::Middle);
                    }
                }
            }
            Drag::Wire { from, from_side, end } => {
                if let Some(a) = self.board.get(*from) {
                    let target = self.wire_target(*from, *from_side, *end);
                    let (to, to_n) = match target.and_then(|(t, s)| self.board.get(t).map(|o| (o.port(s), s.normal()))) {
                        Some((pt, n)) => (pt, Some(n)),
                        None => (*end, None),
                    };
                    let fs = from_side.unwrap_or_else(|| Side::facing(a.rect(), to));
                    let pts = board::route(a.port(fs), fs.normal(), to, to_n, self.wire_style);
                    painter.add(egui::Shape::line(pts.into_iter().map(to_s).collect(), Stroke::new(2.0, p.accent)));
                    if let Some(t) = target.and_then(|(t, _)| self.board.get(t)) {
                        let r = self.camera.rect_to_screen(origin, t.rect()).expand(4.0);
                        painter.rect_stroke(r, 6.0, Stroke::new(1.0, p.accent), StrokeKind::Middle);
                    }
                }
            }
            _ => {}
        }
    }

    /// The text box shown while an object's or a connection's label is typed.
    fn edit_overlay(&mut self, ui: &mut Ui, origin: Pos2, p: &Pal) {
        let z = self.camera.zoom;
        let (key, area, font, color, center, mut label) = if let Some(wid) = self.editing_wire {
            let Some(w) = self.board.wire(wid) else {
                self.editing_wire = None;
                return;
            };
            let mid = self.camera.to_screen(origin, self.board.wire_points(w).map(|pts| board::midpoint(&pts)).unwrap_or_default());
            let font = FontId::proportional(wire_font(z));
            let area = Rect::from_center_size(mid, vec2(160.0, font.size * 1.4));
            (("wire", wid), area, font, p.text, true, w.label.clone())
        } else if let Some(id) = self.editing {
            let Some(o) = self.board.get(id) else {
                self.editing = None;
                return;
            };
            let r = self.camera.rect_to_screen(origin, o.rect());
            let ink = o.style.ink.color(p);
            let size = quantize_font((o.style.font_size * z).max(6.0));
            let (area, font, color, center) = match &o.kind {
                Kind::Text => (Rect::from_min_size(r.min, vec2(r.width().max(40.0), r.height().max(20.0))), FontId::proportional(size), ink, false),
                Kind::Shape { .. } => (r.shrink2(vec2(8.0 * z, 4.0 * z)), FontId::proportional(size), p.text, true),
                Kind::Group { sketch: false, .. } => {
                    let pos = group_name_pos(r, z, size);
                    (Rect::from_min_size(pos, vec2((r.width() - 24.0 * z).max(80.0), size * 1.3)), FontId::new(size, theme::semibold_family()), group_color(o, p), false)
                }
                _ => (Rect::from_center_size(pos2(r.center().x, r.bottom() + 16.0), vec2(r.width().max(160.0), 24.0)), FontId::proportional(13.0), p.text, true),
            };
            (("obj", id), area, font, color, center, o.label.clone())
        } else {
            return;
        };

        let edit_id = ui.make_persistent_id(key);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
        if center && self.editing.is_some() {
            // Vertically centre the text in shapes.
            let rows = label.lines().count().max(1) as f32;
            let h = rows * font_line_height(ui, &font);
            child.add_space(((area.height() - h) * 0.5).max(0.0));
        }
        if self.editing_wire.is_some() {
            child.painter().rect(area.expand2(vec2(6.0, 3.0)), CornerRadius::same(7), p.canvas, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        }
        let mut te = egui::TextEdit::multiline(&mut label).id(edit_id).frame(egui::Frame::NONE).font(font).text_color(color).desired_width(area.width()).desired_rows(1).margin(egui::Margin::ZERO);
        if center {
            te = te.horizontal_align(Align::Center);
        }
        let out = child.add(te);
        if self.edit_focus {
            out.request_focus();
            self.edit_focus = false;
        }
        self.edit_rect = Some(area.union(out.rect));
        if let Some(wid) = self.editing_wire {
            if let Some(w) = self.board.wire_mut(wid) {
                w.label = label.replace('\n', " ");
            }
        } else if let Some(id) = self.editing {
            if let Some(o) = self.board.get_mut(id) {
                if o.label != label {
                    o.label = if o.is_group() { label.replace('\n', " ") } else { label };
                }
                if matches!(o.kind, Kind::Text) {
                    let h = out.rect.height() / z;
                    if h > 0.0 {
                        o.rect[3] = h;
                    }
                }
            }
        }
        let escape = ui.input(|i| i.key_pressed(Key::Escape));
        if out.lost_focus() || escape {
            self.finish_edit();
        }
    }
}

fn wire_font(z: f32) -> f32 {
    quantize_font(12.0 * z.sqrt().clamp(0.7, 1.6))
}

/// A group's colour: its ink, or quiet grey when it follows the theme.
fn group_color(o: &Object, p: &Pal) -> Color32 {
    match o.style.ink {
        board::Ink::Auto => p.text_secondary,
        ink => ink.color(p),
    }
}

/// A rounded box drawn as two shapes. Given both at once, egui shrinks the fill's outline by half the
/// border width; at small corner radii that outline folds back on itself and leaves a faint seam.
fn boxed(painter: &Painter, r: Rect, radius: CornerRadius, fill: Color32, stroke: Stroke, kind: StrokeKind) {
    if fill != Color32::TRANSPARENT {
        painter.rect_filled(r, radius, fill);
    }
    painter.rect_stroke(r, radius, stroke, kind);
}

/// Where a group's name starts: right of its collapse arrow. `size` is the name's font size in pixels.
fn group_name_pos(r: Rect, z: f32, size: f32) -> Pos2 {
    r.min + vec2(10.0 * z + size.clamp(10.0, 40.0) + 4.0, 9.0 * z)
}

/// The collapse arrow: pointing right when collapsed, down when open.
fn chevron(painter: &Painter, r: Rect, collapsed: bool, color: Color32) {
    let c = r.center();
    let s = r.width() * 0.32;
    let pts = if collapsed {
        vec![c + vec2(-s * 0.6, -s), c + vec2(s * 0.8, 0.0), c + vec2(-s * 0.6, s)]
    } else {
        vec![c + vec2(-s, -s * 0.6), c + vec2(0.0, s * 0.8), c + vec2(s, -s * 0.6)]
    };
    painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// A speech bubble with the number of comments, or a plus when there are none yet.
fn comment_bubble(painter: &Painter, r: Rect, count: usize, hot: bool, p: &Pal) {
    let body = Rect::from_min_max(r.min, r.max - vec2(0.0, 4.0));
    let (fill, fg) = if hot {
        (p.accent, Color32::WHITE)
    } else if count > 0 {
        (p.bg_control_active, p.text_strong)
    } else {
        (p.canvas, p.text_secondary)
    };
    let stroke = Stroke::new(1.0, if count > 0 || hot { fill } else { p.text_secondary });
    painter.rect(body, CornerRadius::same(5), fill, stroke, StrokeKind::Inside);
    let tail = vec![pos2(body.left() + 4.0, body.bottom() - 0.5), pos2(body.left() + 10.0, body.bottom() - 0.5), pos2(body.left() + 4.0, r.bottom())];
    painter.add(egui::Shape::convex_polygon(tail, fill, Stroke::NONE));
    let label = if count == 0 { "+".to_owned() } else if count > 99 { "99+".to_owned() } else { count.to_string() };
    painter.text(body.center(), egui::Align2::CENTER_CENTER, label, FontId::new(10.0, theme::semibold_family()), fg);
}

fn padlock(painter: &Painter, r: Rect, color: Color32) {
    let body = Rect::from_min_max(pos2(r.left() + 1.0, r.center().y - 1.0), r.max);
    painter.rect_filled(body, CornerRadius::same(2), color);
    let shackle = ellipse_points(pos2(r.center().x, r.center().y - 1.0), r.width() * 0.28, r.height() * 0.38, std::f32::consts::PI, 2.0 * std::f32::consts::PI, 12);
    painter.add(egui::Shape::line(shackle, Stroke::new(1.5, color)));
}

fn font_line_height(ui: &Ui, font: &FontId) -> f32 {
    ui.fonts_mut(|f| f.row_height(font))
}

fn ellipse_points(c: Pos2, rx: f32, ry: f32, from: f32, to: f32, n: usize) -> Vec<Pos2> {
    (0..=n).map(|i| {
        let a = from + (to - from) * i as f32 / n as f32;
        pos2(c.x + rx * a.cos(), c.y + ry * a.sin())
    }).collect()
}

/// A shape with just a fill and an outline (tool icons, previews, the Overview).
pub fn paint_shape(painter: &Painter, shape: Shape, r: Rect, z: f32, fill: Color32, stroke: Stroke) {
    shapes::paint(painter, shape, r, z, &shapes::Look::simple(fill, stroke));
}

/// The full look of a shape item from its style, faded by `fade` (opacity).
fn look_of(o: &Object, p: &Pal, z: f32, fade: impl Fn(Color32) -> Color32) -> (shapes::Look, Color32) {
    let s = &o.style;
    let ink = fade(s.ink.color(p));
    let (fill, label) = if !s.fill {
        (shapes::Fill::None, fade(p.text))
    } else {
        match s.fill_color {
            // Automatic: a light tint of the line colour.
            None if s.gradient => (shapes::Fill::Gradient(ink.gamma_multiply(0.34), ink.gamma_multiply(0.06)), fade(p.text)),
            None => (shapes::Fill::Solid(ink.gamma_multiply(0.16)), fade(p.text)),
            Some(c) => {
                let c = c.color(p);
                // Labels stay readable on bright or dark fills.
                let label = fade(shapes::contrast_text(c));
                let c = fade(c);
                if s.gradient { (shapes::Fill::Gradient(c, c.lerp_to_gamma(fade(p.canvas), 0.65)), label) } else { (shapes::Fill::Solid(c), label) }
            }
        }
    };
    let look = shapes::Look {
        fill,
        stroke: s.line.then(|| Stroke::new((s.width * z).max(0.75), ink)),
        dash: s.dash,
        radius: s.radius.map(|r| r * z),
        shadow: s.shadow,
        glass: s.glass,
        sketch: s.sketch.then_some(o.id),
    };
    (look, label)
}

/// A freehand stroke in screen space. Solid brushes get round ends; see-through brushes do not, so the
/// ends are not darker where the cap overlaps the line.
pub fn paint_stroke(painter: &Painter, pts: &[Pos2], stroke: Stroke, brush: Brush) {
    let width = stroke.width.max(0.5);
    if pts.is_empty() {
        return;
    }
    if pts.len() == 1 || (pts.len() == 2 && pts[0] == pts[1]) {
        painter.circle_filled(pts[0], width * 0.5, stroke.color);
        return;
    }
    // Points closer than a pixel apart on screen add work but no visible detail.
    painter.add(egui::Shape::line(thin(pts.iter().copied(), 0.75), Stroke::new(width, stroke.color)));
    if brush.alpha() >= 0.99 && width > 2.0 {
        painter.circle_filled(pts[0], width * 0.5, stroke.color);
        painter.circle_filled(pts[pts.len() - 1], width * 0.5, stroke.color);
    }
}

/// Drop points that are within `min` pixels of the last kept one (the first and last always stay).
fn thin(pts: impl Iterator<Item = Pos2>, min: f32) -> Vec<Pos2> {
    let mut out: Vec<Pos2> = Vec::new();
    let mut pending = None;
    for q in pts {
        match out.last() {
            Some(l) if l.distance_sq(q) < min * min => pending = Some(q),
            _ => {
                out.push(q);
                pending = None;
            }
        }
    }
    if let Some(q) = pending {
        out.push(q);
    }
    out
}

fn arrow_head(painter: &Painter, tip: Pos2, dir: Vec2, size: f32, stroke: Stroke) {
    let n = vec2(-dir.y, dir.x);
    let back = tip - dir * size;
    painter.add(egui::Shape::line(vec![back + n * size * 0.55, tip, back - n * size * 0.55], stroke));
}

fn arrow_tip(painter: &Painter, tip: Pos2, dir: Vec2, size: f32, color: Color32) {
    let n = vec2(-dir.y, dir.x);
    let back = tip - dir * size;
    painter.add(egui::Shape::convex_polygon(vec![tip, back + n * size * 0.55, back - n * size * 0.55], color, Stroke::NONE));
}
