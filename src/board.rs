//! The board: objects, the wires between them, and embedded images.
//!
//! Every item on the board is an [`Object`] with a rectangle and a port in the middle of each side.
//! A [`Wire`] joins a port on one object to a port on another, so the board is a graph whatever the
//! objects are: images, sketches, shapes, flowchart boxes, text or groups of these. Wires show
//! relationships; they do not carry data.

use crate::palettes;
use crate::theme::Pal;
use egui::{Color32, Pos2, Rect, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type Id = u64;

/// Space around a group's members, in world units. The top is taller to fit the name.
pub const GROUP_PAD: f32 = 16.0;
pub const GROUP_PAD_TOP: f32 = 36.0;

/// A colour that is either fixed or follows the theme's text colour (so ink stays visible in both modes).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Ink {
    Auto,
    Rgb([u8; 3]),
}

impl Ink {
    pub fn color(self, p: &Pal) -> Color32 {
        match self {
            Ink::Auto => p.text,
            Ink::Rgb([r, g, b]) => Color32::from_rgb(r, g, b),
        }
    }
}

/// Swatches offered in Properties: theme ink, then the first eight categorical colours.
pub fn swatches() -> [Ink; 9] {
    let mut s = [Ink::Auto; 9];
    for (i, slot) in s.iter_mut().enumerate().skip(1) {
        *slot = Ink::Rgb(palettes::cat_color(i - 1));
    }
    s
}

/// How a line is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Dash {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    /// Line colour (and text colour for text items).
    pub ink: Ink,
    pub width: f32,
    /// Shapes and groups: fill the inside.
    pub fill: bool,
    pub font_size: f32,
    /// Fill colour. `None` is automatic: the line colour, lightly.
    pub fill_color: Option<Ink>,
    /// Fill fades from the fill colour at the top towards the board at the bottom.
    pub gradient: bool,
    /// Shapes: draw the outline.
    #[serde(default = "yes")]
    pub line: bool,
    pub dash: Dash,
    /// Corner radius in world units. `None` uses the shape's own (a process box is rounded, a
    /// rectangle nearly square); `Some(0)` is square.
    pub radius: Option<f32>,
    pub shadow: bool,
    /// A soft highlight over the top half.
    pub glass: bool,
    /// Hand-drawn outline.
    pub sketch: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            ink: Ink::Auto,
            width: 2.0,
            fill: false,
            font_size: 16.0,
            fill_color: None,
            gradient: false,
            line: true,
            dash: Dash::Solid,
            radius: None,
            shadow: false,
            glass: false,
            sketch: false,
        }
    }
}

/// A ready-made look: line colour, fill and effects, offered as a grid in Properties.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub ink: Ink,
    pub fill: bool,
    pub fill_color: Option<Ink>,
    pub gradient: bool,
}

impl Preset {
    pub fn apply(&self, s: &mut Style) {
        s.ink = self.ink;
        s.fill = self.fill;
        s.fill_color = self.fill_color;
        s.gradient = self.gradient;
        s.line = true;
    }
}

/// A colour blended towards black (`t` 0 to 1), for outlines around solid fills.
pub fn darker(c: [u8; 3], t: f32) -> [u8; 3] {
    c.map(|v| (v as f32 * (1.0 - t)) as u8)
}

/// Pages of presets: tinted, solid, gradient, outline only. Each page starts with the theme colour.
pub fn preset_pages() -> Vec<(&'static str, Vec<Preset>)> {
    let colors: Vec<[u8; 3]> = (0..7).map(palettes::cat_color).collect();
    let tint = std::iter::once(Preset { ink: Ink::Auto, fill: true, fill_color: None, gradient: false })
        .chain(colors.iter().map(|c| Preset { ink: Ink::Rgb(*c), fill: true, fill_color: None, gradient: false }))
        .collect();
    let solid = std::iter::once(Preset { ink: Ink::Auto, fill: true, fill_color: Some(Ink::Rgb([72, 72, 78])), gradient: false })
        .chain(colors.iter().map(|c| Preset { ink: Ink::Rgb(darker(*c, 0.35)), fill: true, fill_color: Some(Ink::Rgb(*c)), gradient: false }))
        .collect();
    let gradient = std::iter::once(Preset { ink: Ink::Auto, fill: true, fill_color: Some(Ink::Rgb([90, 90, 98])), gradient: true })
        .chain(colors.iter().map(|c| Preset { ink: Ink::Rgb(darker(*c, 0.35)), fill: true, fill_color: Some(Ink::Rgb(*c)), gradient: true }))
        .collect();
    let outline = std::iter::once(Preset { ink: Ink::Auto, fill: false, fill_color: None, gradient: false })
        .chain(colors.iter().map(|c| Preset { ink: Ink::Rgb(*c), fill: false, fill_color: None, gradient: false }))
        .collect();
    vec![("Tinted", tint), ("Solid", solid), ("Gradient", gradient), ("Outline", outline)]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Shape {
    Rectangle,
    Ellipse,
    /// A plain node with nothing in it.
    Node,
    /// Flowchart: a step (rounded box).
    Process,
    /// Flowchart: a yes/no branch.
    Decision,
    /// Flowchart: start or end (pill).
    Terminal,
    /// Flowchart: input or output (slanted box).
    Data,
    /// Flowchart: a document (wavy bottom).
    Document,
    /// Flowchart: stored data (cylinder).
    Database,
    /// Flowchart: a step defined elsewhere (box with side bars).
    Subprocess,
    /// Flowchart: typed input (sloped top).
    ManualInput,
    /// Flowchart: a jump to another part of the chart (small circle).
    Connector,
    /// A note with a folded corner.
    Note,
}

impl Shape {
    /// Shapes offered by the flowchart box tool.
    pub const FLOW: [Shape; 11] = [
        Shape::Node,
        Shape::Process,
        Shape::Decision,
        Shape::Terminal,
        Shape::Data,
        Shape::Document,
        Shape::Database,
        Shape::Subprocess,
        Shape::ManualInput,
        Shape::Connector,
        Shape::Note,
    ];
    /// Every shape, for the shape menu of a selected item.
    pub const ALL: [Shape; 13] = [
        Shape::Node,
        Shape::Process,
        Shape::Decision,
        Shape::Terminal,
        Shape::Data,
        Shape::Document,
        Shape::Database,
        Shape::Subprocess,
        Shape::ManualInput,
        Shape::Connector,
        Shape::Note,
        Shape::Rectangle,
        Shape::Ellipse,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Shape::Rectangle => "Rectangle",
            Shape::Ellipse => "Ellipse",
            Shape::Node => "Empty node",
            Shape::Process => "Process",
            Shape::Decision => "Decision",
            Shape::Terminal => "Start or end",
            Shape::Data => "Input or output",
            Shape::Document => "Document",
            Shape::Database => "Database",
            Shape::Subprocess => "Subprocess",
            Shape::ManualInput => "Manual input",
            Shape::Connector => "Connector",
            Shape::Note => "Note",
        }
    }

    /// World size when the shape is placed with a click.
    pub fn default_size(self) -> Vec2 {
        match self {
            Shape::Node => vec2(100.0, 56.0),
            Shape::Connector => vec2(48.0, 48.0),
            Shape::Decision => vec2(150.0, 96.0),
            Shape::Database => vec2(120.0, 90.0),
            Shape::Document => vec2(160.0, 80.0),
            Shape::Note => vec2(160.0, 110.0),
            Shape::Ellipse => vec2(120.0, 90.0),
            Shape::Rectangle => vec2(140.0, 90.0),
            _ => vec2(160.0, 64.0),
        }
    }

    /// Shapes that start with a label being typed when they are created.
    pub fn takes_label(self) -> bool {
        !matches!(self, Shape::Node | Shape::Connector | Shape::Rectangle | Shape::Ellipse)
    }
}

/// How a freehand stroke is painted.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Brush {
    #[default]
    Pen,
    Pencil,
    Marker,
    Highlighter,
}

impl Brush {
    pub const ALL: [Brush; 4] = [Brush::Pen, Brush::Pencil, Brush::Marker, Brush::Highlighter];

    pub fn index(self) -> usize {
        self as usize
    }
    /// Opacity of the ink. Markers and highlighters let what is underneath show through.
    pub fn alpha(self) -> f32 {
        match self {
            Brush::Pen => 1.0,
            Brush::Pencil => 0.78,
            Brush::Marker => 0.62,
            Brush::Highlighter => 0.32,
        }
    }
    /// Starting width in world units.
    pub fn default_width(self) -> f32 {
        match self {
            Brush::Pen => 3.0,
            Brush::Pencil => 1.5,
            Brush::Marker => 12.0,
            Brush::Highlighter => 28.0,
        }
    }
}

/// Smallest and largest brush width, in world units.
pub const MIN_WIDTH: f32 = 0.25;
pub const MAX_WIDTH: f32 = 200.0;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Kind {
    Image { image: Id },
    /// Freehand stroke or straight line. Points are local to the rect and were recorded at `base` size;
    /// they scale with the rect when it is resized.
    Path {
        points: Vec<[f32; 2]>,
        base: [f32; 2],
        arrow: bool,
        #[serde(default)]
        brush: Brush,
        /// Polylines: the last point joins back to the first.
        #[serde(default)]
        closed: bool,
        /// Polylines: a smooth curve through the points instead of straight segments.
        #[serde(default)]
        curved: bool,
    },
    Shape { shape: Shape },
    Text,
    /// A named, coloured frame around other objects. Its rect follows its members. A collapsed group
    /// hides its members and shows as one compact node; their connections attach to it instead.
    Group {
        members: Vec<Id>,
        #[serde(default)]
        collapsed: bool,
        /// A sketch: strokes joined into one item, with no frame or name.
        #[serde(default)]
        sketch: bool,
    },
}

/// Size of a collapsed group, in world units.
pub const COLLAPSED_SIZE: Vec2 = vec2(200.0, 64.0);

/// One comment in a node's thread.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Comment {
    pub text: String,
    /// Local time it was written, `YYYY-MM-DD HH:MM:SS`.
    pub time: String,
}

fn one() -> f32 {
    1.0
}

impl Kind {
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Image { .. } => "Image",
            Kind::Path { points, arrow, brush, closed, curved, .. } => {
                if *arrow {
                    "Arrow"
                } else if *closed {
                    if *curved { "Closed curve" } else { "Polygon" }
                } else if *curved {
                    "Curve"
                } else if points.len() == 2 {
                    "Line"
                } else {
                    match brush {
                        Brush::Pen => "Pen stroke",
                        Brush::Pencil => "Pencil stroke",
                        Brush::Marker => "Marker stroke",
                        Brush::Highlighter => "Highlight",
                    }
                }
            }
            Kind::Shape { shape } => shape.name(),
            Kind::Text => "Text",
            Kind::Group { sketch: true, .. } => "Sketch",
            Kind::Group { .. } => "Group",
        }
    }
}

/// One of the four ports on an object.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum Side {
    Left,
    Top,
    Right,
    Bottom,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Left, Side::Top, Side::Right, Side::Bottom];

    /// Outward direction from the object.
    pub fn normal(self) -> Vec2 {
        match self {
            Side::Left => vec2(-1.0, 0.0),
            Side::Right => vec2(1.0, 0.0),
            Side::Top => vec2(0.0, -1.0),
            Side::Bottom => vec2(0.0, 1.0),
        }
    }

    /// The side of `r` that faces point `p`.
    pub fn facing(r: Rect, p: Pos2) -> Side {
        let d = p - r.center();
        // Compare in units of the rect's half-size so wide boxes still pick top or bottom sensibly.
        let (nx, ny) = (d.x / r.width().max(1.0), d.y / r.height().max(1.0));
        if nx.abs() > ny.abs() {
            if d.x < 0.0 { Side::Left } else { Side::Right }
        } else if d.y < 0.0 {
            Side::Top
        } else {
            Side::Bottom
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Object {
    pub id: Id,
    /// World rectangle: x, y, width, height.
    pub rect: [f32; 4],
    pub kind: Kind,
    pub label: String,
    pub style: Style,
    /// A locked item can be selected but not moved, resized, edited, erased or deleted.
    #[serde(default)]
    pub locked: bool,
    /// 0 to 1.
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub comments: Vec<Comment>,
    /// The layer it is on. 0 until it is placed on one.
    #[serde(default)]
    pub layer: Id,
}

impl Object {
    pub fn new(id: Id, rect: Rect, kind: Kind, label: String, style: Style) -> Self {
        let mut o = Object { id, rect: [0.0; 4], kind, label, style, locked: false, opacity: 1.0, comments: Vec::new(), layer: 0 };
        o.set_rect(rect);
        o
    }
    pub fn rect(&self) -> Rect {
        let [x, y, w, h] = self.rect;
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }
    pub fn set_rect(&mut self, r: Rect) {
        self.rect = [r.min.x, r.min.y, r.width(), r.height()];
    }
    pub fn translate(&mut self, d: Vec2) {
        self.rect[0] += d.x;
        self.rect[1] += d.y;
    }
    pub fn port(&self, side: Side) -> Pos2 {
        let r = self.rect();
        match side {
            Side::Left => pos2(r.left(), r.center().y),
            Side::Right => pos2(r.right(), r.center().y),
            Side::Top => pos2(r.center().x, r.top()),
            Side::Bottom => pos2(r.center().x, r.bottom()),
        }
    }
    pub fn is_group(&self) -> bool {
        matches!(self.kind, Kind::Group { .. })
    }
    pub fn is_sketch(&self) -> bool {
        matches!(self.kind, Kind::Group { sketch: true, .. })
    }
    /// The line as drawn, in world space: curved polylines are sampled, closed ones end where they start.
    pub fn line_points(&self) -> Vec<Pos2> {
        let pts = self.world_points();
        match &self.kind {
            Kind::Path { closed, curved: true, .. } => catmull_rom(&pts, *closed),
            Kind::Path { closed: true, .. } => {
                let mut out = pts;
                if let Some(f) = out.first().copied() {
                    out.push(f);
                }
                out
            }
            _ => pts,
        }
    }
    /// Path points in world space.
    pub fn world_points(&self) -> Vec<Pos2> {
        let Kind::Path { points, base, .. } = &self.kind else { return Vec::new() };
        let r = self.rect();
        let sx = if base[0] > 0.0 { r.width() / base[0] } else { 1.0 };
        let sy = if base[1] > 0.0 { r.height() / base[1] } else { 1.0 };
        points.iter().map(|p| pos2(r.min.x + p[0] * sx, r.min.y + p[1] * sy)).collect()
    }
    /// Does a world point hit this object? `tol` is in world units.
    pub fn hit(&self, p: Pos2, tol: f32) -> bool {
        match &self.kind {
            Kind::Path { closed, .. } => {
                let pts = self.line_points();
                let t = tol + self.style.width * 0.5;
                pts.windows(2).any(|s| dist_to_segment(p, s[0], s[1]) <= t) || (pts.len() == 1 && pts[0].distance(p) <= t) || (*closed && self.style.fill && inside(&pts, p))
            }
            // A sketch is hit through its strokes, never its empty box.
            Kind::Group { sketch: true, .. } => false,
            _ => self.rect().expand(tol).contains(p),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum WireStyle {
    #[default]
    Curve,
    Straight,
    Elbow,
}

impl WireStyle {
    pub fn name(self) -> &'static str {
        match self {
            WireStyle::Curve => "Curve",
            WireStyle::Straight => "Straight",
            WireStyle::Elbow => "Elbow",
        }
    }
}

fn default_from_side() -> Side {
    Side::Right
}
fn default_to_side() -> Side {
    Side::Left
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Wire {
    #[serde(default)]
    pub id: Id,
    pub from: Id,
    #[serde(default = "default_from_side")]
    pub from_side: Side,
    pub to: Id,
    #[serde(default = "default_to_side")]
    pub to_side: Side,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub style: WireStyle,
}

/// An image embedded in the board, stored as its original encoded bytes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageAsset {
    #[serde(with = "b64")]
    pub bytes: Arc<Vec<u8>>,
    pub size: [u32; 2],
}

/// Images never change once added, so two assets are equal when they share the same bytes.
impl PartialEq for ImageAsset {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.bytes, &other.bytes) || (self.size == other.size && self.bytes == other.bytes)
    }
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Board {
    /// Draw order: first is at the back.
    pub objects: Vec<Object>,
    pub wires: Vec<Wire>,
    pub images: BTreeMap<Id, ImageAsset>,
    pub next_id: Id,
    /// Back to front. Every object is on one; their order decides what is in front.
    #[serde(default)]
    pub layers: Vec<Layer>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: Id,
    pub name: String,
}

impl Board {
    pub fn new_id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }
    pub fn get(&self, id: Id) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }
    pub fn get_mut(&mut self, id: Id) -> Option<&mut Object> {
        self.objects.iter_mut().find(|o| o.id == id)
    }
    pub fn wire(&self, id: Id) -> Option<&Wire> {
        self.wires.iter().find(|w| w.id == id)
    }
    pub fn wire_mut(&mut self, id: Id) -> Option<&mut Wire> {
        self.wires.iter_mut().find(|w| w.id == id)
    }
    pub fn add(&mut self, rect: Rect, kind: Kind, style: Style) -> Id {
        let id = self.new_id();
        self.objects.push(Object::new(id, rect, kind, String::new(), style));
        id
    }
    /// Topmost visible object under a world point. Groups sit behind their members, so members win;
    /// a stroke inside a sketch gives the sketch.
    pub fn hit(&self, p: Pos2, tol: f32) -> Option<Id> {
        // Work out what is hidden only if something is actually under the point.
        let mut hidden: Option<std::collections::HashSet<Id>> = None;
        self.objects
            .iter()
            .rev()
            .find(|o| o.hit(p, tol) && !hidden.get_or_insert_with(|| self.hidden_set()).contains(&o.id))
            .map(|o| self.outer_sketch(o.id))
    }

    /// The outermost sketch around `id`, or `id` itself.
    pub fn outer_sketch(&self, id: Id) -> Id {
        let mut out = id;
        while let Some(p) = self.parent(out).filter(|p| self.get(*p).is_some_and(|o| o.is_sketch())) {
            out = p;
        }
        out
    }

    /// Strokes inside sketches. They are drawn, but are picked, connected and listed as their sketch.
    pub fn sketch_inner(&self) -> std::collections::HashSet<Id> {
        if !self.objects.iter().any(|o| o.is_sketch()) {
            return Default::default();
        }
        let members: Vec<Id> = self.objects.iter().filter(|o| o.is_sketch()).flat_map(|o| self.members(o.id)).collect();
        self.with_descendants(&members).into_iter().collect()
    }

    // ── layers ──

    /// Make sure there is a layer, put objects that are on none onto `active`, and keep the object list
    /// in layer order (back layer first). Cheap when nothing needs doing.
    pub fn fix_layers(&mut self, active: &mut Id) {
        if self.layers.is_empty() {
            let id = self.new_id();
            self.layers.push(Layer { id, name: "Layer 1".into() });
        }
        if !self.layers.iter().any(|l| l.id == *active) {
            *active = self.layers[self.layers.len() - 1].id;
        }
        let rank: std::collections::HashMap<Id, usize> = self.layers.iter().enumerate().map(|(i, l)| (l.id, i)).collect();
        for o in &mut self.objects {
            if !rank.contains_key(&o.layer) {
                o.layer = *active;
            }
        }
        if !self.objects.windows(2).all(|w| rank[&w[0].layer] <= rank[&w[1].layer]) {
            self.objects.sort_by_key(|o| rank[&o.layer]);
        }
    }

    /// Put objects (and everything inside groups among them) on a layer.
    pub fn set_layer(&mut self, ids: &[Id], layer: Id) {
        let ids = self.with_descendants(ids);
        for o in &mut self.objects {
            if ids.contains(&o.id) {
                o.layer = layer;
            }
        }
    }

    /// Index and collapsed-group stand-ins, built once so a whole frame of drawing can look items up
    /// without scanning the board each time.
    pub fn lookup(&self) -> Lookup {
        let index = self.objects.iter().enumerate().map(|(i, o)| (o.id, i)).collect();
        let mut proxy = std::collections::HashMap::new();
        for g in self.objects.iter().filter(|o| matches!(o.kind, Kind::Group { collapsed: true, .. })) {
            let stand_in = self.proxy(g.id);
            for d in self.with_descendants(&self.members(g.id)) {
                proxy.insert(d, stand_in);
            }
        }
        Lookup { index, proxy, inner: self.sketch_inner() }
    }

    /// Is `id` inside a collapsed group (at any depth)?
    pub fn hidden(&self, id: Id) -> bool {
        let mut cur = id;
        while let Some(p) = self.parent(cur) {
            if matches!(self.get(p).map(|o| &o.kind), Some(Kind::Group { collapsed: true, .. })) {
                return true;
            }
            cur = p;
        }
        false
    }

    /// Every item inside a collapsed group, computed once (cheaper than asking `hidden` per item).
    pub fn hidden_set(&self) -> std::collections::HashSet<Id> {
        let collapsed: Vec<Id> = self.objects.iter().filter(|o| matches!(o.kind, Kind::Group { collapsed: true, .. })).map(|o| o.id).collect();
        let members: Vec<Id> = collapsed.iter().flat_map(|g| self.members(*g)).collect();
        self.with_descendants(&members).into_iter().collect()
    }

    /// What stands in for `id` on screen: the outermost collapsed group around it, or itself.
    pub fn proxy(&self, id: Id) -> Id {
        let mut cur = id;
        let mut out = id;
        while let Some(p) = self.parent(cur) {
            if matches!(self.get(p).map(|o| &o.kind), Some(Kind::Group { collapsed: true, .. })) {
                out = p;
            }
            cur = p;
        }
        out
    }

    pub fn set_collapsed(&mut self, gid: Id, value: bool) {
        if let Some(Object { kind: Kind::Group { collapsed, .. }, .. }) = self.get_mut(gid) {
            *collapsed = value;
        }
        self.update_groups();
    }

    /// Give wires from older files an id.
    pub fn fix_after_load(&mut self) {
        let max = self.objects.iter().map(|o| o.id).chain(self.wires.iter().map(|w| w.id)).chain(self.images.keys().copied()).max().unwrap_or(0);
        self.next_id = self.next_id.max(max);
        for k in 0..self.wires.len() {
            if self.wires[k].id == 0 {
                let id = self.new_id();
                self.wires[k].id = id;
            }
        }
        self.update_groups();
    }

    // ── groups ──

    /// The group that directly contains `id`.
    pub fn parent(&self, id: Id) -> Option<Id> {
        self.objects.iter().find(|o| matches!(&o.kind, Kind::Group { members, .. } if members.contains(&id))).map(|o| o.id)
    }

    pub fn members(&self, id: Id) -> Vec<Id> {
        match self.get(id).map(|o| &o.kind) {
            Some(Kind::Group { members, .. }) => members.clone(),
            _ => Vec::new(),
        }
    }

    /// `ids` plus everything inside any groups among them, at any depth.
    pub fn with_descendants(&self, ids: &[Id]) -> Vec<Id> {
        let mut out: Vec<Id> = Vec::new();
        let mut stack: Vec<Id> = ids.to_vec();
        while let Some(id) = stack.pop() {
            if out.contains(&id) {
                continue;
            }
            out.push(id);
            stack.extend(self.members(id));
        }
        out
    }

    /// Fit every group around its members (inner groups first).
    pub fn update_groups(&mut self) {
        if !self.objects.iter().any(|o| o.is_group()) {
            return;
        }
        for _ in 0..8 {
            let mut changed = false;
            let rects: std::collections::HashMap<Id, Rect> = self.objects.iter().map(|o| (o.id, o.rect())).collect();
            for k in 0..self.objects.len() {
                let Kind::Group { members, collapsed, sketch } = &self.objects[k].kind else { continue };
                let Some(b) = members.iter().filter_map(|m| rects.get(m).copied()).reduce(|a, b| a.union(b)) else { continue };
                let min = b.min - vec2(GROUP_PAD, GROUP_PAD_TOP);
                // A collapsed group keeps its top-left corner and shrinks to a compact node. A sketch has
                // no frame, so it fits its strokes exactly.
                let r = if *sketch {
                    b
                } else if *collapsed {
                    Rect::from_min_size(min, COLLAPSED_SIZE)
                } else {
                    Rect::from_min_max(min, b.max + vec2(GROUP_PAD, GROUP_PAD))
                };
                if (r.min - self.objects[k].rect().min).length() > 0.01 || (r.size() - self.objects[k].rect().size()).length() > 0.01 {
                    self.objects[k].set_rect(r);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Put `ids` into a new group. Objects already in a group leave it; if they all came from the same
    /// group, the new group goes inside that one. Returns the new group's id.
    pub fn group(&mut self, ids: &[Id], style: Style) -> Option<Id> {
        self.group_as(ids, style, false)
    }

    /// Join strokes into one sketch. Sketches among them are merged in rather than nested.
    pub fn sketch(&mut self, ids: &[Id], style: Style) -> Option<Id> {
        let mut flat = Vec::new();
        for id in ids {
            if self.get(*id).is_some_and(|o| o.is_sketch()) {
                flat.extend(self.ungroup(*id));
            } else {
                flat.push(*id);
            }
        }
        self.group_as(&flat, style, true)
    }

    fn group_as(&mut self, ids: &[Id], style: Style, sketch: bool) -> Option<Id> {
        // Skip anything already inside another selected group.
        let inner: Vec<Id> = ids.iter().flat_map(|id| self.with_descendants(&[*id]).into_iter().skip(1)).collect();
        let ids: Vec<Id> = ids.iter().copied().filter(|id| !inner.contains(id) && self.get(*id).is_some()).collect();
        if ids.is_empty() {
            return None;
        }
        let parents: Vec<Option<Id>> = ids.iter().map(|id| self.parent(*id)).collect();
        let common = parents[0].filter(|p| parents.iter().all(|q| *q == Some(*p)));
        for o in &mut self.objects {
            if let Kind::Group { members, .. } = &mut o.kind {
                members.retain(|m| !ids.contains(m));
            }
        }
        let gid = self.new_id();
        let at = self.objects.iter().position(|o| ids.contains(&o.id)).unwrap_or(0);
        let layer = self.objects.get(at).map(|o| o.layer).unwrap_or(0);
        let mut g = Object::new(gid, Rect::ZERO, Kind::Group { members: ids, collapsed: false, sketch }, if sketch { String::new() } else { "Group".into() }, style);
        g.layer = layer;
        self.objects.insert(at, g);
        if let Some(p) = common {
            if let Some(Object { kind: Kind::Group { members, .. }, .. }) = self.get_mut(p) {
                members.push(gid);
            }
        }
        self.update_groups();
        Some(gid)
    }

    /// Remove a group but keep its members (they move to the group's own parent, if any).
    pub fn ungroup(&mut self, gid: Id) -> Vec<Id> {
        let members = self.members(gid);
        if let Some(p) = self.parent(gid) {
            if let Some(Object { kind: Kind::Group { members: pm, .. }, .. }) = self.get_mut(p) {
                pm.extend(members.iter().copied());
            }
        }
        self.remove(&[gid]);
        members
    }

    pub fn remove(&mut self, ids: &[Id]) {
        let mut ids = ids.to_vec();
        loop {
            self.objects.retain(|o| !ids.contains(&o.id));
            self.wires.retain(|w| !ids.contains(&w.from) && !ids.contains(&w.to));
            let mut empty = Vec::new();
            for o in &mut self.objects {
                if let Kind::Group { members, .. } = &mut o.kind {
                    members.retain(|m| !ids.contains(m));
                    if members.is_empty() {
                        empty.push(o.id);
                    }
                }
            }
            if empty.is_empty() {
                break;
            }
            ids = empty;
        }
        self.gc_images();
        self.update_groups();
    }

    /// Drop images that no object uses any more.
    pub fn gc_images(&mut self) {
        let used: Vec<Id> = self.objects.iter().filter_map(|o| if let Kind::Image { image } = o.kind { Some(image) } else { None }).collect();
        self.images.retain(|id, _| used.contains(id));
    }

    // ── wires ──

    /// Adds a wire unless it would loop on one object or join the same two ports twice. Returns its id.
    pub fn connect(&mut self, from: Id, from_side: Side, to: Id, to_side: Side, style: WireStyle) -> Option<Id> {
        if from == to || self.get(from).is_none() || self.get(to).is_none() {
            return None;
        }
        // A group and its own contents are not connected to each other.
        if self.with_descendants(&[from]).contains(&to) || self.with_descendants(&[to]).contains(&from) {
            return None;
        }
        if self.wires.iter().any(|w| w.from == from && w.to == to && w.from_side == from_side && w.to_side == to_side) {
            return None;
        }
        let id = self.new_id();
        self.wires.push(Wire { id, from, from_side, to, to_side, label: String::new(), style });
        Some(id)
    }

    /// World points of a wire's line, from its start port to its end port.
    /// Connections to items inside a collapsed group attach to the group; ones that stay inside it are not drawn.
    pub fn wire_points(&self, w: &Wire) -> Option<Vec<Pos2>> {
        let (pa, pb) = (self.proxy(w.from), self.proxy(w.to));
        if pa == pb {
            return None;
        }
        let (a, b) = (self.get(pa)?, self.get(pb)?);
        Some(route(a.port(w.from_side), w.from_side.normal(), b.port(w.to_side), Some(w.to_side.normal()), w.style))
    }

    pub fn inputs(&self, id: Id) -> impl Iterator<Item = Id> + '_ {
        self.wires.iter().filter(move |w| w.to == id).map(|w| w.from)
    }
    pub fn outputs(&self, id: Id) -> impl Iterator<Item = Id> + '_ {
        self.wires.iter().filter(move |w| w.from == id).map(|w| w.to)
    }
    /// Area covered by everything visible (items inside collapsed groups are left out).
    pub fn bounds(&self) -> Option<Rect> {
        let hidden = self.hidden_set();
        self.objects.iter().filter(|o| !hidden.contains(&o.id)).map(|o| o.rect()).reduce(|a, b| a.union(b))
    }
    /// Move the given objects to the front (`front`) or back, keeping their relative order.
    pub fn reorder(&mut self, ids: &[Id], front: bool) {
        let ids = self.with_descendants(ids);
        let (picked, rest): (Vec<Object>, Vec<Object>) = std::mem::take(&mut self.objects).into_iter().partition(|o| ids.contains(&o.id));
        self.objects = if front { rest.into_iter().chain(picked).collect() } else { picked.into_iter().chain(rest).collect() };
    }
    /// A label to show for an object in lists: its text, or its kind and number.
    pub fn display_name(&self, id: Id) -> String {
        match self.get(id) {
            Some(o) if !o.label.trim().is_empty() => o.label.lines().next().unwrap_or_default().chars().take(40).collect(),
            Some(o) => format!("{} {}", o.kind.name(), o.id),
            None => "missing".into(),
        }
    }
}

/// See `Board::lookup`.
pub struct Lookup {
    index: std::collections::HashMap<Id, usize>,
    /// Items inside collapsed groups, and the group that stands in for each.
    proxy: std::collections::HashMap<Id, Id>,
    /// Strokes inside sketches: drawn, but without ports or controls of their own.
    inner: std::collections::HashSet<Id>,
}

impl Lookup {
    pub fn get<'a>(&self, b: &'a Board, id: Id) -> Option<&'a Object> {
        self.index.get(&id).and_then(|i| b.objects.get(*i)).filter(|o| o.id == id)
    }
    pub fn hidden(&self, id: Id) -> bool {
        self.proxy.contains_key(&id)
    }
    pub fn in_sketch(&self, id: Id) -> bool {
        self.inner.contains(&id)
    }
    pub fn proxy(&self, id: Id) -> Id {
        self.proxy.get(&id).copied().unwrap_or(id)
    }
    /// Same as `Board::wire_points`, without scanning the board.
    pub fn wire_points(&self, b: &Board, w: &Wire) -> Option<Vec<Pos2>> {
        let (pa, pb) = (self.proxy(w.from), self.proxy(w.to));
        if pa == pb {
            return None;
        }
        let (a, c) = (self.get(b, pa)?, self.get(b, pb)?);
        Some(route(a.port(w.from_side), w.from_side.normal(), c.port(w.to_side), Some(w.to_side.normal()), w.style))
    }
}

/// The line of a wire. `to_normal` is `None` while the end is not on a port yet.
pub fn route(from: Pos2, n0: Vec2, to: Pos2, to_normal: Option<Vec2>, style: WireStyle) -> Vec<Pos2> {
    let n3 = to_normal.unwrap_or_else(|| (from - to).normalized());
    match style {
        WireStyle::Straight => vec![from, to],
        WireStyle::Curve => {
            let d = (from.distance(to) * 0.4).max(40.0);
            sample_curve(&[from, from + n0 * d, to + n3 * d, to], 32)
        }
        WireStyle::Elbow => {
            const STUB: f32 = 20.0;
            let a = from + n0 * STUB;
            let b = to + n3 * STUB;
            let mut pts = vec![from, a];
            if n0.x != 0.0 && n3.x != 0.0 {
                let mx = (a.x + b.x) * 0.5;
                pts.extend([pos2(mx, a.y), pos2(mx, b.y)]);
            } else if n0.y != 0.0 && n3.y != 0.0 {
                let my = (a.y + b.y) * 0.5;
                pts.extend([pos2(a.x, my), pos2(b.x, my)]);
            } else if n0.x != 0.0 {
                pts.push(pos2(b.x, a.y));
            } else {
                pts.push(pos2(a.x, b.y));
            }
            pts.extend([b, to]);
            pts
        }
    }
}

/// Round off the corners of a freehand stroke (Chaikin's method), keeping both ends in place.
pub fn smooth(points: &[Pos2], passes: usize) -> Vec<Pos2> {
    let mut pts = points.to_vec();
    for _ in 0..passes {
        if pts.len() < 3 {
            break;
        }
        let mut out = Vec::with_capacity(pts.len() * 2);
        out.push(pts[0]);
        for s in pts.windows(2) {
            out.push(s[0] + (s[1] - s[0]) * 0.25);
            out.push(s[0] + (s[1] - s[0]) * 0.75);
        }
        out.push(pts[pts.len() - 1]);
        pts = out;
    }
    pts
}

/// Remove points that lie within `eps` of the line through their neighbours (Ramer-Douglas-Peucker),
/// keeping both ends. The shape stays the same to within `eps`.
pub fn simplify(points: &[Pos2], eps: f32) -> Vec<Pos2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (mut far, mut dist) = (0, eps);
        for i in a + 1..b {
            let d = dist_to_segment(points[i], points[a], points[b]);
            if d > dist {
                far = i;
                dist = d;
            }
        }
        if far != 0 {
            keep[far] = true;
            stack.push((a, far));
            stack.push((far, b));
        }
    }
    points.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

/// The point halfway along a polyline.
pub fn midpoint(pts: &[Pos2]) -> Pos2 {
    let total: f32 = pts.windows(2).map(|s| s[0].distance(s[1])).sum();
    let mut left = total * 0.5;
    for s in pts.windows(2) {
        let l = s[0].distance(s[1]);
        if l >= left && l > 0.0 {
            return s[0] + (s[1] - s[0]) * (left / l);
        }
        left -= l;
    }
    pts.first().copied().unwrap_or_default()
}

/// A smooth curve through every point (Catmull-Rom), as a polyline.
pub fn catmull_rom(pts: &[Pos2], closed: bool) -> Vec<Pos2> {
    let n = pts.len();
    if n < 3 {
        let mut v = pts.to_vec();
        if closed && n > 0 {
            v.push(pts[0]);
        }
        return v;
    }
    let at = |i: isize| -> Pos2 { if closed { pts[i.rem_euclid(n as isize) as usize] } else { pts[i.clamp(0, n as isize - 1) as usize] } };
    let segs = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(segs * 12 + 1);
    for i in 0..segs as isize {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let steps = ((p1.distance(p2) / 6.0).ceil() as usize).clamp(4, 32);
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3);
            out.push(pos2(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y)));
        }
    }
    out.push(if closed { pts[0] } else { pts[n - 1] });
    out
}

/// Is `p` inside the polygon `pts` (even-odd)?
pub fn inside(pts: &[Pos2], p: Pos2) -> bool {
    let n = pts.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
    }
    c
}

pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 <= f32::EPSILON {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

pub fn sample_curve(c: &[Pos2; 4], n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            pos2(
                c.iter().zip(w).map(|(p, w)| p.x * w).sum(),
                c.iter().zip(w).map(|(p, w)| p.y * w).sum(),
            )
        })
        .collect()
}

/// Saved file: the board plus the view it was left at.
#[derive(Serialize, Deserialize)]
pub struct FileV1 {
    pub format: String,
    pub version: u32,
    pub board: Board,
    pub camera: crate::canvas::Camera,
    /// States and branches (version 2 and later).
    #[serde(default)]
    pub history: Option<crate::history::HistoryFile>,
}

pub const FILE_FORMAT: &str = "overlay-board";

mod b64 {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub fn serialize<S: Serializer>(v: &Arc<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(v.as_slice()))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Arc<Vec<u8>>, D::Error> {
        let s = String::deserialize(d)?;
        STANDARD.decode(s).map(Arc::new).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(b: &mut Board, x: f32) -> Id {
        b.add(Rect::from_min_size(pos2(x, 0.0), vec2(10.0, 10.0)), Kind::Text, Style::default())
    }

    #[test]
    fn wires_follow_removal() {
        let mut b = Board::default();
        let a = square(&mut b, 0.0);
        let c = square(&mut b, 20.0);
        assert!(b.connect(a, Side::Right, c, Side::Left, WireStyle::Curve).is_some());
        assert!(b.connect(a, Side::Right, c, Side::Left, WireStyle::Curve).is_none(), "duplicate");
        assert!(b.connect(a, Side::Bottom, c, Side::Top, WireStyle::Curve).is_some(), "other ports are fine");
        assert!(b.connect(a, Side::Left, a, Side::Right, WireStyle::Curve).is_none(), "self loop");
        b.remove(&[c]);
        assert!(b.wires.is_empty());
    }

    #[test]
    fn path_scales_with_rect() {
        let mut b = Board::default();
        let id = b.add(
            Rect::from_min_size(Pos2::ZERO, vec2(10.0, 10.0)),
            Kind::Path { points: vec![[0.0, 0.0], [10.0, 10.0]], base: [10.0, 10.0], arrow: false, brush: Brush::Pen, closed: false, curved: false },
            Style::default(),
        );
        b.get_mut(id).unwrap().set_rect(Rect::from_min_size(pos2(5.0, 5.0), vec2(20.0, 20.0)));
        assert_eq!(b.get(id).unwrap().world_points()[1], pos2(25.0, 25.0));
        assert!(b.get(id).unwrap().hit(pos2(15.0, 15.0), 1.0));
    }

    #[test]
    fn groups_fit_nest_and_dissolve() {
        let mut b = Board::default();
        let a = square(&mut b, 0.0);
        let c = square(&mut b, 100.0);
        let d = square(&mut b, 200.0);
        let g = b.group(&[a, c], Style::default()).unwrap();
        let r = b.get(g).unwrap().rect();
        assert_eq!(r.min, pos2(-GROUP_PAD, -GROUP_PAD_TOP));
        assert_eq!(r.max, pos2(110.0 + GROUP_PAD, 10.0 + GROUP_PAD));
        // The group draws behind its members.
        let gi = b.objects.iter().position(|o| o.id == g).unwrap();
        let ai = b.objects.iter().position(|o| o.id == a).unwrap();
        assert!(gi < ai);
        // A group nested inside: grouping the outer group with d.
        let outer = b.group(&[g, d], Style::default()).unwrap();
        assert_eq!(b.parent(g), Some(outer));
        assert_eq!(b.with_descendants(&[outer]).len(), 5);
        // A group can be wired, but not to its own contents.
        assert!(b.connect(g, Side::Right, d, Side::Left, WireStyle::Curve).is_some());
        assert!(b.connect(outer, Side::Right, a, Side::Left, WireStyle::Curve).is_none());
        // Ungrouping keeps members and hands them to the parent.
        b.ungroup(g);
        assert!(b.get(g).is_none());
        assert_eq!(b.parent(a), Some(outer));
        // Removing every member removes the group.
        b.remove(&[a, c, d]);
        assert!(b.get(outer).is_none());
    }

    #[test]
    fn collapsed_group_hides_and_stands_in() {
        let mut b = Board::default();
        let a = square(&mut b, 0.0);
        let c = square(&mut b, 100.0);
        let d = square(&mut b, 300.0);
        let g = b.group(&[a, c], Style::default()).unwrap();
        let w = b.connect(a, Side::Right, d, Side::Left, WireStyle::Curve).unwrap();
        let inner = b.connect(a, Side::Right, c, Side::Left, WireStyle::Curve).unwrap();
        b.set_collapsed(g, true);
        assert!(b.hidden(a) && !b.hidden(g) && !b.hidden(d));
        assert_eq!(b.proxy(a), g);
        assert_eq!(b.get(g).unwrap().rect().size(), COLLAPSED_SIZE);
        // The wire from a now starts at the group; the one inside it is not drawn.
        let pts = b.wire_points(b.wire(w).unwrap()).unwrap();
        assert_eq!(pts[0], b.get(g).unwrap().port(Side::Right));
        assert!(b.wire_points(b.wire(inner).unwrap()).is_none());
        // Hidden members cannot be clicked.
        assert_eq!(b.hit(pos2(5.0, 5.0), 0.0), Some(g));
        b.set_collapsed(g, false);
        assert_eq!(b.hit(pos2(5.0, 5.0), 0.0), Some(a));
    }

    #[test]
    fn smoothing_keeps_ends() {
        let pts = [pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(10.0, 10.0)];
        let s = smooth(&pts, 2);
        assert_eq!(s.first(), Some(&pts[0]));
        assert_eq!(s.last(), Some(&pts[2]));
        assert!(s.len() > pts.len());
    }

    #[test]
    fn old_paths_are_pen() {
        let k: Kind = serde_json::from_str(r#"{"Path":{"points":[[0,0],[1,1],[2,0]],"base":[2,1],"arrow":false}}"#).unwrap();
        assert!(matches!(k, Kind::Path { brush: Brush::Pen, .. }));
    }

    #[test]
    fn facing_side() {
        let r = Rect::from_min_size(Pos2::ZERO, vec2(200.0, 50.0));
        assert_eq!(Side::facing(r, pos2(100.0, -100.0)), Side::Top);
        assert_eq!(Side::facing(r, pos2(400.0, 30.0)), Side::Right);
    }

    #[test]
    fn old_wires_load() {
        let json = r#"{"objects":[],"wires":[{"from":1,"to":2}],"images":{},"next_id":2}"#;
        let mut b: Board = serde_json::from_str(json).unwrap();
        b.fix_after_load();
        assert_eq!(b.wires[0].from_side, Side::Right);
        assert_eq!(b.wires[0].to_side, Side::Left);
        assert!(b.wires[0].id > 2);
    }

    #[test]
    fn file_round_trip() {
        let mut b = Board::default();
        let img = b.new_id();
        b.images.insert(img, ImageAsset { bytes: Arc::new(vec![1, 2, 3]), size: [1, 1] });
        b.add(Rect::from_min_size(Pos2::ZERO, vec2(1.0, 1.0)), Kind::Image { image: img }, Style::default());
        let f = FileV1 { format: FILE_FORMAT.into(), version: 1, board: b, camera: Default::default(), history: None };
        let s = serde_json::to_string(&f).unwrap();
        let back: FileV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back.board.images[&img].bytes.as_slice(), &[1, 2, 3]);
    }
}
