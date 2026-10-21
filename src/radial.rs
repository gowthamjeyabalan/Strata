//! The radial menu: hold the right mouse button on an item (or hold it still on empty board) and a
//! ring of actions opens under the pointer. Move onto one and let go to do it. Some open a second
//! ring further out (Add, tools, layers, colours, front and back), and Add's sections open a third;
//! move on to one and let go. Letting go in the middle does nothing. A quick right-click leaves the
//! ring open to click in.

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
    /// An item of the third ring: wedge, section, item.
    Grand(usize, usize, usize),
}

pub struct Radial {
    /// Screen position it opened at.
    center: Pos2,
    /// The same point on the board, where new items go when it opened on empty board.
    world: Pos2,
    /// Opened on an item (new items then go beside the selection) rather than on empty board.
    on_item: bool,
    /// Opened on a note in the Notes list: the ring is that note's options.
    note: Option<Id>,
    opened: Instant,
    /// The wedge whose outer ring is showing.
    sub: Option<usize>,
    /// The item of that ring whose own ring (a third one) is showing.
    sub2: Option<(usize, usize)>,
    hot: Hot,
    /// Opened by a quick click: stays open until something is clicked.
    sticky: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Act {
    /// Opens a further ring; does nothing itself.
    Open,
    New(NewItem),
    /// Open the colour picker window.
    PickColour,
    NewNote,
    PlaceNote(Id),
    OpenNote(Id),
    DeleteNote(Id),
    /// Move a note (and the others picked with it) into a folder; 0 for none.
    NoteFolder(Id, Id),
    Tool(Tool),
    /// The polyline tool, drawing straight segments or a curve.
    Poly(bool),
    Width(f32),
    Fit,
    FitSelection,
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
    Note,
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
    /// A further ring that opens when this is pointed at.
    sub: Vec<Child>,
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
        Radial { center, world, on_item, note: None, opened: Instant::now(), sub: None, sub2: None, hot: Hot::None, sticky: false }
    }
    /// The ring for a note in the Notes list. Cards placed from it go at `world`.
    pub fn for_note(center: Pos2, world: Pos2, note: Id) -> Self {
        Radial { note: Some(note), ..Radial::new(center, world, false) }
    }
    pub fn from_list(&self) -> bool {
        self.note.is_some()
    }
    /// Open and waiting for a click (after a quick right-click).
    pub fn sticky(mut self) -> Self {
        self.sticky = true;
        self
    }
}

/// The ring for the current selection, clockwise from the top: making things first, then what to do
/// with the selection. Layers sit on the right so their names run outward without overlapping.
fn items(ed: &Editor, r: &Radial) -> Vec<Item> {
    let any = !ed.selection.is_empty();
    let one = |label: &str, icon: Icon, act: Act, enabled: bool| Item { label: label.into(), icon, enabled, act: Some(act), children: Vec::new() };
    // On a note in the Notes list: just that note's options.
    if let Some(n) = r.note {
        let here = ed.board.note(n).map(|x| x.folder).unwrap_or(0);
        let mut folders = vec![Child { label: "Out of its folder".into(), look: Look::Chip("No folder".into()), act: Act::NoteFolder(n, 0), current: here == 0, sub: Vec::new() }];
        folders.extend(ed.board.folders.iter().map(|f| Child { label: format!("Move to {}", f.name), look: Look::Chip(f.name.clone()), act: Act::NoteFolder(n, f.id), current: here == f.id, sub: Vec::new() }));
        return vec![
            one("Open", Icon::Note, Act::OpenNote(n), true),
            one("Place on board", Icon::New, Act::PlaceNote(n), true),
            Item { label: "Move to folder".into(), icon: Icon::Layers, enabled: !ed.board.folders.is_empty(), act: None, children: folders },
            one("Delete note", Icon::Delete, Act::DeleteNote(n), true),
        ];
    }
    // A selected note card adds a Note section: open it, another card, delete the note itself.
    let card_note = match ed.selection[..] {
        [id] => match ed.board.get(id).map(|o| &o.kind) {
            Some(board::Kind::Note { note }) => Some(*note),
            _ => None,
        },
        _ => None,
    };
    let first = ed.selection.first().and_then(|id| ed.board.get(*id));
    let all_locked = !ed.selection.is_empty() && ed.selection.iter().all(|id| ed.is_locked(*id));
    let layer = first.map(|o| o.layer);
    let ink = Some(first.map(|o| o.style.ink).unwrap_or(ed.style.ink));
    let groups = ed.any_group_selected();
    let sketch = ed.selection.len() == 1 && first.is_some_and(|o| o.is_sketch());
    let add_label = if ed.selection.len() == 1 { "Add, connected" } else { "Add" };
    let mut list = vec![
        Item {
            label: add_label.into(),
            icon: Icon::New,
            enabled: true,
            act: None,
            children: {
                let tool = |t: Tool| Child { label: ed.keys.label(crate::keys::Action::Tool(t)), look: Look::Tool(t), act: Act::Tool(t), current: ed.tool == t, sub: Vec::new() };
                let section = |label: &str, look: Look, sub: Vec<Child>| Child { label: label.into(), look, act: Act::Open, current: false, sub };
                let poly = ed.tool == Tool::Polyline;
                let mut shapes: Vec<Child> = [Tool::Line, Tool::Arrow].into_iter().map(tool).collect();
                shapes.push(Child { label: "Polyline (U)".into(), look: Look::Tool(Tool::Polyline), act: Act::Poly(false), current: poly && !ed.poly_curved, sub: Vec::new() });
                shapes.push(Child { label: "Curve through points".into(), look: Look::Paint(paint_curve_icon), act: Act::Poly(true), current: poly && ed.poly_curved, sub: Vec::new() });
                shapes.extend([Tool::Rectangle, Tool::Ellipse, Tool::Text, Tool::Frame].into_iter().map(tool));
                // Flowchart boxes are added like the "/" list does: connected to the selected node.
                let flow: Vec<Child> = crate::insert::items()
                    .into_iter()
                    .filter(|(i, _, _)| matches!(i, NewItem::Shape(s) if board::Shape::FLOW.contains(s)))
                    .map(|(i, name, _)| Child { label: name.into(), look: Look::New(i), act: Act::New(i), current: false, sub: Vec::new() })
                    .collect();
                let mut notes = vec![Child { label: "New note".into(), look: Look::Chip("+ New note".into()), act: Act::NewNote, current: false, sub: Vec::new() }];
                notes.extend(ed.board.notes.iter().rev().take(12).map(|n| Child { label: format!("Place \"{}\"", n.title()), look: Look::Chip(n.title()), act: Act::PlaceNote(n.id), current: false, sub: Vec::new() }));
                vec![
                    section("Draw", Look::Tool(Tool::Pen), [Tool::Pen, Tool::Pencil, Tool::Marker, Tool::Highlighter, Tool::Eraser].into_iter().map(tool).collect()),
                    section("Shapes", Look::Tool(Tool::Rectangle), shapes),
                    section("Flowchart", Look::Tool(Tool::Node), flow),
                    section("Note", Look::Icon(Icon::Note), notes),
                    Child { label: "Image…".into(), look: Look::New(NewItem::Image), act: Act::New(NewItem::Image), current: false, sub: Vec::new() },
                ]
            },
        },
        Item {
            label: "Tools".into(),
            icon: Icon::Tools,
            enabled: true,
            act: None,
            children: [Tool::Select, Tool::Hand, Tool::Connect]
                .into_iter()
                .map(|t| Child { label: ed.keys.label(crate::keys::Action::Tool(t)), look: Look::Tool(t), act: Act::Tool(t), current: ed.tool == t, sub: Vec::new() })
                .collect(),
        },
        Item {
            label: "Width".into(),
            icon: Icon::Width,
            enabled: true,
            act: None,
            // Line width for what is drawn next, and for the selection.
            children: [0.5f32, 2.0, 6.0, 16.0, 40.0, 100.0, 250.0, 600.0]
                .into_iter()
                .map(|w| Child { label: format!("Width {w}"), look: Look::Width(w), act: Act::Width(w), current: (ed.tool_width() - w).abs() < 0.01, sub: Vec::new() })
                .collect(),
        },
        Item {
            label: "Layer".into(),
            icon: Icon::Layers,
            enabled: any,
            act: None,
            // Front layer first, as in the Layers panel.
            children: ed.board.layers.iter().rev().map(|l| Child { label: format!("Move to {}", l.name), look: Look::Chip(l.name.clone()), act: Act::Layer(l.id), current: layer == Some(l.id), sub: Vec::new() }).collect(),
        },
        Item {
            label: "Colour".into(),
            icon: Icon::Colour,
            enabled: true,
            act: None,
            // The presets, the colours picked lately, and the full picker.
            children: board::swatches()
                .into_iter()
                .chain(ed.custom_colors.iter().map(|c| Ink::Rgb(*c)))
                .map(|i| Child { label: if i == Ink::Auto { "Ink (follows the theme)".into() } else { "Colour".into() }, look: Look::Swatch(i), act: Act::Ink(i), current: ink == Some(i), sub: Vec::new() })
                .chain(std::iter::once(Child { label: "More colours…".into(), look: Look::Paint(paint_spectrum_icon), act: Act::PickColour, current: false, sub: Vec::new() }))
                .collect(),
        },
        Item {
            label: "Arrange".into(),
            icon: Icon::Arrange,
            enabled: any,
            act: None,
            children: vec![
                Child { label: "Bring to front".into(), look: Look::Icon(Icon::Front), act: Act::Front, current: false, sub: Vec::new() },
                Child { label: "Send to back".into(), look: Look::Icon(Icon::Back), act: Act::Back, current: false, sub: Vec::new() },
            ],
        },
        one(if card_note.is_some() { "Remove card" } else { "Delete" }, Icon::Delete, Act::Delete, any && !all_locked),
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
                Child { label: "Zoom to fit".into(), look: Look::Paint(crate::tools::paint_fit_icon), act: Act::Fit, current: false, sub: Vec::new() },
                Child { label: "Zoom to the selection".into(), look: Look::Paint(crate::tools::paint_fit_icon), act: Act::FitSelection, current: false, sub: Vec::new() },
                Child { label: if ed.show_overview { "Hide the Overview (M)".into() } else { "Show the Overview (M)".into() }, look: Look::Paint(crate::tools::paint_overview_icon), act: Act::Overview, current: ed.show_overview, sub: Vec::new() },
                Child { label: if ed.snapping { "Turn snapping off".into() } else { "Turn snapping on".into() }, look: Look::Paint(crate::tools::paint_snap_icon), act: Act::Snapping, current: ed.snapping, sub: Vec::new() },
            ],
        },
        one("Properties", Icon::Properties, Act::Properties, true),
    ];
    if let Some(n) = card_note {
        let leaf = |label: &str, icon: Icon, act: Act| Child { label: label.into(), look: Look::Icon(icon), act, current: false, sub: Vec::new() };
        list.insert(1, Item {
            label: "Note".into(),
            icon: Icon::Note,
            enabled: true,
            act: None,
            children: vec![leaf("Open the note", Icon::Note, Act::OpenNote(n)), leaf("Another card", Icon::New, Act::PlaceNote(n)), leaf("Delete the note and its cards", Icon::Delete, Act::DeleteNote(n))],
        });
    }
    list
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

/// Where the items of a ring sit, in direction `a` from the centre. Round items go on arcs starting
/// at radius `r_round`, in rows of at most eight; named pills stack in a column at radius `r_chip`.
fn ring_rects(painter: &Painter, center: Pos2, a: f32, r_round: f32, r_chip: f32, children: &[Child]) -> Vec<Rect> {
    let chips = children.iter().any(|c| matches!(c.look, Look::Chip(_)));
    if chips {
        let dir = vec2(a.cos(), a.sin());
        let anchor = center + dir * r_chip;
        let widths: Vec<f32> = children
            .iter()
            .map(|c| match &c.look {
                Look::Chip(name) => painter.layout_no_wrap(name.clone(), FontId::proportional(12.0), Color32::WHITE).size().x.clamp(24.0, 160.0) + 22.0,
                _ => 28.0,
            })
            .collect();
        let (h, gap) = (24.0, 6.0);
        let total = children.len() as f32 * (h + gap) - gap;
        // Above or below the centre the column grows away from it; to the sides it is centred.
        let (top, vertical) = if dir.y.abs() > 0.7 { (if dir.y < 0.0 { anchor.y - total } else { anchor.y }, true) } else { (anchor.y - total * 0.5, false) };
        return widths
            .iter()
            .enumerate()
            .map(|(k, w)| {
                let y = top + k as f32 * (h + gap);
                let x = if vertical { anchor.x - w * 0.5 } else if dir.x >= 0.0 { anchor.x } else { anchor.x - w };
                Rect::from_min_size(pos2(x, y), vec2(*w, h))
            })
            .collect();
    }
    let per_row = children.len().div_ceil(children.len().div_ceil(8).max(1)).max(1);
    children
        .iter()
        .enumerate()
        .map(|(j, _)| {
            let (row, col) = (j / per_row, j % per_row);
            let in_row = (children.len() - row * per_row).min(per_row) as f32;
            let rr = r_round + row as f32 * 36.0;
            let t = a + (col as f32 - (in_row - 1.0) * 0.5) * (34.0 / rr);
            Rect::from_center_size(center + vec2(t.cos(), t.sin()) * rr, vec2(28.0, 28.0))
        })
        .collect()
}

/// The second ring, for wedge `i`.
fn child_rects(painter: &Painter, center: Pos2, i: usize, n: usize, children: &[Child]) -> Vec<Rect> {
    ring_rects(painter, center, wedge_angle(i, n), R1 + 26.0, R1 + 16.0, children)
}

/// The third ring, for item `j` of the second ring: further out, in that item's direction.
fn grand_rects(painter: &Painter, center: Pos2, second: &[Rect], j: usize, children: &[Child]) -> Vec<Rect> {
    let c = second[j].center();
    let a = (c - center).angle();
    let out = second.iter().map(|rc| (rc.center() - center).length()).fold(0.0, f32::max);
    ring_rects(painter, center, a, out + 42.0, out + 30.0, children)
}

/// The item of a ring nearest the pointer, as long as the pointer is beyond `min_dist` and still in
/// the ring's direction.
fn nearest(center: Pos2, q: Pos2, rects: &[Rect], dir: f32, min_dist: f32) -> Option<usize> {
    if rects.is_empty() || (q - center).length() < min_dist {
        return None;
    }
    let span = rects.iter().map(|rc| angle_diff((rc.center() - center).angle(), dir).abs()).fold(0.25, f32::max) + 0.12;
    if angle_diff((q - center).angle(), dir).abs() > span {
        return None;
    }
    rects.iter().enumerate().min_by(|a, b| a.1.center().distance(q).total_cmp(&b.1.center().distance(q))).map(|(j, _)| j)
}

/// Run the radial menu for one frame: follow the pointer, draw, and do the chosen action when the
/// button is let go (or clicked, if it stayed open).
pub fn update(ed: &mut Editor, ctx: &egui::Context) {
    let Some(mut r) = ed.radial.take() else { return };
    let p = pal(ctx);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("radial")));
    let list = items(ed, &r);
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
            r.sub2 = None;
            Hot::Main(w)
        } else {
            // Past the ring: an item of the third ring, of the second, or else the wedge in that
            // direction.
            let mut hot = None;
            if let Some(s) = r.sub {
                let second = child_rects(&painter, r.center, s, n, &list[s].children);
                if let Some((_, j)) = r.sub2.filter(|(ss, j)| *ss == s && *j < second.len()) {
                    let kids = &list[s].children[j].sub;
                    let third = grand_rects(&painter, r.center, &second, j, kids);
                    if let Some(k) = third.iter().position(|rc| rc.expand(4.0).contains(q)) {
                        hot = Some(Hot::Grand(s, j, k));
                    }
                }
                if hot.is_none() {
                    if let Some(j) = second.iter().position(|rc| rc.expand(4.0).contains(q)) {
                        hot = Some(Hot::Child(s, j));
                    }
                }
                if hot.is_none() {
                    if let Some((_, j)) = r.sub2.filter(|(ss, j)| *ss == s && *j < second.len()) {
                        let kids = &list[s].children[j].sub;
                        let third = grand_rects(&painter, r.center, &second, j, kids);
                        let out = second.iter().map(|rc| (rc.center() - r.center).length()).fold(0.0, f32::max) + 16.0;
                        if let Some(k) = nearest(r.center, q, &third, (second[j].center() - r.center).angle(), out) {
                            hot = Some(Hot::Grand(s, j, k));
                        }
                    }
                }
                if hot.is_none() {
                    if let Some(j) = nearest(r.center, q, &second, wedge_angle(s, n), R1) {
                        hot = Some(Hot::Child(s, j));
                    }
                }
            }
            match hot {
                Some(h) => {
                    if let Hot::Child(s, j) = h {
                        r.sub2 = (!list[s].children[j].sub.is_empty()).then_some((s, j));
                    }
                    h
                }
                None => {
                    let w = wedge_at(angle, n);
                    r.sub = (!list[w].children.is_empty() && list[w].enabled).then_some(w);
                    r.sub2 = None;
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
            // A click on something that only opens a further ring keeps the menu open.
            let opens = match r.hot {
                Hot::Main(w) => !list[w].children.is_empty(),
                Hot::Child(s, j) => !list[s].children[j].sub.is_empty(),
                _ => false,
            };
            done = act.is_some() || !opens;
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
        Hot::Child(s, j) => list[s].children.get(j).map(|c| c.act).filter(|a| *a != Act::Open),
        Hot::Grand(s, j, k) => list[s].children.get(j).and_then(|c| c.sub.get(k)).map(|c| c.act),
        _ => None,
    }
}

fn run(ed: &mut Editor, a: Act, r: &Radial) {
    // A note card goes where the ring was opened, or beside the selection.
    let note_at = || -> Pos2 {
        let sel = ed.selection.iter().filter_map(|id| ed.board.get(*id)).map(|o| o.rect()).reduce(|a, b| a.union(b));
        match sel {
            Some(b) if r.on_item => pos2(b.right() + 230.0, b.center().y),
            _ => r.world,
        }
    };
    let at = note_at();
    match a {
        Act::Open => {}
        Act::PickColour => {
            let cur = ed.selection.first().and_then(|id| ed.board.get(*id)).map(|o| ed.board.ink_of(o)).unwrap_or(ed.style.ink);
            ed.colour_pick = Some(cur);
        }
        Act::NewNote => {
            let note = ed.add_note("# New note\n\n", true);
            ed.place_note(note, at);
        }
        Act::PlaceNote(note) => {
            ed.place_note(note, at);
        }
        Act::OpenNote(note) => ed.open_note = Some(note),
        Act::DeleteNote(note) => {
            let ids = if ed.notes_sel.contains(&note) { ed.notes_sel.clone() } else { vec![note] };
            ed.delete_notes(&ids);
        }
        Act::NoteFolder(note, folder) => {
            let ids = if ed.notes_sel.contains(&note) { ed.notes_sel.clone() } else { vec![note] };
            ed.move_notes(&ids, folder);
        }
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
        Act::FitSelection => ed.zoom_to_selection(),
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
        Act::Ink(ink) => crate::panels::use_colour(ed, ink),
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
        Hot::Child(s, _) | Hot::Grand(s, _, _) => Some(s),
        Hot::None => None,
    };
    if let Some(w) = open {
        let a = wedge_angle(w, n);
        let color = if list[w].enabled { p.accent } else { p.bg_control_hover };
        sector(painter, c, R0 + 3.0, R1 - 3.0, a - step * 0.5 + 0.03, a + step * 0.5 - 0.03, if matches!(r.hot, Hot::Child(..) | Hot::Grand(..)) { color.gamma_multiply(0.55) } else { color });
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
        Hot::Grand(s, j, k) => list[s].children.get(j).and_then(|c| c.sub.get(k)).map(|ch| ch.label.clone()).unwrap_or_default(),
        Hot::None => if r.sticky { "Click one".into() } else { "Cancel".into() },
    };
    let g = painter.layout(label, FontId::proportional(10.5), p.text, R0 * 2.0 - 8.0);
    painter.galley(c - g.size() * 0.5, g, p.text);

    // The outer rings of the open wedge.
    if let Some(s) = r.sub.filter(|s| list[*s].enabled) {
        let second = child_rects(painter, c, s, n, &list[s].children);
        let open2 = r.sub2.filter(|(ss, j)| *ss == s && *j < second.len()).map(|(_, j)| j);
        for (j, (ch, rc)) in list[s].children.iter().zip(&second).enumerate() {
            // The item whose third ring is open stays lit, more softly, while that ring is used.
            let lit = if r.hot == Hot::Child(s, j) { 2 } else if open2 == Some(j) { 1 } else { 0 };
            draw_child(painter, p, ch, *rc, lit);
        }
        if let Some(j) = open2 {
            let kids = &list[s].children[j].sub;
            let third = grand_rects(painter, c, &second, j, kids);
            for (k, (ch, rc)) in kids.iter().zip(&third).enumerate() {
                draw_child(painter, p, ch, *rc, if r.hot == Hot::Grand(s, j, k) { 2 } else { 0 });
            }
        }
    }
}

/// One item of an outer ring. `lit` is 2 when pointed at, 1 when its own ring is open.
fn draw_child(painter: &Painter, p: &Pal, ch: &Child, rc: Rect, lit: u8) {
    let hot = lit == 2;
    let fill = match lit {
        2 => p.accent,
        1 => p.accent.gamma_multiply(0.35),
        _ => p.window_fill,
    };
    match &ch.look {
        Look::Chip(name) => {
            painter.rect(rc, CornerRadius::same(12), fill, Stroke::new(1.0, if lit > 0 { p.accent } else { p.window_stroke }), StrokeKind::Inside);
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
        _ => {
            // Round tiles; the current tool is ringed in blue, and sections show a dot that more
            // opens from them.
            let ring = if lit > 0 || ch.current { p.accent } else { p.window_stroke };
            painter.circle(rc.center(), 14.0, fill, Stroke::new(if ch.current && !hot { 1.5 } else { 1.0 }, ring));
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
            if !ch.sub.is_empty() {
                painter.circle_filled(rc.center() + vec2(9.5, -9.5), 2.6, if hot { Color32::WHITE } else { p.accent });
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
        Icon::Note => {
            closed(&[(4.0, 2.5), (12.5, 2.5), (16.0, 6.0), (16.0, 17.5), (4.0, 17.5)]);
            line(&[(7.0, 8.0), (13.0, 8.0)]);
            line(&[(7.0, 11.0), (13.0, 11.0)]);
            line(&[(7.0, 14.0), (11.0, 14.0)]);
            p.circle_filled(at(13.5, 5.0), 1.3 * s, dot);
        }
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

/// Icon for the colour picker: a ring of hues.
fn paint_spectrum_icon(p: &Painter, rect: Rect, _c: Color32, _dot: Color32) {
    let c = rect.center();
    let rad = rect.width().min(rect.height()) * 0.42;
    for k in 0..12 {
        let a0 = k as f32 / 12.0 * TAU;
        let col = egui::epaint::Hsva::new(k as f32 / 12.0, 0.75, 0.95, 1.0);
        sector(p, c, rad * 0.45, rad, a0, a0 + TAU / 12.0 + 0.02, Color32::from(col));
    }
}
