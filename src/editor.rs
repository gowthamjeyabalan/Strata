//! Editor state and every operation on the board (history, clipboard, images, files).
//! The canvas (`canvas.rs`) and the panels (`panels.rs`) both drive the board through this.

use crate::board::{Board, Brush, Comment, FILE_FORMAT, FileV1, Id, ImageAsset, Ink, Kind, Object, Shape, Style, Wire, WireStyle};
use crate::canvas::{Camera, Drag};
use crate::history::Undo;
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
    /// The last changes, for undo and redo.
    pub history: Undo,
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
    /// The page showing; new items go on it.
    pub page: Id,
    /// Presenting: which of the page's frames is showing.
    pub presenting: Option<usize>,
    /// The view before presenting, to return to.
    pub(crate) present_cam: Option<Camera>,
    /// Where each page's view was left, to come back to it.
    pub(crate) page_cams: HashMap<Id, Camera>,
    /// A page whose name is being typed, and the name so far.
    pub(crate) renaming_page: Option<(Id, String)>,
    /// Where the last brush stroke ended: Shift+click draws a straight stroke from here.
    pub(crate) brush_last: Option<Pos2>,
    /// Corners of the polyline being drawn, in world space.
    pub poly: Vec<Pos2>,
    /// The polyline tool draws a smooth curve through its points.
    pub poly_curved: bool,
    /// Search everything (Ctrl+K), while it is open.
    pub palette: Option<crate::search::Palette>,
    /// The radial menu, while it is open.
    pub radial: Option<crate::radial::Radial>,
    /// Right button held on empty board: where (screen, world) and since when. Held still, it opens
    /// the radial menu; moved, it pans.
    pub(crate) radial_hold: Option<(Pos2, Pos2, Instant)>,
    /// The radial menu asked for the Properties panel to be shown.
    pub want_properties: bool,
    /// The Overview window is showing.
    pub show_overview: bool,
    /// The grid of dots behind the board.
    pub show_grid: bool,
    /// Holding the right button still on empty board opens the radial menu.
    pub radial_empty: bool,
    /// How long that hold is, in milliseconds.
    pub radial_hold_ms: u32,
    /// How many changes undo keeps.
    pub undo_steps: usize,
    /// The keyboard shortcuts.
    pub keys: crate::keys::Keymap,
    /// Dotted lines join the cards of notes that link to each other.
    pub show_note_links: bool,
    /// The board was drawn this frame (its tab is the one showing).
    pub(crate) canvas_visible: bool,
    /// A note to open in its own tab beside the board (taken by the app).
    pub open_note: Option<Id>,
    pub note_query: String,
    /// Note tabs: 0 write, 1 split, 2 read.
    pub note_mode: u8,
    /// Layers opened in the Layers panel to list their items.
    pub layers_open: std::collections::HashSet<Id>,
    /// The image being cropped.
    pub cropping: Option<Id>,
    /// Colours picked in the colour picker, newest first.
    pub custom_colors: Vec<[u8; 3]>,
    /// The colour picker window is open, starting from this colour.
    pub colour_pick: Option<Ink>,
    /// Pictures shown in notes, by source, loaded once (none if the source could not be read).
    pub(crate) note_pictures: std::cell::RefCell<HashMap<String, Option<(egui::TextureHandle, egui::Vec2)>>>,
    /// Notes picked in the Notes list.
    pub notes_sel: Vec<Id>,
    /// The Notes list was clicked last (rather than the board), so Delete deletes notes.
    pub notes_focus: bool,
    /// The folder picked in the Notes list; new notes go in it. 0 for none.
    pub notes_folder: Id,
    /// Folders folded shut in the Notes list.
    pub folded: std::collections::HashSet<Id>,
    /// A folder whose name is being typed, and the name so far.
    pub(crate) renaming_folder: Option<(Id, String)>,
    /// How far each note card is scrolled, in world units.
    pub(crate) note_scroll: HashMap<Id, f32>,
    /// Each note card as last drawn: content height (world units) and what can be clicked (screen).
    pub(crate) note_view: std::cell::RefCell<HashMap<Id, (f32, Vec<(Rect, crate::markdown::Target)>)>>,
    /// A web link clicked in a note, to open.
    pub(crate) open_url: Option<String>,
    /// Calendar: the month showing (its first day); none is this month.
    pub cal_month: Option<chrono::NaiveDate>,
    /// Calendar: the chosen days, `YYYY-MM-DD`.
    pub cal_days: Vec<String>,
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
            history: Undo::new(&Board::default(), crate::history::STEPS),
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
            page: 0,
            presenting: None,
            present_cam: None,
            page_cams: HashMap::new(),
            renaming_page: None,
            brush_last: None,
            poly: Vec::new(),
            poly_curved: false,
            palette: None,
            radial: None,
            radial_hold: None,
            want_properties: false,
            show_overview: true,
            show_grid: true,
            radial_empty: true,
            radial_hold_ms: 300,
            undo_steps: crate::history::STEPS,
            keys: Default::default(),
            show_note_links: true,
            canvas_visible: false,
            open_note: None,
            note_query: String::new(),
            note_mode: 1,
            layers_open: Default::default(),
            cropping: None,
            custom_colors: Vec::new(),
            colour_pick: None,
            note_pictures: Default::default(),
            notes_sel: Vec::new(),
            notes_focus: false,
            notes_folder: 0,
            folded: Default::default(),
            renaming_folder: None,
            note_scroll: HashMap::new(),
            note_view: Default::default(),
            open_url: None,
            cal_month: None,
            cal_days: Vec::new(),
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
    /// A new editor for another board, keeping the settings.
    fn fresh(&self) -> Editor {
        Editor {
            style: self.style,
            node_shape: self.node_shape,
            wire_style: self.wire_style,
            brush_widths: self.brush_widths,
            tool: self.tool,
            snapping: self.snapping,
            poly_curved: self.poly_curved,
            show_overview: self.show_overview,
            show_note_links: self.show_note_links,
            show_grid: self.show_grid,
            radial_empty: self.radial_empty,
            radial_hold_ms: self.radial_hold_ms,
            undo_steps: self.undo_steps,
            note_mode: self.note_mode,
            keys: self.keys.clone(),
            custom_colors: self.custom_colors.clone(),
            history: Undo::new(&Board::default(), self.undo_steps),
            ..Editor::default()
        }
    }

    // ── notes ──

    /// Write a new note (not on the board yet). `open` opens it in a tab to write in.
    pub fn add_note(&mut self, text: &str, open: bool) -> Id {
        let id = self.board.new_id();
        // New notes go in the folder picked in the Notes list.
        let folder = if self.board.folders.iter().any(|f| f.id == self.notes_folder) { self.notes_folder } else { 0 };
        self.board.notes.push(crate::board::Note { id, text: text.to_owned(), created: String::new(), folder });
        self.mark_changed();
        if open {
            self.open_note = Some(id);
        }
        id
    }

    /// A new layer above the current one, which becomes current. Returns it and its name.
    pub fn add_layer(&mut self) -> (Id, String) {
        let id = self.board.new_id();
        let n = (1..).find(|n| !self.board.layers.iter().any(|l| l.name == format!("Layer {n}"))).unwrap_or(1);
        let at = self.board.layers.iter().position(|l| l.id == self.active_layer).map(|i| i + 1).unwrap_or(self.board.layers.len());
        self.mark_changed();
        self.board.layers.insert(at, crate::board::Layer { id, name: format!("Layer {n}"), hidden: false, locked: false, color: None });
        self.active_layer = id;
        (id, format!("Layer {n}"))
    }

    pub fn add_folder(&mut self) -> Id {
        let id = self.board.new_id();
        let n = (1..).find(|n| !self.board.folders.iter().any(|f| f.name == format!("Folder {n}"))).unwrap_or(1);
        self.board.folders.push(crate::board::Folder { id, name: format!("Folder {n}") });
        self.mark_changed();
        self.notes_folder = id;
        self.renaming_folder = Some((id, format!("Folder {n}")));
        id
    }

    /// Delete a folder; its notes stay, outside any folder.
    pub fn delete_folder(&mut self, id: Id) {
        self.mark_changed();
        self.board.folders.retain(|f| f.id != id);
        for n in &mut self.board.notes {
            if n.folder == id {
                n.folder = 0;
            }
        }
        if self.notes_folder == id {
            self.notes_folder = 0;
        }
    }

    /// Put notes in a folder (0 for none).
    pub fn move_notes(&mut self, ids: &[Id], folder: Id) {
        self.mark_changed();
        for n in &mut self.board.notes {
            if ids.contains(&n.id) {
                n.folder = folder;
            }
        }
    }

    /// Notes that link to `note` with `[[Its title]]`, and notes that only mention its title.
    pub fn backlinks(&self, note: Id) -> (Vec<Id>, Vec<Id>) {
        let Some(title) = self.board.note(note).map(|n| n.title().to_lowercase()) else { return (Vec::new(), Vec::new()) };
        let mut linked = Vec::new();
        let mut mentioned = Vec::new();
        for n in self.board.notes.iter().filter(|n| n.id != note) {
            if crate::markdown::wiki_links(&n.text).iter().any(|l| l.to_lowercase() == title) {
                linked.push(n.id);
            } else if title.chars().count() >= 3 && n.text.to_lowercase().contains(&title) {
                mentioned.push(n.id);
            }
        }
        (linked, mentioned)
    }

    /// Delete several notes, and their cards.
    pub fn delete_notes(&mut self, ids: &[Id]) {
        let n = ids.len();
        let name = ids.first().and_then(|id| self.board.note(*id)).map(|n| n.title()).unwrap_or_default();
        for id in ids {
            self.delete_note(*id);
        }
        self.say(if n == 1 { format!("Deleted note \"{name}\". Ctrl+Z brings it back.") } else { format!("Deleted {}. Ctrl+Z brings them back.", plural(n, "note", "notes")) });
    }

    /// Put a card for a note on the board, centred at `at`, and select it.
    pub fn place_note(&mut self, note: Id, at: Pos2) -> Id {
        let id = self.board.add(Rect::from_center_size(at, vec2(300.0, 240.0)), Kind::Note { note }, Style::default());
        self.mark_changed();
        self.selection = vec![id];
        self.selected_wire = None;
        id
    }

    /// Delete a note and every card showing it.
    pub fn delete_note(&mut self, note: Id) {
        let cards: Vec<Id> = self.board.objects.iter().filter(|o| matches!(o.kind, Kind::Note { note: n } if n == note)).map(|o| o.id).collect();
        self.mark_changed();
        self.board.notes.retain(|n| n.id != note);
        self.board.remove(&cards);
        let b = &self.board;
        self.selection.retain(|id| b.get(*id).is_some());
        self.notes_sel.retain(|id| *id != note);
    }

    pub fn toggle_note_task(&mut self, note: Id, line: usize) {
        if let Some(n) = self.board.note_mut(note) {
            n.text = crate::markdown::toggle_task(&n.text, line);
            self.mark_changed();
        }
    }

    /// A picture for a note: `image:<id>` from the board's images, or a file path (relative ones
    /// are found next to the board's file).
    pub fn note_picture(&self, ctx: &egui::Context, src: &str) -> Option<(egui::TextureId, egui::Vec2)> {
        let mut cache = self.note_pictures.borrow_mut();
        let entry = cache.entry(src.to_owned()).or_insert_with(|| {
            let bytes = if let Some(id) = src.strip_prefix("image:").and_then(|s| s.trim().parse::<Id>().ok()) {
                self.board.images.get(&id).map(|a| a.bytes.to_vec())
            } else {
                let path = std::path::PathBuf::from(src.trim_start_matches("file:///"));
                let path = if path.is_relative() { self.path.as_ref().and_then(|p| p.parent()).map(|d| d.join(&path)).unwrap_or(path) } else { path };
                std::fs::read(path).ok()
            }?;
            let img = image::load_from_memory(&bytes).ok()?;
            let img = if img.width().max(img.height()) > 2048 { img.resize(2048, 2048, image::imageops::FilterType::Triangle) } else { img };
            let rgba = img.to_rgba8();
            let size = egui::vec2(rgba.width() as f32, rgba.height() as f32);
            let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
            Some((ctx.load_texture(format!("note-picture-{src}"), ci, egui::TextureOptions::LINEAR), size))
        });
        entry.as_ref().map(|(t, s)| (t.id(), *s))
    }

    /// Put a picture file into the board for a note; returns the text that shows it.
    pub fn embed_note_picture(&mut self, path: &Path) -> Option<String> {
        let bytes = std::fs::read(path).ok()?;
        let img = image::load_from_memory(&bytes).ok()?;
        let id = self.board.new_id();
        self.board.images.insert(id, ImageAsset { bytes: Arc::new(bytes), size: [img.width(), img.height()] });
        self.mark_changed();
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        Some(format!("![{name}](image:{id})"))
    }

    /// Is there a note or an item called `name` (what a `[[Name]]` link leads to)?
    pub fn link_known(&self, name: &str) -> bool {
        let key = name.trim().to_lowercase();
        self.board.notes.iter().any(|n| n.title().to_lowercase() == key) || self.board.objects.iter().any(|o| !o.label.trim().is_empty() && self.board.display_name(o.id).to_lowercase() == key)
    }

    /// A note's blocks, ready to draw: links to notes that do not exist yet are marked.
    pub fn note_blocks(&self, text: &str) -> Vec<crate::markdown::Block> {
        let mut blocks = crate::markdown::parse(text);
        crate::markdown::mark_missing(&mut blocks, &|n| self.link_known(n));
        blocks
    }

    /// Follow a link in a note (`from`, if it came from one): a web link opens in the browser;
    /// `[[Name]]` goes to the note or item with that name (a note's card on the board, or else the note
    /// in its tab). If there is none, the note is made, in the same folder, and opened to write.
    pub fn follow_link(&mut self, link: &crate::markdown::Link, from: Option<Id>) {
        match link {
            crate::markdown::Link::Url(u) => self.open_url = Some(u.clone()),
            crate::markdown::Link::Wiki(name) => {
                let key = name.trim().to_lowercase();
                let note = self.board.notes.iter().find(|n| n.title().to_lowercase() == key).map(|n| n.id);
                let card = note.and_then(|nid| self.board.objects.iter().find(|o| matches!(o.kind, Kind::Note { note: n } if n == nid)).map(|o| o.id));
                let item = card.or_else(|| self.board.objects.iter().find(|o| self.board.display_name(o.id).to_lowercase() == key).map(|o| o.id));
                if let Some(id) = item {
                    self.go_to(id);
                } else if let Some(nid) = note {
                    self.open_note = Some(nid);
                } else {
                    let folder = from.and_then(|f| self.board.note(f)).map(|n| n.folder).unwrap_or(self.notes_folder);
                    let id = self.add_note(&format!("# {}\n\n", name.trim()), true);
                    if let Some(n) = self.board.note_mut(id) {
                        n.folder = folder;
                    }
                    self.notes_sel = vec![id];
                    self.say(format!("Made the note \"{}\"", name.trim()));
                }
            }
        }
    }

    /// Hide the items on other pages, and with days chosen in the date roller, fade the items from
    /// other days and mark those from the chosen days.
    pub fn apply_day_filter(&mut self) {
        let page = self.page;
        // Off the board: other pages, and hidden layers.
        let hidden_layers: Vec<Id> = self.board.layers.iter().filter(|l| l.hidden).map(|l| l.id).collect();
        let veil: std::collections::HashSet<Id> = self.board.objects.iter().filter(|o| o.page != page || hidden_layers.contains(&o.layer)).map(|o| o.id).collect();
        if self.cal_days.is_empty() {
            self.day_shown.clear();
            self.day_direct.clear();
        } else {
            let (shown, direct) = crate::dates::matching(&self.board, &self.cal_days);
            self.day_shown = shown;
            self.day_direct = direct;
        }
        if !veil.is_empty() {
            self.selection.retain(|id| !veil.contains(id));
        }
        self.board.veil = veil;
    }

    // ── frames and presenting ──

    /// Present the page's frames full screen, starting at `from`.
    pub fn start_present(&mut self, from: usize) {
        let frames = self.board.frames(self.page);
        if frames.is_empty() {
            self.say("Draw a frame first: the Frame tool (F), or Add > Shapes > Frame.");
            return;
        }
        self.finish_edit();
        self.present_cam = Some(self.camera);
        self.presenting = Some(from.min(frames.len() - 1));
    }

    pub fn stop_present(&mut self) {
        self.presenting = None;
        if let Some(c) = self.present_cam.take() {
            self.camera = c;
        }
    }

    /// Fit the view to a frame and select it.
    pub fn show_frame(&mut self, id: Id) {
        if let Some(r) = self.board.get(id).map(|o| o.rect()) {
            self.camera.fit(self.canvas_rect, r.expand(r.width().max(r.height()) * 0.04));
            self.selection = vec![id];
            self.selected_wire = None;
        }
    }

    // ── pages ──

    /// Show another page, keeping each page's view.
    pub fn switch_page(&mut self, id: Id) {
        if id == self.page || !self.board.pages.iter().any(|p| p.id == id) {
            return;
        }
        self.finish_edit();
        self.page_cams.insert(self.page, self.camera);
        self.page = id;
        self.selection.clear();
        self.selected_wire = None;
        self.poly.clear();
        match self.page_cams.get(&id) {
            Some(c) => self.camera = *c,
            None => self.fit_pending = true,
        }
        // Hide the other pages at once, so this frame already shows the new page.
        self.apply_day_filter();
    }

    pub fn add_page(&mut self) {
        let id = self.board.new_id();
        let n = (1..).find(|n| !self.board.pages.iter().any(|p| p.name == format!("Page {n}"))).unwrap_or(1);
        let at = self.board.pages.iter().position(|p| p.id == self.page).map(|i| i + 1).unwrap_or(self.board.pages.len());
        self.board.pages.insert(at, crate::board::Page { id, name: format!("Page {n}") });
        self.mark_changed();
        self.switch_page(id);
        self.camera = Camera::centered(self.canvas_rect);
        self.fit_pending = false;
    }

    /// Delete a page and everything on it. There is always at least one page.
    pub fn delete_page(&mut self, id: Id) {
        if self.board.pages.len() < 2 {
            return;
        }
        let name = self.board.pages.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
        let on: Vec<Id> = self.board.objects.iter().filter(|o| o.page == id).map(|o| o.id).collect();
        if self.page == id {
            let other = self.board.pages.iter().find(|p| p.id != id).map(|p| p.id).unwrap_or(0);
            self.switch_page(other);
        }
        self.mark_changed();
        self.board.remove(&on);
        self.board.pages.retain(|p| p.id != id);
        self.say(format!("Deleted page \"{name}\". Ctrl+Z brings it back."));
    }

    /// Select an item and bring it into view, on whatever page it is.
    pub fn go_to(&mut self, id: Id) {
        let Some((page, c)) = self.board.get(id).map(|o| (o.page, o.rect().center())) else { return };
        self.switch_page(page);
        self.camera.look_at(self.canvas_rect, c);
        self.fit_pending = false;
        self.finish_edit();
        self.selection = vec![self.board.outer_sketch(id)];
        self.selected_wire = None;
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
            crate::dates::stamp(self.history.current(), &mut self.board, &crate::history::now());
            self.history.record(&self.board);
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

    /// Locked itself, or on a locked layer.
    pub fn is_locked(&self, id: Id) -> bool {
        self.board.get(id).is_some_and(|o| o.locked || self.board.layer_of(o).is_some_and(|l| l.locked))
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
            .filter_map(|o| if let Kind::Image { image, .. } = o.kind { self.board.images.get(&image).map(|a| (image, a.clone())) } else { None })
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
    /// Add copies of `clip`, moved by `offset`, and select them. Returns each original's copy.
    fn paste_clip(&mut self, clip: &Clip, offset: Vec2) -> HashMap<Id, Id> {
        if clip.objects.is_empty() {
            return HashMap::new();
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
            if let Kind::Image { image, .. } = &mut n.kind {
                if let Some(x) = img_map.get(image) {
                    *image = *x;
                }
            }
            // Pasted items land on the current layer and page, and are new today.
            n.layer = self.active_layer;
            n.page = self.page;
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
                        nw.color = w.color;
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
        map
    }

    pub fn duplicate(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let clip = self.clip_from_selection();
        self.paste_clip(&clip, vec2(24.0, 24.0));
    }

    /// Copies of the selection in the same place, selected (Ctrl+drag moves them). Returns each
    /// selected item with its copy. Copies are never locked, so they can be dragged.
    pub fn copy_in_place(&mut self) -> Vec<(Id, Id)> {
        let originals = self.selection.clone();
        let clip = self.clip_from_selection();
        let map = self.paste_clip(&clip, Vec2::ZERO);
        let pairs: Vec<(Id, Id)> = originals.iter().filter_map(|o| map.get(o).map(|c| (*o, *c))).collect();
        for (_, c) in &pairs {
            if let Some(o) = self.board.get_mut(*c) {
                o.locked = false;
            }
        }
        self.say(format!("Copying {}: drop it where it goes", plural(pairs.len(), "item", "items")));
        pairs
    }

    /// Paste from the system clipboard: our own objects, an image (for example a screenshot) or text.
    pub fn paste(&mut self) {
        let at = self.paste_point();
        let mut cb = match arboard::Clipboard::new() {
            Ok(cb) => cb,
            Err(_) => {
                let clip = self.clip.clone();
                self.paste_clip(&clip, vec2(24.0, 24.0));
                return;
            }
        };
        let text = cb.get_text().ok();
        if text.as_deref() == Some(CLIPBOARD_MARKER) {
            let clip = self.clip.clone();
            if let Some(b) = clip.objects.iter().map(|o| o.rect()).reduce(|a, b| a.union(b)) {
                self.paste_clip(&clip, at - b.center());
                return;
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
        if let Some(t) = text.as_ref().filter(|t| crate::markdown::looks_like_markdown(t)) {
            let note = self.add_note(t, false);
            self.place_note(note, at);
            self.say("Pasted as a note");
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
        let id = self.board.add(Rect::from_center_size(at, size), Kind::Image { image, crop: None }, Style::default());
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

    /// A box is being drawn (the arrow keys then shape its grid instead of nudging).
    pub fn drawing_box(&self) -> bool {
        matches!(self.drag, Drag::Box { .. })
    }

    /// Fit the view to the selected items (or the selected connection's ends).
    pub fn zoom_to_selection(&mut self) {
        let mut ids = self.selection.clone();
        if let Some(w) = self.selected_wire.and_then(|w| self.board.wire(w)) {
            ids.extend([w.from, w.to]);
        }
        match ids.iter().filter_map(|id| self.board.get(*id)).map(|o| o.rect()).reduce(|a, b| a.union(b)) {
            Some(b) => self.camera.fit(self.canvas_rect, b.expand(b.width().max(b.height()) * 0.08 + 24.0)),
            None => self.say("Select something to zoom to"),
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
        self.load_text(&text, Some(path))
    }

    /// The demo board that ships inside the app (opened untitled, so saving asks where).
    pub fn load_demo(&mut self) {
        if let Err(e) = self.load_text(include_str!("../examples/demo.overlay"), None) {
            self.say(format!("Could not open the demo: {e}"));
        }
    }

    fn load_text(&mut self, text: &str, path: Option<&Path>) -> Result<(), String> {
        let f: FileV1 = serde_json::from_str(text).map_err(|e| format!("not an Overlay board ({e})"))?;
        if f.format != FILE_FORMAT {
            return Err("not an Overlay board".into());
        }
        *self = self.fresh();
        self.board = f.board;
        self.board.fix_after_load();
        // Files from older versions kept images their history used; the board only needs its own.
        self.board.gc_images();
        self.history = Undo::new(&self.board, self.undo_steps);
        self.camera = f.camera;
        self.path = path.map(Path::to_owned);
        let n = self.board.objects.len();
        self.say(format!("Opened {}, {}", plural(n, "object", "objects"), plural(self.board.wires.len(), "connection", "connections")));
        Ok(())
    }

    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        self.finish_edit();
        self.flush();
        // Written straight from the board, without copying it first.
        #[derive(serde::Serialize)]
        struct Out<'a> {
            format: &'a str,
            version: u32,
            board: &'a Board,
            camera: Camera,
        }
        let f = Out { format: FILE_FORMAT, version: 3, board: &self.board, camera: self.camera };
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
