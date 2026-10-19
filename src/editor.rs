//! Editor state and every operation on the board (history, clipboard, images, files).
//! The canvas (`canvas.rs`) and the panels (`panels.rs`) both drive the board through this.

use crate::board::{Board, Brush, Comment, FILE_FORMAT, FileV1, Id, ImageAsset, Ink, Kind, Object, Shape, Style, Wire, WireStyle};
use crate::canvas::{Camera, Drag};
use crate::history::History;
use crate::tools::Tool;
use egui::{Pos2, Rect, TextureHandle, Vec2, vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Written to the system clipboard when objects are copied, so a paste knows to use the internal copy.
const CLIPBOARD_MARKER: &str = "overlay:objects";

/// What the eyedropper's colour goes to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    Fill,
    Line,
}

#[derive(Clone, Default)]
struct Clip {
    objects: Vec<Object>,
    wires: Vec<Wire>,
    images: Vec<(Id, ImageAsset)>,
}

pub struct Editor {
    pub board: Board,
    /// Every state of the board, in branches. Undo and redo move through it.
    pub history: History,
    /// When the board last changed without being recorded yet.
    pending: Option<Instant>,
    pub dirty: bool,
    pub path: Option<PathBuf>,

    pub camera: Camera,
    pub selection: Vec<Id>,
    pub selected_wire: Option<Id>,
    pub tool: Tool,
    /// Style for new objects.
    pub style: Style,
    /// Shape made by the flowchart box tool.
    pub node_shape: Shape,
    /// Line style for new connections.
    pub wire_style: WireStyle,
    /// Width of each freehand brush (pen, pencil, marker, highlighter), so each keeps its own size.
    pub brush_widths: [f32; 4],
    /// New items go on this layer.
    pub active_layer: Id,
    /// Corners of the polyline being drawn, in world space.
    pub poly: Vec<Pos2>,
    /// The polyline tool draws a smooth curve through its points.
    pub poly_curved: bool,
    /// The radial menu, while it is open.
    pub radial: Option<crate::radial::Radial>,
    /// The radial menu asked for the Properties panel to be shown.
    pub want_properties: bool,
    /// Calendar: weeks before (negative) or after this one.
    pub cal_week: i64,
    /// Calendar: the chosen days, `YYYY-MM-DD`.
    pub cal_days: Vec<String>,
    /// What the chosen days do to the board.
    pub day_mode: crate::dates::DayMode,
    /// While highlighting: everything shown at full strength, and the items from the chosen days.
    pub(crate) day_shown: std::collections::HashSet<Id>,
    pub(crate) day_direct: std::collections::HashSet<Id>,

    /// Object whose label is being typed on the canvas.
    pub editing: Option<Id>,
    /// Connection whose label is being typed on the canvas.
    pub editing_wire: Option<Id>,
    /// Waiting for a click on the board to take a colour from.
    pub eyedropper: Option<Pick>,
    /// Snap to other items while moving and resizing (Alt skips it for one drag).
    pub snapping: bool,
    /// Guides to draw while a drag is snapped.
    pub(crate) guides: Vec<crate::snap::Guide>,
    /// The "/" list for adding an item, if open.
    pub insert: Option<crate::insert::InsertMenu>,
    /// Object whose comment thread is open.
    pub comments_open: Option<Id>,
    pub comment_draft: String,
    pub(crate) edit_focus: bool,
    pub(crate) edit_rect: Option<Rect>,
    pub(crate) drag: Drag,
    pub(crate) swallow: bool,
    pub(crate) textures: HashMap<Id, Option<TextureHandle>>,
    pub(crate) canvas_rect: Rect,
    /// Pointer position in world space while it is over the canvas.
    pub(crate) hover_world: Option<Pos2>,
    pub fit_pending: bool,

    /// One-line report of the last action, shown in secondary grey.
    pub status: Option<(String, Instant)>,
    clip: Clip,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            board: Board::default(),
            history: History::new(&Board::default(), "New board"),
            pending: None,
            dirty: false,
            path: None,
            camera: Camera::default(),
            selection: Vec::new(),
            selected_wire: None,
            tool: Tool::Select,
            style: Style::default(),
            node_shape: Shape::Process,
            wire_style: WireStyle::Curve,
            brush_widths: Brush::ALL.map(|b| b.default_width()),
            active_layer: 0,
            poly: Vec::new(),
            poly_curved: false,
            radial: None,
            want_properties: false,
            cal_week: 0,
            cal_days: Vec::new(),
            day_mode: crate::dates::DayMode::All,
            day_shown: Default::default(),
            day_direct: Default::default(),
            editing: None,
            editing_wire: None,
            eyedropper: None,
            snapping: true,
            guides: Vec::new(),
            insert: None,
            comments_open: None,
            comment_draft: String::new(),
            edit_focus: false,
            edit_rect: None,
            drag: Drag::None,
            swallow: false,
            textures: HashMap::new(),
            canvas_rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
            hover_world: None,
            fit_pending: false,
            status: None,
            clip: Clip::default(),
        }
    }
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl Editor {
    pub fn say(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), Instant::now()));
    }

    /// The width the current tool draws with: the brush's own width for freehand tools, otherwise the
    /// shared stroke width for lines and shapes.
    pub fn tool_width(&self) -> f32 {
        match self.tool.brush() {
            Some(b) => self.brush_widths[b.index()],
            None => self.style.width,
        }
    }

    pub fn set_tool_width(&mut self, w: f32) {
        let w = w.clamp(crate::board::MIN_WIDTH, crate::board::MAX_WIDTH);
        match self.tool.brush() {
            Some(b) => self.brush_widths[b.index()] = w,
            None => self.style.width = w,
        }
    }

    /// Keep the per-editor settings when the board is replaced.
    fn fresh(&self) -> Editor {
        Editor {
            style: self.style,
            node_shape: self.node_shape,
            wire_style: self.wire_style,
            brush_widths: self.brush_widths,
            tool: self.tool,
            snapping: self.snapping,
            ..Editor::default()
        }
    }

    /// Apply the Calendar's chosen days to the board: hide or fade the items from other days.
    pub fn apply_day_filter(&mut self) {
        use crate::dates::DayMode;
        if self.cal_days.is_empty() || self.day_mode == DayMode::All {
            if !self.board.veil.is_empty() || !self.day_shown.is_empty() {
                self.board.veil.clear();
                self.day_shown.clear();
                self.day_direct.clear();
            }
            return;
        }
        let (shown, direct) = crate::dates::matching(&self.board, &self.cal_days);
        if self.day_mode == DayMode::Only {
            self.board.veil = self.board.objects.iter().map(|o| o.id).filter(|id| !shown.contains(id)).collect();
            self.day_shown.clear();
            self.day_direct.clear();
            let veil = &self.board.veil;
            self.selection.retain(|id| !veil.contains(id));
        } else {
            self.board.veil.clear();
            self.day_shown = shown;
            self.day_direct = direct;
        }
    }

    // ── history ──

    /// The board has changed. It becomes a state once the change settles (see `tick`).
    pub fn mark_changed(&mut self) {
        self.pending = Some(Instant::now());
        self.dirty = true;
    }

    /// Record a state now if a change is waiting.
    pub fn flush(&mut self) {
        if self.pending.take().is_some() {
            // New and changed items get today's date.
            crate::dates::stamp(&self.history.current().board, &mut self.board, &crate::history::now());
            if let Some((change, branched)) = self.history.record(&self.board) {
                if branched {
                    let name = self.history.branch(self.history.current().branch).map(|b| b.name.clone()).unwrap_or_default();
                    self.say(format!("{}. Started {name}; the earlier version is kept.", change.label));
                }
            }
        }
    }

    /// Called every frame. A change becomes a state once nothing is being dragged or typed and the
    /// board has been still for a moment, so one drag or one burst of slider moves is one state.
    pub fn tick(&mut self, ctx: &egui::Context) {
        let Some(t) = self.pending else { return };
        let busy = ctx.input(|i| i.pointer.any_down()) || self.is_editing() || !matches!(self.drag, Drag::None);
        let wait = std::time::Duration::from_millis(400);
        if !busy && t.elapsed() >= wait {
            self.flush();
        } else {
            ctx.request_repaint_after(wait);
        }
    }

    pub fn can_undo(&self) -> bool {
        self.pending.is_some() || self.history.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.pending.is_none() && self.history.can_redo()
    }

    pub fn undo(&mut self) {
        self.finish_edit();
        self.flush();
        if let Some(b) = self.history.undo() {
            self.board = b;
            self.after_history();
        }
    }

    pub fn redo(&mut self) {
        self.finish_edit();
        self.flush();
        if let Some(b) = self.history.redo() {
            self.board = b;
            self.after_history();
        }
    }

    /// Show a state from the History panel.
    pub fn jump_to_state(&mut self, id: crate::history::StateId) {
        self.finish_edit();
        self.flush();
        if let Some(b) = self.history.jump(id) {
            self.board = b;
            self.after_history();
        }
    }

    /// Delete several states and branches at once (branches first, then the states still there).
    pub fn delete_history_selection(&mut self, states: &[crate::history::StateId], branches: &[crate::history::BranchId]) {
        self.finish_edit();
        self.flush();
        let was = self.history.current;
        let (mut s_done, mut b_done, mut skipped) = (0, 0, 0);
        for b in branches {
            if self.history.delete_branch(*b).is_ok() { b_done += 1 } else { skipped += 1 }
        }
        for s in states {
            if self.history.state(*s).is_none() {
                continue; // went with its branch
            }
            if self.history.delete_state(*s).is_ok() { s_done += 1 } else { skipped += 1 }
        }
        if self.history.current != was {
            self.board = self.history.current().board.to_board();
            self.after_history();
        }
        self.dirty = true;
        let mut msg = format!("Deleted {} and {}", plural(s_done, "state", "states"), plural(b_done, "branch", "branches"));
        if skipped > 0 {
            msg.push_str(&format!("; {skipped} kept (the first state and the first branch stay)"));
        }
        self.say(msg);
    }

    /// Keep only the selection and the path to it (see `History::keep_only`).
    pub fn keep_history_only(&mut self, states: &[crate::history::StateId], branches: &[crate::history::BranchId]) {
        self.finish_edit();
        self.flush();
        let was = self.history.current;
        match self.history.keep_only(states, branches) {
            Ok(n) => {
                if self.history.current != was {
                    self.board = self.history.current().board.to_board();
                    self.after_history();
                }
                self.dirty = true;
                self.say(format!("Kept the selection; deleted {}", plural(n, "other state", "other states")));
            }
            Err(e) => self.say(e),
        }
    }

    fn after_history(&mut self) {
        self.drag = Drag::None;
        self.dirty = true;
        let b = &self.board;
        self.selection.retain(|id| b.get(*id).is_some());
        if self.selected_wire.is_some_and(|w| b.wire(w).is_none()) {
            self.selected_wire = None;
        }
        if self.comments_open.is_some_and(|id| b.get(id).is_none()) {
            self.comments_open = None;
        }
    }

    // ── lock, opacity, comments, collapse ──

    pub fn is_locked(&self, id: Id) -> bool {
        self.board.get(id).is_some_and(|o| o.locked)
    }

    /// Lock the selection, or unlock it if everything in it is already locked.
    pub fn toggle_lock(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let lock = !self.selection.iter().all(|id| self.is_locked(*id));
        for id in self.selection.clone() {
            if let Some(o) = self.board.get_mut(id) {
                o.locked = lock;
            }
        }
        self.mark_changed();
    }

    pub fn toggle_collapsed(&mut self, gid: Id) {
        let Some(Kind::Group { collapsed, sketch: false, .. }) = self.board.get(gid).map(|o| o.kind.clone()) else { return };
        self.board.set_collapsed(gid, !collapsed);
        // Hidden items cannot stay selected.
        let b = &self.board;
        self.selection.retain(|id| !b.hidden(*id));
        self.mark_changed();
    }

    pub fn add_comment(&mut self, id: Id, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if let Some(o) = self.board.get_mut(id) {
            o.comments.push(Comment { text: text.to_owned(), time: crate::history::now() });
            self.mark_changed();
        }
    }

    pub fn delete_comment(&mut self, id: Id, index: usize) {
        if let Some(o) = self.board.get_mut(id) {
            if index < o.comments.len() {
                o.comments.remove(index);
                self.mark_changed();
            }
        }
    }

    // ── selection ──

    pub fn select_all(&mut self) {
        let inner = self.board.sketch_inner();
        self.selection = self.board.objects.iter().map(|o| o.id).filter(|id| !inner.contains(id)).collect();
        self.selected_wire = None;
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selected_wire = None;
    }

    /// Deletes the selected objects (a group goes with everything in it), or the selected connection.
    pub fn delete_selection(&mut self) {
        if !self.selection.is_empty() {
            // Locked items, and groups holding anything locked, stay.
            let ids: Vec<Id> = self.selection.iter().copied().filter(|id| !self.board.with_descendants(&[*id]).iter().any(|x| self.is_locked(*x))).collect();
            let kept = self.selection.len() - ids.len();
            if ids.is_empty() {
                self.say("Locked items cannot be deleted. Unlock them first (Ctrl+L).");
                return;
            }
            self.mark_changed();
            self.selection.retain(|id| !ids.contains(id));
            let all = self.board.with_descendants(&ids);
            self.board.remove(&all);
            let mut msg = format!("Deleted {}", plural(all.len(), "object", "objects"));
            if kept > 0 {
                msg.push_str(&format!("; {} locked, kept", plural(kept, "item is", "items are")));
            }
            self.say(msg);
        } else if let Some(w) = self.selected_wire.take() {
            self.mark_changed();
            self.board.wires.retain(|x| x.id != w);
            self.say("Deleted 1 connection");
        }
    }

    pub fn reorder_selection(&mut self, front: bool) {
        if self.selection.is_empty() {
            return;
        }
        self.mark_changed();
        let ids = self.selection.clone();
        self.board.reorder(&ids, front);
    }

    pub fn nudge(&mut self, d: Vec2) {
        if self.selection.is_empty() {
            return;
        }
        self.mark_changed();
        let unlocked: Vec<Id> = self.selection.iter().copied().filter(|id| !self.is_locked(*id)).collect();
        for id in self.board.with_descendants(&unlocked) {
            if let Some(o) = self.board.get_mut(id).filter(|o| !o.locked) {
                o.translate(d);
            }
        }
        self.board.update_groups();
    }

    // ── groups ──

    pub fn group_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let ids = self.selection.clone();
        // Strokes alone join into one sketch, with no frame around them.
        let strokes = ids.iter().all(|id| self.board.get(*id).is_some_and(|o| matches!(o.kind, Kind::Path { .. }) || o.is_sketch()));
        if strokes && ids.len() > 1 {
            if let Some(g) = self.board.sketch(&ids, Style::default()) {
                self.mark_changed();
                let n = self.board.members(g).len();
                self.selection = vec![g];
                self.selected_wire = None;
                self.say(format!("Joined {} into one sketch. Ctrl+Shift+G splits it again.", plural(n, "stroke", "strokes")));
            }
            return;
        }
        let style = Style { ink: Ink::Auto, width: 1.5, fill: true, font_size: 15.0, ..Style::default() };
        if let Some(g) = self.board.group(&ids, style) {
            self.mark_changed();
            let n = self.board.members(g).len();
            self.selection = vec![g];
            self.selected_wire = None;
            self.say(format!("Grouped {}. Double-click the name to rename it.", plural(n, "object", "objects")));
        }
    }

    pub fn ungroup_selection(&mut self) {
        let groups: Vec<Id> = self.selection.iter().copied().filter(|id| self.board.get(*id).is_some_and(|o| o.is_group())).collect();
        if groups.is_empty() {
            return;
        }
        self.mark_changed();
        let mut freed = Vec::new();
        for g in groups {
            freed.extend(self.board.ungroup(g));
        }
        self.say(format!("Ungrouped {}", plural(freed.len(), "object", "objects")));
        self.selection = freed;
    }

    pub fn any_group_selected(&self) -> bool {
        self.selection.iter().any(|id| self.board.get(*id).is_some_and(|o| o.is_group()))
    }

    // ── text editing ──

    /// Start editing an object's label on the canvas. Locked items are not edited.
    pub fn start_edit(&mut self, id: Id) {
        self.finish_edit();
        self.selection = vec![id];
        self.selected_wire = None;
        if self.is_locked(id) {
            self.say("This item is locked. Unlock it to edit it.");
            return;
        }
        self.editing = Some(id);
        self.edit_focus = true;
    }

    /// Start typing a connection's label on the canvas.
    pub fn start_wire_edit(&mut self, id: Id) {
        self.finish_edit();
        self.editing_wire = Some(id);
        self.edit_focus = true;
        self.selection.clear();
        self.selected_wire = Some(id);
    }

    pub fn is_editing(&self) -> bool {
        self.editing.is_some() || self.editing_wire.is_some()
    }

    /// Stop typing. An empty new text box is removed. The history only records a state if the board
    /// actually differs from the last one, so an edit that changed nothing leaves no trace.
    pub fn finish_edit(&mut self) {
        if self.editing_wire.take().is_some() {
            self.edit_rect = None;
            self.mark_changed();
            return;
        }
        let Some(id) = self.editing.take() else { return };
        self.edit_rect = None;
        let empty_text = self.board.get(id).is_some_and(|o| matches!(o.kind, Kind::Text) && o.label.trim().is_empty());
        if empty_text {
            self.board.remove(&[id]);
            self.selection.retain(|x| *x != id);
        }
        self.mark_changed();
    }

    // ── clipboard ──

    fn clip_from_selection(&self) -> Clip {
        let ids = &self.board.with_descendants(&self.selection);
        let objects: Vec<Object> = self.board.objects.iter().filter(|o| ids.contains(&o.id)).cloned().collect();
        let wires = self.board.wires.iter().filter(|w| ids.contains(&w.from) && ids.contains(&w.to)).cloned().collect();
        let images = objects
            .iter()
            .filter_map(|o| if let Kind::Image { image } = o.kind { self.board.images.get(&image).map(|a| (image, a.clone())) } else { None })
            .collect();
        Clip { objects, wires, images }
    }

    pub fn copy(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.clip = self.clip_from_selection();
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(CLIPBOARD_MARKER);
        }
        self.say(format!("Copied {}", plural(self.clip.objects.len(), "object", "objects")));
    }

    pub fn cut(&mut self) {
        self.copy();
        self.delete_selection();
    }

    /// Paste the internal clip with new ids, offset by `offset`.
    fn paste_clip(&mut self, clip: &Clip, offset: Vec2) {
        if clip.objects.is_empty() {
            return;
        }
        self.mark_changed();
        let mut map: HashMap<Id, Id> = HashMap::new();
        let mut img_map: HashMap<Id, Id> = HashMap::new();
        for (old, asset) in &clip.images {
            let new = self.board.new_id();
            self.board.images.insert(new, asset.clone());
            img_map.insert(*old, new);
        }
        let mut new_ids = Vec::new();
        for o in &clip.objects {
            let mut n = o.clone();
            n.id = self.board.new_id();
            n.translate(offset);
            if let Kind::Image { image } = &mut n.kind {
                if let Some(x) = img_map.get(image) {
                    *image = *x;
                }
            }
            // Pasted items land on the current layer, and are new today.
            n.layer = self.active_layer;
            n.created.clear();
            n.days.clear();
            map.insert(o.id, n.id);
            new_ids.push(n.id);
            self.board.objects.push(n);
        }
        // Groups point at the new copies of their members.
        for id in &new_ids {
            if let Some(Object { kind: Kind::Group { members, .. }, .. }) = self.board.get_mut(*id) {
                *members = members.iter().filter_map(|m| map.get(m).copied()).collect();
            }
        }
        for w in &clip.wires {
            if let (Some(a), Some(b)) = (map.get(&w.from), map.get(&w.to)) {
                if let Some(nid) = self.board.connect(*a, w.from_side, *b, w.to_side, w.style) {
                    if let Some(nw) = self.board.wire_mut(nid) {
                        nw.label = w.label.clone();
                    }
                }
            }
        }
        self.board.update_groups();
        self.say(format!("Pasted {}", plural(new_ids.len(), "object", "objects")));
        // Select only the top level of what was pasted.
        let inner: Vec<Id> = new_ids.iter().flat_map(|id| self.board.members(*id)).collect();
        new_ids.retain(|id| !inner.contains(id));
        self.selection = new_ids;
        self.selected_wire = None;
    }

    pub fn duplicate(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let clip = self.clip_from_selection();
        self.paste_clip(&clip, vec2(24.0, 24.0));
    }

    /// Paste from the system clipboard: our own objects, an image (for example a screenshot) or text.
    pub fn paste(&mut self) {
        let at = self.paste_point();
        let mut cb = match arboard::Clipboard::new() {
            Ok(cb) => cb,
            Err(_) => {
                let clip = self.clip.clone();
                return self.paste_clip(&clip, vec2(24.0, 24.0));
            }
        };
        let text = cb.get_text().ok();
        if text.as_deref() == Some(CLIPBOARD_MARKER) {
            let clip = self.clip.clone();
            if let Some(b) = clip.objects.iter().map(|o| o.rect()).reduce(|a, b| a.union(b)) {
                return self.paste_clip(&clip, at - b.center());
            }
            return;
        }
        if let Ok(img) = cb.get_image() {
            match encode_png(img.width as u32, img.height as u32, &img.bytes) {
                Ok(png) => {
                    let _ = self.import_image(png, at, None);
                }
                Err(e) => self.say(format!("Clipboard image could not be read: {e}")),
            }
            return;
        }
        if let Some(t) = text.filter(|t| !t.trim().is_empty()) {
            self.mark_changed();
            let style = Style { ink: self.style.ink, ..Style::default() };
            let id = self.board.add(Rect::from_min_size(at, vec2(320.0, 24.0)), Kind::Text, style);
            if let Some(o) = self.board.get_mut(id) {
                o.label = t;
            }
            self.selection = vec![id];
            self.say("Pasted text");
        } else {
            self.say("Nothing to paste");
        }
    }

    /// Centre of the visible canvas in world space, or the pointer if it is over the canvas.
    pub fn paste_point(&self) -> Pos2 {
        self.hover_world.unwrap_or_else(|| self.camera.to_world(self.canvas_rect.min, self.canvas_rect.center()))
    }

    // ── images ──

    /// Add an image to the board with its centre at `at`. Returns the object id.
    pub fn import_image(&mut self, bytes: Vec<u8>, at: Pos2, name: Option<&str>) -> Result<Id, String> {
        let img = image::load_from_memory(&bytes).map_err(|e| {
            let msg = format!("{} could not be opened: {e}", name.unwrap_or("The image"));
            self.say(msg.clone());
            msg
        })?;
        let (w, h) = (img.width(), img.height());
        self.mark_changed();
        let image = self.board.new_id();
        self.board.images.insert(image, ImageAsset { bytes: Arc::new(bytes), size: [w, h] });
        // Natural size, capped so a large photo does not cover the whole board.
        let scale = (640.0 / w.max(h) as f32).min(1.0);
        let size = vec2(w as f32 * scale, h as f32 * scale);
        let id = self.board.add(Rect::from_center_size(at, size), Kind::Image { image }, Style::default());
        if let Some(n) = name {
            if let Some(o) = self.board.get_mut(id) {
                o.label = n.to_owned();
            }
        }
        self.selection = vec![id];
        self.selected_wire = None;
        self.say(format!("Added image {w} x {h}"));
        Ok(id)
    }

    pub fn import_image_files(&mut self, paths: &[PathBuf], at: Pos2) {
        let mut added = 0;
        let mut ids = Vec::new();
        for (k, p) in paths.iter().enumerate() {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned());
            match std::fs::read(p) {
                Ok(bytes) => {
                    if let Ok(id) = self.import_image(bytes, at + vec2(32.0, 32.0) * k as f32, name.as_deref()) {
                        ids.push(id);
                        added += 1;
                    }
                }
                Err(e) => self.say(format!("{} could not be read: {e}", p.display())),
            }
        }
        if added > 1 {
            self.selection = ids;
            self.say(format!("Added {}", plural(added, "image", "images")));
        }
    }

    pub fn pick_images(&mut self) {
        let at = self.paste_point();
        self.pick_images_at(at);
    }

    /// Ask for image files and place them around `at`.
    pub fn pick_images_at(&mut self, at: Pos2) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp", "tif", "tiff"])
            .pick_files()
        {
            self.import_image_files(&paths, at);
        }
    }

    // ── view ──

    pub fn zoom_to_fit(&mut self) {
        let r = self.canvas_rect;
        match self.board.bounds() {
            Some(b) => self.camera.fit(r, b.expand(40.0)),
            None => self.camera = Camera::centered(r),
        }
    }

    pub fn reset_zoom(&mut self) {
        let r = self.canvas_rect;
        let c = self.camera.to_world(r.min, r.center());
        self.camera.zoom = 1.0;
        self.camera.look_at(r, c);
    }

    // ── files ──

    pub fn file_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Untitled".into())
    }

    pub fn new_board(&mut self) {
        *self = self.fresh();
        self.fit_pending = true;
    }

    pub fn load(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let f: FileV1 = serde_json::from_str(&text).map_err(|e| format!("not an Overlay board ({e})"))?;
        if f.format != FILE_FORMAT {
            return Err("not an Overlay board".into());
        }
        *self = self.fresh();
        let pool = f.board.images.clone();
        self.board = f.board;
        self.board.fix_after_load();
        // The file keeps every image any state uses; the board only needs its own.
        self.board.gc_images();
        self.history = match f.history.and_then(|h| History::from_file(h, &pool)) {
            Some(h) => {
                // Boards from before items had dates take them from their history.
                crate::dates::backfill(&h, &mut self.board);
                h
            }
            None => History::new(&self.board, "Opened"),
        };
        self.camera = f.camera;
        self.path = Some(path.to_owned());
        let n = self.board.objects.len();
        let branches = self.history.branches.len();
        let mut msg = format!("Opened {}, {}", plural(n, "object", "objects"), plural(self.board.wires.len(), "connection", "connections"));
        if branches > 1 {
            msg.push_str(&format!(", {}", plural(branches, "branch", "branches")));
        }
        self.say(msg);
        Ok(())
    }

    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        self.finish_edit();
        self.flush();
        // Every image any state uses is stored once, with the current board.
        let mut board = self.board.clone();
        for (k, v) in self.history.all_images() {
            board.images.entry(k).or_insert(v);
        }
        let f = FileV1 { format: FILE_FORMAT.into(), version: 2, board, camera: self.camera, history: Some(self.history.to_file()) };
        let json = serde_json::to_string(&f).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("overlay.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        self.path = Some(path.to_owned());
        self.dirty = false;
        self.say(format!("Saved {}", self.file_name()));
        Ok(())
    }

    /// Save to the current file, or ask for one. Returns false if the user cancelled or saving failed.
    pub fn save(&mut self, save_as: bool) -> bool {
        let path = match (&self.path, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new().add_filter("Overlay board", &["overlay"]).set_file_name(format!("{}.overlay", self.file_name().trim_end_matches(".overlay"))).save_file(),
        };
        let Some(path) = path else { return false };
        match self.save_to(&path) {
            Ok(()) => true,
            Err(e) => {
                self.say(format!("Could not save: {e}"));
                false
            }
        }
    }

    pub fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Overlay board", &["overlay"]).pick_file() {
            if let Err(e) = self.load(&path) {
                self.say(format!("Could not open {}: {e}", path.display()));
            } else {
                self.fit_pending = false;
            }
        }
    }

    /// Are there changes that closing or replacing the board would lose? An empty, never-saved board
    /// has nothing worth asking about.
    pub fn needs_save(&self) -> bool {
        (self.dirty || self.pending.is_some()) && !(self.board.objects.is_empty() && self.path.is_none())
    }
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or("unexpected size")?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}
