//! The radial menu: hold the right mouse button on an item (or hold it still on empty board) and a
//! ring of actions opens under the pointer. Move onto one and let go to do it. Some open a second
//! ring further out (new items, tools, layers, colours, front and back); move on to one of those and
//! let go. Letting go in the middle does nothing. A quick right-click leaves the ring open to click in.

use crate::board::{self, Id, Ink, Side};
use crate::editor::Editor;
use crate::insert::{InsertMenu, Item as NewItem, Place};
use crate::tools::Tool;
use crate::theme::{Pal, pal};
use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::time::Instant;

/// Radius of the empty middle, where letting go cancels.
const R0: f32 = 32.0;
/// Outer edge of the ring.
const R1: f32 = 100.0;
/// A right-click shorter than this leaves the ring open to click in.
const TAP_SECS: f32 = 0.28;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hot {
    None,
    Main(usize),
    Child(usize, usize),
}

pub struct Radial {
    /// Screen position it opened at.
    center: Pos2,
    /// The same point on the board, where new items go when it opened on empty board.
    world: Pos2,
    /// Opened on an item (new items then go beside the selection) rather than on empty board.
    on_item: bool,
    opened: Instant,
    /// The wedge whose outer ring is showing.
    sub: Option<usize>,
    hot: Hot,
    /// Opened by a quick click: stays open until something is clicked.
    sticky: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Act {
    New(NewItem),
    Tool(Tool),
    /// The polyline tool, drawing straight segments or a curve.
    Poly(bool),
    Width(f32),
    Fit,
    Overview,
    Snapping,
    Properties,
    Layer(Id),
    Ink(Ink),
    Front,
    Back,
    Group,
    Ungroup,
    Lock,
    Duplicate,
    Delete,
}

#[derive(Clone, Copy, PartialEq)]
enum Icon {
    New,
    Tools,
    Width,
    View,
    Properties,
    Colour,
    Layers,
    Arrange,
    Delete,
    Duplicate,
    Lock,
    Unlock,
    Group,
    Front,
    Back,
}

/// How an item of the outer ring looks.
enum Look {
    /// A named pill (layers).
    Chip(String),
    Swatch(Ink),
    Icon(Icon),
    Tool(Tool),
    New(NewItem),
    /// A dot as wide as the line width.
    Width(f32),
    Paint(fn(&Painter, Rect, Color32, Color32)),
}

struct Child {
    label: String,
    look: Look,
    act: Act,
    /// What the selection already has (its layer, its colour).
    current: bool,
}

struct Item {
    label: String,
    icon: Icon,
    enabled: bool,
    act: Option<Act>,
    children: Vec<Child>,
}

impl Radial {
    pub fn new(center: Pos2, world: Pos2, on_item: bool) -> Self {
        Radial { center, world, on_item, opened: Instant::now(), sub: None, hot: Hot::None, sticky: false }
    }
    /// Open and waiting for a click (after a quick right-click).
    pub fn sticky(mut self) -> Self {
        self.sticky = true;
        self
    }
}

/// The ring for the current selection, clockwise from the top: making things first, then what to do
/// with the selection. Layers sit on the right so their names run outward without overlapping.
fn items(ed: &Editor) -> Vec<Item> {
    let any = !ed.selection.is_empty();
    let first = ed.selection.first().and_then(|id| ed.board.get(*id));
    let all_locked = !ed.selection.is_empty() && ed.selection.iter().all(|id| ed.is_locked(*id));
    let layer = first.map(|o| o.layer);
    let ink = Some(first.map(|o| o.style.ink).unwrap_or(ed.style.ink));
    let groups = ed.any_group_selected();
    let sketch = ed.selection.len() == 1 && first.is_some_and(|o| o.is_sketch());
    let one = |label: &str, icon: Icon, act: Act, enabled: bool| Item { label: label.into(), icon, enabled, act: Some(act), children: Vec::new() };
    let new_label = if ed.selection.len() == 1 { "New, connected" } else { "New" };
    vec![
        Item {
            label: new_label.into(),
            icon: Icon::New,
            enabled: true,
            act: None,
            // The same items as the "/" list.
            children: crate::insert::items().into_iter().map(|(i, name, _)| Child { label: name.into(), look: Look::New(i), act: Act::New(i), current: false }).collect(),
        },
        Item {
            label: "Tools".into(),
            icon: Icon::Tools,
            enabled: true,
            act: None,
            children: {
                let mut v: Vec<Child> = [Tool::Select, Tool::Hand, Tool::Connect, Tool::Text, Tool::Pen, Tool::Pencil, Tool::Marker, Tool::Highlighter, Tool::Eraser, Tool::Line, Tool::Arrow]
                    .into_iter()
                    .map(|t| Child { label: format!("{} ({})", t.name(), t.key_name()), look: Look::Tool(t), act: Act::Tool(t), current: ed.tool == t })
                    .collect();
                let poly = ed.tool == Tool::Polyline;
                v.push(Child { label: "Polyline (U)".into(), look: Look::Tool(Tool::Polyline), act: Act::Poly(false), current: poly && !ed.poly_curved });
                v.push(Child { label: "Curve through points".into(), look: Look::Paint(paint_curve_icon), act: Act::Poly(true), current: poly && ed.poly_curved });
                v.extend([Tool::Rectangle, Tool::Ellipse, Tool::Node].into_iter().map(|t| Child { label: format!("{} ({})", t.name(), t.key_name()), look: Look::Tool(t), act: Act::Tool(t), current: ed.tool == t }));
                v
            },
        },
        Item {
            label: "Width".into(),
            icon: Icon::Width,
            enabled: true,
            act: None,
            // Line width for what is drawn next, and for the selection.
            children: [0.5f32, 2.0, 6.0, 16.0, 40.0, 100.0]
                .into_iter()
                .map(|w| Child { label: format!("Width {w}"), look: Look::Width(w), act: Act::Width(w), current: (ed.tool_width() - w).abs() < 0.01 })
                .collect(),
        },
        Item {
            label: "Layer".into(),
            icon: Icon::Layers,
            enabled: any,
            act: None,
            // Front layer first, as in the Layers panel.
            children: ed.board.layers.iter().rev().map(|l| Child { label: format!("Move to {}", l.name), look: Look::Chip(l.name.clone()), act: Act::Layer(l.id), current: layer == Some(l.id) }).collect(),
        },
        Item {
            label: "Colour".into(),
            icon: Icon::Colour,
            enabled: true,
            act: None,
            children: board::swatches().into_iter().map(|i| Child { label: if i == Ink::Auto { "Ink (follows the theme)".into() } else { "Colour".into() }, look: Look::Swatch(i), act: Act::Ink(i), current: ink == Some(i) }).collect(),
        },
        Item {
            label: "Arrange".into(),
            icon: Icon::Arrange,
            enabled: any,
            act: None,
            children: vec![
                Child { label: "Bring to front".into(), look: Look::Icon(Icon::Front), act: Act::Front, current: false },
                Child { label: "Send to back".into(), look: Look::Icon(Icon::Back), act: Act::Back, current: false },
            ],
        },
        one("Delete", Icon::Delete, Act::Delete, any && !all_locked),
        one("Duplicate", Icon::Duplicate, Act::Duplicate, any),
        one(if all_locked { "Unlock" } else { "Lock" }, if all_locked { Icon::Unlock } else { Icon::Lock }, Act::Lock, any),
        if groups {
            one(if sketch { "Split sketch" } else { "Ungroup" }, Icon::Group, Act::Ungroup, true)
        } else {
            one("Group", Icon::Group, Act::Group, ed.selection.len() > 1)
        },
        Item {
            label: "View".into(),
            icon: Icon::View,
            enabled: true,
            act: None,
            children: vec![
                Child { label: "Zoom to fit (Shift+1)".into(), look: Look::Paint(crate::tools::paint_fit_icon), act: Act::Fit, current: false },
                Child { label: if ed.show_overview { "Hide the Overview (M)".into() } else { "Show the Overview (M)".into() }, look: Look::Paint(crate::tools::paint_overview_icon), act: Act::Overview, current: ed.show_overview },
                Child { label: if ed.snapping { "Turn snapping off".into() } else { "Turn snapping on".into() }, look: Look::Paint(crate::tools::paint_snap_icon), act: Act::Snapping, current: ed.snapping },
            ],
        },
        one("Properties", Icon::Properties, Act::Properties, true),
    ]
}

fn wedge_angle(i: usize, n: usize) -> f32 {
    -FRAC_PI_2 + i as f32 * TAU / n as f32
}

fn wedge_at(angle: f32, n: usize) -> usize {
    let step = TAU / n as f32;
    (((angle + FRAC_PI_2 + step * 0.5).rem_euclid(TAU)) / step) as usize % n
}

/// Smallest signed difference between two angles.
fn angle_diff(a: f32, b: f32) -> f32 {
    (a - b + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI
}

/// Where each item of a wedge's outer ring sits: its rect and the angle it is at.
fn child_rects(painter: &Painter, center: Pos2, i: usize, n: usize, children: &[Child]) -> Vec<Rect> {
    let a = wedge_angle(i, n);
    let chips = children.iter().any(|c| matches!(c.look, Look::Chip(_)));
    // Pills sit on a ring just outside, stacked 30 px apart; round items on a ring a little further,
    // in rows of at most eight so long lists stay close to the wedge.
    let per_row = if chips { children.len().max(1) } else { 8 };
    let rows = children.len().div_ceil(per_row).max(1);
    let per_row = children.len().div_ceil(rows).max(1);
    children
        .iter()
        .enumerate()
        .map(|(j, c)| {
            let (row, col) = (j / per_row, j % per_row);
            let in_row = (children.len() - row * per_row).min(per_row) as f32;
            let (r, step_px) = if chips { (R1 + 16.0, 30.0) } else { (R1 + 26.0 + row as f32 * 36.0, 34.0) };
            let step = step_px / r;
            let t = a + (col as f32 - (in_row - 1.0) * 0.5) * step;
            let at = center + vec2(t.cos(), t.sin()) * r;
            match &c.look {
                Look::Chip(name) => {
                    let w = painter.layout_no_wrap(name.clone(), FontId::proportional(12.0), Color32::WHITE).size().x.clamp(24.0, 140.0) + 22.0;
                    // The pill grows away from the ring on whichever side it is.
                    let min = if t.cos() >= -0.2 { pos2(at.x, at.y - 12.0) } else { pos2(at.x - w, at.y - 12.0) };
                    Rect::from_min_size(min, vec2(w, 24.0))
                }
                _ => Rect::from_center_size(at, vec2(28.0, 28.0)),
            }
        })
        .collect()
}

/// Run the radial menu for one frame: follow the pointer, draw, and do the chosen action when the
/// button is let go (or clicked, if it stayed open).
pub fn update(ed: &mut Editor, ctx: &egui::Context) {
    let Some(mut r) = ed.radial.take() else { return };
    let p = pal(ctx);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("radial")));
    let list = items(ed);
    let n = list.len();
    let (ptr, sec_down, sec_released, prim_pressed, sec_pressed, esc) = ctx.input(|i| {
        (i.pointer.hover_pos(), i.pointer.secondary_down(), i.pointer.secondary_released(), i.pointer.primary_pressed(), i.pointer.secondary_pressed(), i.key_pressed(egui::Key::Escape))
    });

    // ── what is under the pointer ──
    if let Some(q) = ptr {
        let d = q - r.center;
        let dist = d.length();
        let angle = d.y.atan2(d.x);
        r.hot = if dist < R0 {
            Hot::None
        } else if dist <= R1 {
            let w = wedge_at(angle, n);
            r.sub = (!list[w].children.is_empty() && list[w].enabled).then_some(w);
            Hot::Main(w)
        } else {
            // Past the ring: an item of the open outer ring, or else the wedge in that direction.
            let child = r.sub.and_then(|s| {
                let rects = child_rects(&painter, r.center, s, n, &list[s].children);
                rects.iter().position(|rc| rc.expand(4.0).contains(q)).or_else(|| {
                    // Between or past the items of the outer ring, the nearest one is picked, as long
                    // as the pointer is still in that ring's direction.
                    let span = rects.iter().map(|rc| angle_diff((rc.center() - r.center).angle(), wedge_angle(s, n)).abs()).fold(TAU / n as f32 * 0.5, f32::max) + 0.12;
                    if angle_diff(angle, wedge_angle(s, n)).abs() > span {
                        return None;
                    }
                    rects.iter().enumerate().min_by(|a, b| a.1.center().distance(q).total_cmp(&b.1.center().distance(q))).map(|(j, _)| j)
                }).map(|j| (s, j))
            });
            match child {
                Some((s, j)) => Hot::Child(s, j),
                None => {
                    let w = wedge_at(angle, n);
                    r.sub = (!list[w].children.is_empty() && list[w].enabled).then_some(w);
                    Hot::Main(w)
                }
            }
        };
    }

    draw(&painter, &p, &r, &list);

    // ── finishing ──
    let mut done = false;
    let mut act = None;
    if r.sticky {
        if prim_pressed || sec_pressed {
            act = chosen(&r, &list);
            // A click on a wedge that only opens an outer ring keeps the menu open.
            done = act.is_some() || !matches!(r.hot, Hot::Main(w) if !list[w].children.is_empty());
            ed.swallow = true;
        }
    } else if sec_released || !sec_down {
        let quick = r.opened.elapsed().as_secs_f32() < TAP_SECS && r.hot == Hot::None;
        if quick {
            r.sticky = true;
        } else {
            act = chosen(&r, &list);
            done = true;
        }
    }
    if esc {
        done = true;
        act = None;
    }
    if let Some(a) = act {
        run(ed, a, &r);
    }
    if !done {
        ed.radial = Some(r);
        ctx.request_repaint();
    }
}

fn chosen(r: &Radial, list: &[Item]) -> Option<Act> {
    match r.hot {
        Hot::Main(w) if list[w].enabled => list[w].act,
        Hot::Child(s, j) => list[s].children.get(j).map(|c| c.act),
        _ => None,
    }
}

fn run(ed: &mut Editor, a: Act, r: &Radial) {
    match a {
        Act::New(item) => {
            // Like the "/" list: beside the selected node and connected to it, or where the ring was
            // opened on empty board (connected to the selected node, if one is).
            let from = (ed.selection.len() == 1).then(|| ed.selection[0]).filter(|id| ed.board.get(*id).is_some());
            let (from, place) = if r.on_item { (from.map(|id| (id, Some(Side::Right))), Place::RightOf) } else { (from.map(|id| (id, None)), Place::At(r.world)) };
            ed.insert = Some(InsertMenu { from, place, query: String::new(), index: 0, focus: false });
            ed.insert_item(item);
        }
        Act::Tool(t) => {
            ed.finish_edit();
            ed.tool = t;
        }
        Act::Poly(curved) => {
            ed.finish_edit();
            ed.poly_curved = curved;
            ed.tool = Tool::Polyline;
        }
        Act::Width(w) => {
            ed.set_tool_width(w);
            crate::panels::apply_to_selection(ed, |s| s.width = w);
        }
        Act::Fit => ed.zoom_to_fit(),
        Act::Overview => ed.show_overview = !ed.show_overview,
        Act::Snapping => ed.snapping = !ed.snapping,
        Act::Properties => ed.want_properties = true,
        Act::Layer(id) => {
            ed.mark_changed();
            let sel = ed.selection.clone();
            ed.board.set_layer(&sel, id);
            let name = ed.board.layers.iter().find(|l| l.id == id).map(|l| l.name.clone()).unwrap_or_default();
            ed.say(format!("Moved to {name}"));
        }
        Act::Ink(ink) => {
            // The colour for what is drawn next, and for the selection.
            ed.style.ink = ink;
            crate::panels::apply_to_selection(ed, |s| s.ink = ink);
        }
        Act::Front => ed.reorder_selection(true),
        Act::Back => ed.reorder_selection(false),
        Act::Group => ed.group_selection(),
        Act::Ungroup => ed.ungroup_selection(),
        Act::Lock => ed.toggle_lock(),
        Act::Duplicate => ed.duplicate(),
        Act::Delete => ed.delete_selection(),
    }
}

// ── drawing ──

/// A ring segment from angle `a0` to `a1`, between radii `r0` and `r1`.
fn sector(painter: &Painter, c: Pos2, r0: f32, r1: f32, a0: f32, a1: f32, color: Color32) {
    let steps = 24;
    let mut mesh = egui::Mesh::default();
    for k in 0..=steps {
        let a = a0 + (a1 - a0) * k as f32 / steps as f32;
        let d = vec2(a.cos(), a.sin());
        mesh.colored_vertex(c + d * r0, color);
        mesh.colored_vertex(c + d * r1, color);
    }
    for k in 0..steps as u32 {
        let i = k * 2;
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i + 1, i + 3, i + 2);
    }
    painter.add(Shape::mesh(mesh));
}

fn draw(painter: &Painter, p: &Pal, r: &Radial, list: &[Item]) {
    let c = r.center;
    let n = list.len();
    let step = TAU / n as f32;
    // A soft shadow, then the ring in the window colours.
    for (k, a) in [(10.0, 18u8), (5.0, 26), (2.0, 34)] {
        painter.circle_filled(c + vec2(0.0, 3.0), R1 + k, Color32::from_black_alpha(a));
    }
    painter.circle(c, R1, p.window_fill, Stroke::new(1.0, p.window_stroke));
    // Thin dividers between wedges.
    for i in 0..n {
        let a = wedge_angle(i, n) + step * 0.5;
        let d = vec2(a.cos(), a.sin());
        painter.line_segment([c + d * (R0 + 6.0), c + d * (R1 - 6.0)], Stroke::new(1.0, p.hairline.gamma_multiply(0.5)));
    }
    let open = match r.hot {
        Hot::Main(w) => Some(w),
        Hot::Child(s, _) => Some(s),
        Hot::None => None,
    };
    if let Some(w) = open {
        let a = wedge_angle(w, n);
        let color = if list[w].enabled { p.accent } else { p.bg_control_hover };
        sector(painter, c, R0 + 3.0, R1 - 3.0, a - step * 0.5 + 0.03, a + step * 0.5 - 0.03, if matches!(r.hot, Hot::Child(..)) { color.gamma_multiply(0.55) } else { color });
    }
    for (i, it) in list.iter().enumerate() {
        let a = wedge_angle(i, n);
        let at = c + vec2(a.cos(), a.sin()) * (R0 + R1) * 0.5;
        let lit = open == Some(i) && it.enabled;
        let color = if lit { Color32::WHITE } else if it.enabled { p.text } else { p.text_secondary.gamma_multiply(0.5) };
        paint_icon(painter, Rect::from_center_size(at, vec2(22.0, 22.0)), it.icon, color, if lit { Color32::WHITE } else { p.accent });
        // A small notch shows the wedges that open an outer ring.
        if !it.children.is_empty() {
            let d = vec2(a.cos(), a.sin());
            painter.circle_filled(c + d * (R1 - 9.0), 1.8, if lit { Color32::WHITE } else { p.text_secondary });
        }
    }
    // The middle: what letting go will do.
    painter.circle(c, R0, p.well, Stroke::new(1.0, p.hairline));
    let label = match r.hot {
        Hot::Main(w) if !list[w].enabled => "Not available".to_owned(),
        Hot::Main(w) => list[w].label.clone(),
        Hot::Child(s, j) => list[s].children.get(j).map(|ch| ch.label.clone()).unwrap_or_default(),
        Hot::None => if r.sticky { "Click one".into() } else { "Cancel".into() },
    };
    let g = painter.layout(label, FontId::proportional(10.5), p.text, R0 * 2.0 - 8.0);
    painter.galley(c - g.size() * 0.5, g, p.text);

    // The outer ring of the open wedge.
    if let Some(s) = r.sub.filter(|s| list[*s].enabled) {
        let rects = child_rects(painter, c, s, n, &list[s].children);
        for (j, (ch, rc)) in list[s].children.iter().zip(&rects).enumerate() {
            let hot = r.hot == Hot::Child(s, j);
            match &ch.look {
                Look::Chip(name) => {
                    painter.rect(*rc, CornerRadius::same(12), if hot { p.accent } else { p.window_fill }, Stroke::new(1.0, if hot { p.accent } else { p.window_stroke }), StrokeKind::Inside);
                    let text = if hot { Color32::WHITE } else { p.text };
                    let g = painter.layout_no_wrap(name.clone(), FontId::proportional(12.0), text);
                    painter.galley(pos2(rc.left() + 14.0, rc.center().y - g.size().y * 0.5), g, text);
                    if ch.current {
                        painter.circle_filled(pos2(rc.left() + 7.0, rc.center().y), 2.5, if hot { Color32::WHITE } else { p.accent });
                    }
                }
                Look::Swatch(ink) => {
                    painter.circle(rc.center(), 13.0, p.window_fill, Stroke::new(if hot { 2.0 } else { 1.0 }, if hot || ch.current { p.accent } else { p.window_stroke }));
                    painter.circle(rc.center(), 8.0, ink.color(p), Stroke::new(1.0, p.hairline));
                }
                Look::Icon(_) | Look::Tool(_) | Look::New(_) | Look::Width(_) | Look::Paint(_) => {
                    // Round tiles; the current tool is ringed in blue.
                    let ring = if hot { p.accent } else if ch.current { p.accent } else { p.window_stroke };
                    painter.circle(rc.center(), 14.0, if hot { p.accent } else { p.window_fill }, Stroke::new(if ch.current && !hot { 1.5 } else { 1.0 }, ring));
                    let col = if hot { Color32::WHITE } else { p.text };
                    let dot = if hot { Color32::WHITE } else { p.accent };
                    let ir = rc.shrink(6.0);
                    match &ch.look {
                        Look::Icon(icon) => paint_icon(painter, ir, *icon, col, dot),
                        Look::Tool(t) => crate::tools::paint_icon(painter, ir, *t, col, dot),
                        Look::New(i) => crate::insert::paint_item_icon(painter, ir, *i, col, dot),
                        Look::Paint(f) => f(painter, ir, col, dot),
                        Look::Width(w) => {
                            // Dot size grows with the log of the width, as in the old width presets.
                            let t = (w.ln() - board::MIN_WIDTH.ln()) / (board::MAX_WIDTH.ln() - board::MIN_WIDTH.ln());
                            painter.circle_filled(ir.center(), 1.0 + t * 7.5, col);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Small line icons on a 20-unit grid, in the style of the tool icons.
fn paint_icon(p: &Painter, rect: Rect, icon: Icon, c: Color32, dot: Color32) {
    let s = rect.width().min(rect.height()) / 20.0;
    let o = rect.center() - vec2(10.0, 10.0) * s;
    let at = |x: f32, y: f32| -> Pos2 { o + vec2(x, y) * s };
    let st = Stroke::new(1.6 * s.max(0.8), c);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), st));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(Shape::closed_line(pts.iter().map(|&(x, y)| at(x, y)).collect(), st));
    };
    let rect_at = |x0: f32, y0: f32, x1: f32, y1: f32| {
        p.rect_stroke(Rect::from_min_max(at(x0, y0), at(x1, y1)), 1.5 * s, st, StrokeKind::Middle);
    };
    match icon {
        Icon::New => {
            p.rect_stroke(Rect::from_min_max(at(3.0, 5.0), at(13.0, 15.0)), 2.0 * s, st, StrokeKind::Middle);
            line(&[(16.5, 2.5), (16.5, 8.5)]);
            line(&[(13.5, 5.5), (19.5, 5.5)]);
            p.circle_filled(at(13.0, 10.0), 1.5 * s, dot);
        }
        Icon::Tools => {
            // A pen over a ruler.
            closed(&[(13.5, 2.5), (17.5, 6.5), (8.0, 16.0), (4.0, 16.0), (4.0, 12.0)]);
            line(&[(11.0, 5.0), (15.0, 9.0)]);
            p.circle_filled(at(5.2, 14.8), 1.4 * s, dot);
        }
        Icon::Width => {
            line(&[(3.0, 5.0), (17.0, 5.0)]);
            p.line_segment([at(3.0, 10.0), at(17.0, 10.0)], Stroke::new(3.0 * s, c));
            p.line_segment([at(3.0, 15.5), at(17.0, 15.5)], Stroke::new(4.5 * s, c));
            p.circle_filled(at(17.0, 5.0), 1.3 * s, dot);
        }
        Icon::View => {
            // An eye.
            let top: Vec<Pos2> = (0..=16).map(|i| {
                let t = i as f32 / 16.0;
                at(2.5 + 15.0 * t, 10.0 - 5.5 * (t * std::f32::consts::PI).sin())
            }).collect();
            let bottom: Vec<Pos2> = (0..=16).map(|i| {
                let t = i as f32 / 16.0;
                at(2.5 + 15.0 * t, 10.0 + 5.5 * (t * std::f32::consts::PI).sin())
            }).collect();
            p.add(Shape::line(top, st));
            p.add(Shape::line(bottom, st));
            p.circle_stroke(at(10.0, 10.0), 2.6 * s, st);
            p.circle_filled(at(10.0, 10.0), 1.2 * s, dot);
        }
        Icon::Properties => {
            for (y, k) in [(5.0, 13.0), (10.0, 7.0), (15.0, 11.0)] {
                line(&[(3.0, y), (17.0, y)]);
                p.circle(at(k, y), 2.0 * s, c, Stroke::NONE);
            }
            p.circle_filled(at(7.0, 10.0), 1.3 * s, dot);
        }
        Icon::Colour => {
            // A drop: a point at the top, round below.
            let mut drop = vec![at(10.0, 2.5), at(15.5, 12.0)];
            drop.extend((0..=20).map(|i| {
                let a = std::f32::consts::PI * i as f32 / 20.0;
                at(10.0 + 5.5 * a.cos(), 12.0 + 5.5 * a.sin())
            }));
            drop.push(at(10.0, 2.5));
            p.add(Shape::line(drop, st));
            p.circle_filled(at(10.0, 13.0), 1.8 * s, dot);
        }
        Icon::Layers => {
            closed(&[(10.0, 3.0), (17.0, 7.0), (10.0, 11.0), (3.0, 7.0)]);
            line(&[(3.0, 10.5), (10.0, 14.5), (17.0, 10.5)]);
            line(&[(3.0, 14.0), (10.0, 18.0), (17.0, 14.0)]);
            p.circle_filled(at(10.0, 7.0), 1.4 * s, dot);
        }
        Icon::Arrange => {
            rect_at(3.0, 3.0, 12.0, 12.0);
            p.rect(Rect::from_min_max(at(8.0, 8.0), at(17.0, 17.0)), 1.5 * s, c.gamma_multiply(0.25), st, StrokeKind::Middle);
        }
        Icon::Front => {
            rect_at(3.0, 7.0, 13.0, 17.0);
            line(&[(13.5, 9.0), (13.5, 3.0)]);
            line(&[(11.0, 5.5), (13.5, 3.0), (16.0, 5.5)]);
        }
        Icon::Back => {
            rect_at(3.0, 3.0, 13.0, 13.0);
            line(&[(13.5, 11.0), (13.5, 17.0)]);
            line(&[(11.0, 14.5), (13.5, 17.0), (16.0, 14.5)]);
        }
        Icon::Delete => {
            line(&[(3.5, 5.5), (16.5, 5.5)]);
            line(&[(8.0, 5.5), (8.0, 3.5), (12.0, 3.5), (12.0, 5.5)]);
            closed(&[(5.0, 5.5), (6.0, 17.0), (14.0, 17.0), (15.0, 5.5)]);
            line(&[(8.5, 8.5), (8.5, 14.0)]);
            line(&[(11.5, 8.5), (11.5, 14.0)]);
        }
        Icon::Duplicate => {
            rect_at(3.0, 6.5, 13.5, 17.0);
            line(&[(6.5, 3.0), (17.0, 3.0), (17.0, 13.5)]);
        }
        Icon::Lock | Icon::Unlock => {
            rect_at(4.5, 9.0, 15.5, 17.0);
            let open = icon == Icon::Unlock;
            let arc: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.0 + i as f32 / 16.0);
                    at(10.0 + 3.5 * a.cos(), 7.0 + 3.5 * a.sin())
                })
                .collect();
            let mut pts = vec![at(6.5, 9.0)];
            pts.extend(arc);
            pts.push(at(13.5, if open { 5.0 } else { 9.0 }));
            p.add(Shape::line(pts, st));
            p.circle_filled(at(10.0, 13.0), 1.4 * s, dot);
        }
        Icon::Group => {
            p.add(Shape::dashed_line(&[at(3.0, 3.0), at(17.0, 3.0), at(17.0, 17.0), at(3.0, 17.0), at(3.0, 3.0)], st, 2.5 * s, 2.0 * s));
            rect_at(6.5, 6.5, 13.5, 13.5);
        }
    }
}

/// Icon for a curve through points: the polyline icon with a smooth line.
fn paint_curve_icon(p: &Painter, rect: Rect, c: Color32, dot: Color32) {
    let s = rect.width().min(rect.height()) / 20.0;
    let o = rect.center() - vec2(10.0, 10.0) * s;
    let at = |x: f32, y: f32| o + vec2(x, y) * s;
    let pts = [at(3.5, 15.5), at(7.5, 6.0), at(12.5, 13.0), at(16.5, 4.5)];
    p.add(Shape::line(board::catmull_rom(&pts, false), Stroke::new(1.6 * s.max(0.8), c)));
    for q in &pts[..3] {
        p.circle_filled(*q, 1.5 * s, c);
    }
    p.circle_filled(pts[3], 1.7 * s, dot);
}
