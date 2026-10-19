//! The window: top bar, dock area, shortcuts, file drops and settings.

use crate::board::{Shape, Style, WireStyle};
use crate::editor::{Editor, plural};
use crate::export::{self, ExportSettings};
use crate::{overview, panels};
use crate::theme::{self, Appearance, UiExt, pal};
use crate::tools::Tool;
use egui::{Key, KeyboardShortcut, Margin, Modifiers, Ui, ViewportCommand, vec2};
use egui_dock::{DockArea, DockState, NodeIndex, TabStyle, TabViewer};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Tab {
    Board,
    /// Older layouts name these; the tools now live in the floating toolbox.
    Tools,
    Draw,
    Shapes,
    Flowchart,
    Calendar,
    Properties,
    Layers,
    Graph,
    History,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Settings {
    appearance: Appearance,
    /// A saved layout is only reused if it was made by the current default layout's version.
    dock_version: u32,
    dock: Option<DockState<Tab>>,
    last_file: Option<PathBuf>,
    style: Option<Style>,
    node_shape: Option<Shape>,
    wire_style: Option<WireStyle>,
    brush_widths: Option<[f32; 4]>,
    show_overview: Option<bool>,
    snapping: Option<bool>,
    poly_curved: Option<bool>,
}

const SETTINGS_KEY: &str = "overlay-settings";
const DOCK_VERSION: u32 = 4;

fn default_dock() -> DockState<Tab> {
    let mut d = DockState::new(vec![Tab::Board]);
    let s = d.main_surface_mut();
    // The board keeps most of the window (the tools float over it), the Calendar sits under it, and
    // the other panels are on the right.
    let [board, _panels] = s.split_right(NodeIndex::root(), 0.8, vec![Tab::Properties, Tab::Layers, Tab::Graph, Tab::History]);
    s.split_below(board, 0.74, vec![Tab::Calendar]);
    d
}

/// Something that closes or replaces the board, waiting on the "Save changes?" dialog.
#[derive(Clone, Debug)]
enum Pending {
    Close,
    New,
    Open,
    OpenFile(PathBuf),
}

enum SaveChoice {
    Save,
    Discard,
    Cancel,
}

pub struct App {
    ed: Editor,
    dock: DockState<Tab>,
    appearance: Appearance,
    title: String,
    v_was_down: bool,
    closing: bool,
    show_overview: bool,
    export: ExportSettings,
    /// Set while the "Save changes?" dialog is showing.
    ask_save: Option<Pending>,
    /// Frame timing, only when OVERLAY_PERF names a file to write it to.
    perf: Option<Perf>,
}

/// Measures how long Overlay's own work takes per frame. Off unless OVERLAY_PERF=<file> is set;
/// then the window redraws continuously and a line of timings is appended every two seconds.
struct Perf {
    path: PathBuf,
    since: std::time::Instant,
    last: std::time::Instant,
    frames: u32,
    work: f64,
    worst: f64,
    gaps: f64,
}

impl Perf {
    fn from_env() -> Option<Perf> {
        let path = PathBuf::from(std::env::var_os("OVERLAY_PERF")?);
        let now = std::time::Instant::now();
        Some(Perf { path, since: now, last: now, frames: 0, work: 0.0, worst: 0.0, gaps: 0.0 })
    }

    fn frame(&mut self, started: std::time::Instant, objects: usize, wires: usize) {
        let now = std::time::Instant::now();
        let work = (now - started).as_secs_f64() * 1000.0;
        self.gaps += (started - self.last).as_secs_f64() * 1000.0;
        self.last = started;
        self.frames += 1;
        self.work += work;
        self.worst = self.worst.max(work);
        if (now - self.since).as_secs_f64() >= 2.0 {
            let n = self.frames.max(1) as f64;
            let line = format!(
                "objects {objects} wires {wires}  frames {}  ui avg {:.2} ms  ui worst {:.2} ms  frame interval {:.2} ms\n",
                self.frames,
                self.work / n,
                self.worst,
                self.gaps / n
            );
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.path) {
                let _ = f.write_all(line.as_bytes());
            }
            *self = Perf { path: std::mem::take(&mut self.path), since: now, last: now, frames: 0, work: 0.0, worst: 0.0, gaps: 0.0 };
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings: Settings = cc.storage.and_then(|s| eframe::get_value(s, SETTINGS_KEY)).unwrap_or_default();
        theme::apply(&cc.egui_ctx, settings.appearance);
        let mut ed = Editor::default();
        if let Some(s) = settings.style {
            ed.style = s;
        }
        if let Some(s) = settings.node_shape {
            ed.node_shape = s;
        }
        if let Some(s) = settings.wire_style {
            ed.wire_style = s;
        }
        if let Some(w) = settings.brush_widths {
            ed.brush_widths = w;
        }
        if let Some(c) = settings.poly_curved {
            ed.poly_curved = c;
        }
        if let Some(s) = settings.snapping {
            ed.snapping = s;
        }
        // A file given on the command line wins over the last one used.
        let arg = std::env::args().nth(1).map(PathBuf::from);
        let reopened = arg.or(settings.last_file).filter(|p| p.exists()).map(|p| ed.load(&p).is_ok()).unwrap_or(false);
        if !reopened {
            ed.fit_pending = true;
        }
        let dock = settings.dock.filter(|_| settings.dock_version == DOCK_VERSION).unwrap_or_else(default_dock);
        Self {
            ed,
            dock,
            appearance: settings.appearance,
            title: String::new(),
            v_was_down: false,
            closing: false,
            show_overview: settings.show_overview.unwrap_or(true),
            export: ExportSettings::default(),
            ask_save: None,
            perf: Perf::from_env(),
        }
    }

    /// Close or replace the board, first asking to save if there are unsaved changes.
    fn request(&mut self, ctx: &egui::Context, action: Pending) {
        if self.ed.needs_save() {
            self.ask_save = Some(action);
        } else {
            self.perform(ctx, action);
        }
    }

    fn perform(&mut self, ctx: &egui::Context, action: Pending) {
        match action {
            Pending::Close => {
                self.closing = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Pending::New => self.ed.new_board(),
            Pending::Open => self.ed.open_dialog(),
            Pending::OpenFile(p) => {
                if let Err(e) = self.ed.load(&p) {
                    self.ed.say(format!("Could not open {}: {e}", p.display()));
                }
            }
        }
    }

    /// "Save changes?" in the app's own style, over a dimmed window.
    fn save_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.ask_save.clone() else { return };
        let p = pal(ctx);
        let frame = egui::Frame::new()
            .fill(p.window_fill)
            .stroke(egui::Stroke::new(1.0, p.window_stroke))
            .corner_radius(12)
            .inner_margin(Margin::same(18))
            .shadow(ctx.global_style().visuals.window_shadow);
        let backdrop = egui::Color32::from_black_alpha(if ctx.theme() == egui::Theme::Dark { 120 } else { 60 });
        let name = self.ed.file_name();
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("save-changes")).frame(frame).backdrop_color(backdrop).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.label(theme::semibold(format!("Save changes to {name}?")).size(15.0));
            ui.add_space(2.0);
            let why = match action {
                Pending::Close => "If you don't save, changes since the last save are lost when Overlay closes.",
                _ => "If you don't save, changes since the last save are lost.",
            };
            ui.label(theme::secondary(ui, why));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new("Don't save").color(p.danger)).clicked() {
                    choice = Some(SaveChoice::Discard);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // The default action takes the accent, like a selected control.
                    let save = egui::Button::new(theme::semibold("Save").color(egui::Color32::WHITE)).fill(p.accent).min_size(vec2(76.0, 0.0));
                    if ui.add(save).clicked() {
                        choice = Some(SaveChoice::Save);
                    }
                    if ui.add(egui::Button::new("Cancel").min_size(vec2(76.0, 0.0))).clicked() {
                        choice = Some(SaveChoice::Cancel);
                    }
                });
            });
        });
        if choice.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            choice = Some(SaveChoice::Save);
        }
        // Escape or a click outside the dialog cancels.
        if choice.is_none() && modal.should_close() {
            choice = Some(SaveChoice::Cancel);
        }
        match choice {
            Some(SaveChoice::Save) => {
                self.ask_save = None;
                // Saving an untitled board asks for a file; cancelling that cancels the whole action.
                if self.ed.save(false) {
                    self.perform(ctx, action);
                }
            }
            Some(SaveChoice::Discard) => {
                self.ask_save = None;
                self.perform(ctx, action);
            }
            Some(SaveChoice::Cancel) => self.ask_save = None,
            None => {}
        }
    }

    fn shortcuts(&mut self, ui: &Ui) {
        let ctx = ui.ctx().clone();
        let typing = ctx.egui_wants_keyboard_input();
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_shift = |k| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, k);
        let ev = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));

        // While the save dialog is up, it has the keyboard.
        if self.ask_save.is_some() {
            return;
        }
        // File shortcuts work even while typing.
        if ev(cmd(Key::O)) {
            self.request(&ctx, Pending::Open);
        }
        if ev(cmd(Key::N)) {
            self.request(&ctx, Pending::New);
        }
        let ed = &mut self.ed;
        if ev(cmd_shift(Key::S)) {
            ed.save(true);
        } else if ev(cmd(Key::S)) {
            ed.save(false);
        }
        if ev(cmd(Key::E)) {
            self.export.open = true;
        }
        // While a label is being typed, keys belong to it. (egui drops the text box's focus on Esc
        // before this runs, so check the edit itself too: Esc should only end the typing.)
        if typing || ed.is_editing() {
            return;
        }
        // A polyline being drawn takes Enter, Esc and Backspace.
        ed.poly_keys(&ctx);
        if ev(cmd_shift(Key::Z)) || ev(cmd(Key::Y)) {
            ed.redo();
        } else if ev(cmd(Key::Z)) {
            ed.undo();
        }
        if ev(cmd(Key::A)) {
            ed.select_all();
        }
        if ev(cmd(Key::L)) {
            ed.toggle_lock();
        }
        if ev(cmd_shift(Key::G)) {
            ed.ungroup_selection();
        } else if ev(cmd(Key::G)) {
            ed.group_selection();
        }
        if ev(cmd(Key::D)) {
            ed.duplicate();
        }
        if ev(cmd(Key::I)) {
            ed.pick_images();
        }
        if ev(cmd(Key::CloseBracket)) {
            ed.reorder_selection(true);
        }
        if ev(cmd(Key::OpenBracket)) {
            ed.reorder_selection(false);
        }
        if ev(cmd(Key::Num0)) {
            ed.reset_zoom();
        }
        if ev(KeyboardShortcut::new(Modifiers::SHIFT, Key::Num1)) {
            ed.zoom_to_fit();
        }

        let (events, mods) = ctx.input(|i| (i.events.clone(), i.modifiers));
        let mut pasted = false;
        for e in &events {
            match e {
                egui::Event::Copy => ed.copy(),
                egui::Event::Cut => ed.cut(),
                egui::Event::Paste(_) => pasted = true,
                _ => {}
            }
        }
        // egui only reports Ctrl+V when the clipboard holds text, so a screenshot would be missed.
        // Watch the key itself as well.
        let v_down = mods.command && ctrl_v_down();
        if (v_down && !self.v_was_down) || (pasted && !self.v_was_down) {
            ed.paste();
        }
        self.v_was_down = v_down || (pasted && mods.command);

        // "/" opens the list of items to add (connected to the selected node, if there is one).
        if !mods.command && ed.insert.is_none() && ctx.input(|i| i.key_pressed(Key::Slash)) {
            ed.open_insert_here();
            return;
        }
        if mods.is_none() {
            if ctx.input(|i| i.key_pressed(Key::M)) {
                self.show_overview = !self.show_overview;
            }
            ctx.input(|i| {
                for t in Tool::ALL {
                    if i.key_pressed(t.key()) {
                        ed.finish_edit();
                        ed.tool = t;
                    }
                }
            });
        }
        ctx.input(|i| {
            if i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace) {
                ed.delete_selection();
            }
            if i.key_pressed(Key::Escape) && ed.eyedropper.take().is_some() {
                // Escape only cancels the eyedropper.
            } else if i.key_pressed(Key::Escape) {
                if ed.selection.is_empty() && ed.selected_wire.is_none() {
                    ed.tool = Tool::Select;
                }
                ed.clear_selection();
            }
            if i.key_pressed(Key::Enter) && ed.selection.len() == 1 {
                ed.start_edit(ed.selection[0]);
            }
            let step = if i.modifiers.shift { 10.0 } else { 1.0 };
            for (k, d) in [(Key::ArrowLeft, vec2(-step, 0.0)), (Key::ArrowRight, vec2(step, 0.0)), (Key::ArrowUp, vec2(0.0, -step)), (Key::ArrowDown, vec2(0.0, step))] {
                if i.key_pressed(k) {
                    ed.nudge(d);
                }
            }
        });
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let p = pal(ui.ctx());
        // As in Grapht: the menus start the row; the window's own title bar carries the icon and name.
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New board").clicked() {
                    self.request(ui.ctx(), Pending::New);
                }
                if ui.button("Open…").clicked() {
                    self.request(ui.ctx(), Pending::Open);
                }
                ui.separator();
                if ui.button("Save").clicked() {
                    self.ed.save(false);
                }
                if ui.button("Save as…").clicked() {
                    self.ed.save(true);
                }
                ui.separator();
                if ui.button("Insert image…").clicked() {
                    self.ed.pick_images();
                }
                if ui.button("Export image…  Ctrl+E").clicked() {
                    self.export.open = true;
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(self.ed.can_undo(), egui::Button::new("Undo")).clicked() {
                    self.ed.undo();
                }
                if ui.add_enabled(self.ed.can_redo(), egui::Button::new("Redo")).clicked() {
                    self.ed.redo();
                }
                ui.separator();
                if ui.button("Copy").clicked() {
                    self.ed.copy();
                }
                if ui.button("Paste").clicked() {
                    self.ed.paste();
                }
                if ui.button("Duplicate").clicked() {
                    self.ed.duplicate();
                }
                if ui.button("Delete").clicked() {
                    self.ed.delete_selection();
                }
                ui.separator();
                if ui.button("Group").clicked() {
                    self.ed.group_selection();
                }
                if ui.add_enabled(self.ed.any_group_selected(), egui::Button::new("Ungroup")).clicked() {
                    self.ed.ungroup_selection();
                }
                if ui.add_enabled(!self.ed.selection.is_empty(), egui::Button::new("Lock or unlock  Ctrl+L")).clicked() {
                    self.ed.toggle_lock();
                }
                ui.separator();
                if ui.button("Select all").clicked() {
                    self.ed.select_all();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Zoom to fit").clicked() {
                    self.ed.zoom_to_fit();
                }
                if ui.button("Actual size").clicked() {
                    self.ed.reset_zoom();
                }
                ui.separator();
                ui.switch(&mut self.show_overview, "Overview (M)");
                ui.switch(&mut self.ed.snapping, "Snap to items").on_hover_text("Lines items up with each other's edges, centres and gaps while you move or resize them. Hold Alt to skip it for one drag.");
                ui.separator();
                if ui.button("Reset panel layout").clicked() {
                    self.dock = default_dock();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                ui.scope(|ui| {
                    ui.set_width(190.0);
                    let mut a = self.appearance;
                    if ui.segmented(&mut a, &[(Appearance::System, "System"), (Appearance::Dark, "Dark"), (Appearance::Light, "Light")]) {
                        self.appearance = a;
                        theme::set_appearance(ui.ctx(), a);
                    }
                });
                ui.add_space(8.0);
                let zoom = format!("{:.0}%", self.ed.camera.zoom * 100.0);
                if ui.add(egui::Button::new(theme::secondary(ui, zoom)).frame(false)).on_hover_text("Zoom to fit (Shift+1)").clicked() {
                    self.ed.zoom_to_fit();
                }
                let b = &self.ed.board;
                let mut info = format!("{}, {}", plural(b.objects.len(), "object", "objects"), plural(b.wires.len(), "connection", "connections"));
                if let Some((msg, t)) = &self.ed.status {
                    if t.elapsed().as_secs_f32() < 6.0 {
                        info = msg.clone();
                        ui.ctx().request_repaint_after(std::time::Duration::from_secs(6));
                    }
                }
                ui.label(egui::RichText::new(info).color(p.text_secondary));
            });
        });
    }

    fn update_title(&mut self, ui: &Ui) {
        // Worded like Grapht's: the app's name alone, or the board's name and the app's. A dot marks
        // unsaved changes.
        let dot = if self.ed.dirty { "• " } else { "" };
        let t = match self.ed.path.as_ref().and_then(|p| p.file_stem()) {
            Some(stem) => format!("{dot}{} — Overlay", stem.to_string_lossy()),
            None if self.ed.dirty => format!("{dot}Untitled — Overlay"),
            None => "Overlay".to_owned(),
        };
        if t != self.title {
            ui.ctx().send_viewport_cmd(ViewportCommand::Title(t.clone()));
            self.title = t;
        }
    }
}

struct Viewer<'a> {
    ed: &'a mut Editor,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        match tab {
            Tab::Board => "Board",
            Tab::Tools | Tab::Draw | Tab::Shapes | Tab::Flowchart => "Tools",
            Tab::Calendar => "Calendar",
            Tab::Properties => "Properties",
            Tab::Layers => "Layers",
            Tab::Graph => "Graph",
            Tab::History => "History",
        }
        .into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        match tab {
            Tab::Board => self.ed.canvas_ui(ui),
            Tab::Tools | Tab::Draw | Tab::Shapes | Tab::Flowchart => {
                ui.label(theme::secondary(ui, "The tools are in the floating Tools window over the board. View > Reset panel layout removes this tab."));
            }
            Tab::Calendar => crate::calendar::calendar_ui(ui, self.ed),
            Tab::Properties => panels::properties_ui(ui, self.ed),
            Tab::Layers => panels::layers_ui(ui, self.ed),
            Tab::Graph => panels::graph_ui(ui, self.ed),
            Tab::History => panels::history_ui(ui, self.ed),
        }
    }

    fn is_closeable(&self, _tab: &Tab) -> bool {
        false
    }

    fn scroll_bars(&self, _tab: &Tab) -> [bool; 2] {
        [false, false]
    }

    fn tab_style_override(&self, tab: &Tab, global: &TabStyle) -> Option<TabStyle> {
        match tab {
            Tab::Board => {
                let mut s = global.clone();
                s.tab_body.inner_margin = Margin::ZERO;
                Some(s)
            }
            Tab::Tools | Tab::Draw | Tab::Shapes | Tab::Flowchart => {
                let mut s = global.clone();
                s.tab_body.inner_margin = Margin::symmetric(8, 8);
                Some(s)
            }
            _ => None,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let started = std::time::Instant::now();
        self.frame(ui);
        if let Some(perf) = &mut self.perf {
            perf.frame(started, self.ed.board.objects.len(), self.ed.board.wires.len());
            if std::env::var_os("OVERLAY_PERF_PASSIVE").is_some() {
                // Log what asked for this redraw, to find redraws that should not happen.
                let causes = format!("{:?}\n", ui.ctx().repaint_causes());
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(perf.path.with_extension("causes.txt")) {
                    let _ = f.write_all(causes.as_bytes());
                }
            } else {
                ui.ctx().request_repaint();
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.save_settings(storage);
    }
}

impl App {
    fn frame(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();

        // Closing with unsaved changes asks first.
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing {
            if self.ed.needs_save() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.ask_save = Some(Pending::Close);
            } else {
                self.closing = true;
            }
        }

        self.shortcuts(ui);

        // Image files dropped from Explorer land where they were dropped.
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !dropped.is_empty() {
            let at = ctx.input(|i| i.pointer.hover_pos()).filter(|p| self.ed.canvas_rect.contains(*p)).map(|p| self.ed.camera.to_world(self.ed.canvas_rect.min, p)).unwrap_or_else(|| self.ed.paste_point());
            let (boards, images): (Vec<PathBuf>, Vec<PathBuf>) = dropped.into_iter().partition(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("overlay")));
            if let Some(b) = boards.first() {
                self.request(&ctx, Pending::OpenFile(b.clone()));
            }
            if !images.is_empty() {
                self.ed.import_image_files(&images, at);
            }
        }

        self.update_title(ui);

        let p = pal(&ctx);
        egui::Panel::top("top-bar").frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::symmetric(8, 4))).show(ui, |ui| self.top_bar(ui));
        // New items go on the current layer; the item list stays in layer order.
        self.ed.board.fix_layers(&mut self.ed.active_layer);
        // The Calendar's chosen days hide or fade the other items.
        self.ed.apply_day_filter();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg)).show(ui, |ui| {
            let style = theme::dock_style(ui.ctx());
            DockArea::new(&mut self.dock)
                .style(style)
                .show_close_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_leaf_collapse_buttons(false)
                .show_inside(ui, &mut Viewer { ed: &mut self.ed });
        });

        panels::toolbox(&ctx, &mut self.ed, &mut self.show_overview);
        overview::show(&ctx, &mut self.ed, &mut self.show_overview);
        crate::insert::show(&ctx, &mut self.ed);
        panels::comments_window(&ctx, &mut self.ed);
        export::window(&ctx, &mut self.ed, &mut self.export);
        self.save_dialog(&ctx);

        // Turn settled changes into states.
        self.ed.tick(&ctx);

        // Highlight the window while files are dragged over it.
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop")));
            let r = self.ed.canvas_rect.shrink(8.0);
            painter.rect_stroke(r, 10.0, egui::Stroke::new(2.0, p.accent), egui::StrokeKind::Inside);
            painter.text(r.center(), egui::Align2::CENTER_CENTER, "Drop to add to the board", egui::FontId::proportional(14.0), p.accent_text);
        }
    }

    fn save_settings(&mut self, storage: &mut dyn eframe::Storage) {
        let s = Settings {
            appearance: self.appearance,
            dock_version: DOCK_VERSION,
            dock: Some(self.dock.clone()),
            last_file: self.ed.path.clone(),
            style: Some(self.ed.style),
            node_shape: Some(self.ed.node_shape),
            wire_style: Some(self.ed.wire_style),
            brush_widths: Some(self.ed.brush_widths),
            show_overview: Some(self.show_overview),
            snapping: Some(self.ed.snapping),
            poly_curved: Some(self.ed.poly_curved),
        };
        eframe::set_value(storage, SETTINGS_KEY, &s);
    }
}

/// Is the V key physically down? Used to catch Ctrl+V for image-only clipboards.
#[cfg(windows)]
fn ctrl_v_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { (GetAsyncKeyState(b'V' as i32) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
fn ctrl_v_down() -> bool {
    false
}
