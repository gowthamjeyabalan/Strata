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
    Notes,
    Frames,
    /// A note open to read and write, beside the board.
    Note(crate::board::Id),
    Properties,
    Layers,
    Graph,
    /// Older layouts name it; History was removed (undo stays).
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
    show_note_links: Option<bool>,
    show_grid: Option<bool>,
    radial_empty: Option<bool>,
    radial_hold_ms: Option<u32>,
    undo_steps: Option<usize>,
    note_mode: Option<u8>,
    ui_scale: Option<f32>,
    reopen_last: Option<bool>,
    keys: Option<crate::keys::Keymap>,
    start_view: Option<bool>,
    custom_colors: Option<Vec<[u8; 3]>>,
    recent: Option<Vec<PathBuf>>,
    workspaces: Option<Vec<(String, DockState<Tab>)>>,
}

const SETTINGS_KEY: &str = "overlay-settings";
const DOCK_VERSION: u32 = 10;

fn default_dock() -> DockState<Tab> {
    let mut d = DockState::new(vec![Tab::Board]);
    let s = d.main_surface_mut();
    // The board keeps most of the window (the tools float over it), the Calendar sits under it, and
    // the other panels are on the right.
    // Notes are listed on the left, like files in a notebook app; opened notes are tabs beside the board.
    let [board, notes] = s.split_left(NodeIndex::root(), 0.17, vec![Tab::Notes]);
    // The calendar under the notes, like the Windows calendar flyout.
    s.split_below(notes, 0.58, vec![Tab::Calendar]);
    s.split_right(board, 0.78, vec![Tab::Properties, Tab::Layers, Tab::Frames]);
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
    export: ExportSettings,
    /// Set while the "Save changes?" dialog is showing.
    ask_save: Option<Pending>,
    /// Presenting last frame, to switch full screen on and off once.
    was_presenting: bool,
    settings_win: crate::settings::SettingsWindow,
    /// Interface size (egui's zoom factor).
    ui_scale: f32,
    /// Show the start view when the app opens (otherwise reopen the last board).
    start_view: bool,
    /// The start view is showing.
    start_open: bool,
    /// Boards opened or saved lately, newest first.
    recent: Vec<PathBuf>,
    /// Panel arrangements saved from the Window menu.
    workspaces: Vec<(String, DockState<Tab>)>,
    /// The name being typed for a new workspace.
    ws_name: String,
    logo: Option<egui::TextureHandle>,
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
        ed.show_overview = settings.show_overview.unwrap_or(true);
        ed.show_note_links = settings.show_note_links.unwrap_or(true);
        ed.show_grid = settings.show_grid.unwrap_or(true);
        ed.radial_empty = settings.radial_empty.unwrap_or(true);
        ed.radial_hold_ms = settings.radial_hold_ms.unwrap_or(300).clamp(150, 1000);
        ed.undo_steps = settings.undo_steps.unwrap_or(crate::history::STEPS).clamp(20, 500);
        ed.history.set_limit(ed.undo_steps);
        ed.note_mode = settings.note_mode.unwrap_or(1).min(2);
        ed.keys = settings.keys.clone().unwrap_or_default();
        ed.custom_colors = settings.custom_colors.clone().unwrap_or_default();
        let ui_scale = settings.ui_scale.unwrap_or(1.0).clamp(0.75, 1.5);
        cc.egui_ctx.set_zoom_factor(ui_scale);
        let start_view = settings.start_view.unwrap_or(true);
        // A file given on the command line (opened from Explorer) goes straight to the board.
        // Otherwise the start view shows, or with it switched off, the last board opens.
        let arg = std::env::args().nth(1).map(PathBuf::from);
        let from_explorer = arg.is_some();
        let reopened = arg.or(settings.last_file.filter(|_| !start_view)).filter(|p| p.exists()).map(|p| ed.load(&p).is_ok()).unwrap_or(false);
        let start_open = start_view && !from_explorer;
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
            export: ExportSettings::default(),
            ask_save: None,
            was_presenting: false,
            settings_win: Default::default(),
            ui_scale,
            start_view,
            start_open,
            recent: settings.recent.clone().unwrap_or_default(),
            workspaces: settings.workspaces.clone().unwrap_or_default(),
            ws_name: String::new(),
            logo: None,
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

        // While the save dialog is up, it has the keyboard.
        if self.ask_save.is_some() {
            return;
        }
        // While a new shortcut is being set in Settings, keys belong to it.
        if self.settings_win.capturing() {
            return;
        }
        // Shortcuts from the key map (Settings > Keyboard shortcuts). While a label is being typed,
        // only file commands with Ctrl (and search) work; the rest of the keys belong to the text.
        // (egui drops the text box's focus on Esc before this runs, so check the edit itself too.)
        let busy = typing || self.ed.is_editing();
        let presses: Vec<(Key, Modifiers)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        for (key, m) in presses {
            let Some(a) = self.ed.keys.action_for(key, m) else { continue };
            if busy && !(a.while_typing() && (m.command || m.ctrl)) {
                continue;
            }
            ctx.input_mut(|i| i.consume_key(m, key));
            self.run_action(&ctx, a);
        }
        // Ctrl+Shift+Z also redoes, whatever the Redo shortcut is.
        if !busy && ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))) {
            self.ed.redo();
        }
        let ed = &mut self.ed;
        if busy {
            return;
        }
        // A polyline being drawn takes Enter, Esc and Backspace.
        ed.poly_keys(&ctx);
        // Cropping takes Enter and Esc.
        ed.crop_keys(&ctx);

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

        ctx.input(|i| {
            if i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace) {
                // The Notes list, if it was clicked last; otherwise the board.
                if ed.notes_focus && !ed.notes_sel.is_empty() {
                    let ids = ed.notes_sel.clone();
                    ed.delete_notes(&ids);
                } else {
                    ed.delete_selection();
                }
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
                // While a box is being drawn, the arrows make a grid of it instead.
                if i.key_pressed(k) && !ed.drawing_box() {
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
                if ui.button("Export image…").clicked() {
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
                if ui.add_enabled(!self.ed.selection.is_empty(), egui::Button::new("Lock or unlock")).clicked() {
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
                if ui.add_enabled(!self.ed.selection.is_empty() || self.ed.selected_wire.is_some(), egui::Button::new("Zoom to the selection")).clicked() {
                    self.ed.zoom_to_selection();
                }
                if ui.button("Actual size").clicked() {
                    self.ed.reset_zoom();
                }
                ui.separator();
                ui.switch(&mut self.ed.show_overview, "Overview (M)");
                ui.switch(&mut self.ed.show_note_links, "Links between note cards").on_hover_text("Dotted lines join the cards of notes that link to each other with [[Name]].");
                ui.switch(&mut self.ed.snapping, "Snap to items").on_hover_text("Lines items up with each other's edges, centres and gaps while you move or resize them. Hold Alt to skip it for one drag.");
            });
            ui.menu_button("Window", |ui| self.window_menu(ui));
            // Settings has its own place in the bar.
            if ui.button("Settings").clicked() {
                self.settings_win.open = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let z = self.ed.camera.zoom * 100.0;
                let zoom = if z < 1.0 { format!("{z:.2}%") } else if z < 10.0 { format!("{z:.1}%") } else { format!("{z:.0}%") };
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

    /// Panels on and off, and saved arrangements of them.
    fn window_menu(&mut self, ui: &mut Ui) {
        ui.label(theme::secondary(ui, "Panels"));
        for (tab, name) in [(Tab::Notes, "Notes"), (Tab::Calendar, "Calendar"), (Tab::Properties, "Properties"), (Tab::Layers, "Layers"), (Tab::Frames, "Frames")] {
            let mut on = self.dock.find_tab(&tab).is_some();
            if ui.checkbox(&mut on, name).clicked() {
                self.toggle_tab(tab);
            }
        }
        ui.checkbox(&mut self.ed.show_overview, "Overview (M)");
        ui.separator();
        ui.label(theme::secondary(ui, "Workspaces"));
        let mut load = None;
        let mut remove = None;
        for (k, (name, _)) in self.workspaces.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.button(name).on_hover_text("Arrange the panels this way").clicked() {
                    load = Some(k);
                }
                if ui.add(egui::Button::new("×").frame(false)).on_hover_text("Forget this workspace").clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = load {
            self.dock = self.workspaces[k].1.clone();
            ui.close();
        }
        if let Some(k) = remove {
            self.workspaces.remove(k);
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.ws_name).hint_text("Name").desired_width(120.0));
            if ui.button("Save current").on_hover_text("Keep the panels as they are now, under this name").clicked() {
                let name = if self.ws_name.trim().is_empty() { format!("Workspace {}", self.workspaces.len() + 1) } else { self.ws_name.trim().to_owned() };
                // Note tabs belong to a board, so they are not kept in a workspace.
                let mut dock = self.dock.clone();
                dock.retain_tabs(|t| !matches!(t, Tab::Note(_)));
                self.workspaces.retain(|(n, _)| *n != name);
                self.workspaces.push((name.clone(), dock));
                self.ws_name.clear();
                self.ed.say(format!("Saved the workspace \"{name}\""));
            }
        });
        ui.separator();
        if ui.button("Default layout").clicked() {
            self.dock = default_dock();
            ui.close();
        }
    }

    /// Show a panel, or hide it if it is showing. A shown panel goes beside its usual neighbours.
    fn toggle_tab(&mut self, tab: Tab) {
        if let Some(path) = self.dock.find_tab(&tab) {
            self.dock.remove_tab(path);
            return;
        }
        let neighbour = match tab {
            Tab::Notes => None,
            Tab::Calendar => self.dock.find_tab(&Tab::Notes),
            _ => [Tab::Properties, Tab::Layers, Tab::Frames].iter().find_map(|t| self.dock.find_tab(t)),
        };
        if let Some(path) = neighbour {
            if let Ok(leaf) = self.dock.leaf_mut(path.node_path()) {
                leaf.append_tab(tab);
                let last = leaf.tabs.len() - 1;
                let _ = leaf.set_active_tab(last);
            }
            return;
        }
        match self.dock.find_tab(&Tab::Board) {
            Some(path) if path.surface == egui_dock::SurfaceIndex::main() => {
                let s = self.dock.main_surface_mut();
                if tab == Tab::Notes {
                    s.split_left(path.node, 0.17, vec![tab]);
                } else {
                    s.split_right(path.node, 0.8, vec![tab]);
                }
            }
            _ => self.dock.push_to_focused_leaf(tab),
        }
    }

    /// The start view, as in Grapht: the name, ways in, and the boards used lately.
    fn start_view(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let p = pal(&ctx);
        let logo = self
            .logo
            .get_or_insert_with(|| {
                let img = image::load_from_memory(include_bytes!("../brand/png/overlay-icon-blue-256.png")).map(|i| i.to_rgba8()).unwrap_or_default();
                let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                ctx.load_texture("start-logo", ci, egui::TextureOptions::LINEAR)
            })
            .id();
        let rect = ui.max_rect();
        let w = 440.0f32.min(rect.width() - 40.0);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_center_size(rect.center() - vec2(0.0, 30.0), vec2(w, rect.height().min(640.0)))).layout(egui::Layout::top_down(egui::Align::Center)));
        let ui = &mut child;
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        ui.add(egui::Image::new((logo, vec2(72.0, 72.0))));
        ui.label(egui::RichText::new("Overlay").family(theme::display_family()).size(34.0).color(p.text));
        ui.label(egui::RichText::new("A whiteboard where every image, sketch, shape and note is a node.").size(14.0).color(p.text_secondary));
        ui.add_space(14.0);
        let big = |t: &str| egui::Button::new(egui::RichText::new(t).size(14.0)).min_size(vec2(w, 36.0));
        let mut close = false;
        let last = self.recent.iter().find(|p| p.exists()).cloned();
        if let Some(path) = &last {
            let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            if ui.add(big(&format!("Continue with {name}"))).on_hover_text(path.display().to_string()).clicked() {
                if let Err(e) = self.ed.load(path) {
                    self.ed.say(format!("Could not open {}: {e}", path.display()));
                } else {
                    close = true;
                }
            }
        }
        if ui.add(big("New board")).clicked() {
            self.ed.new_board();
            close = true;
        }
        if ui.add(big("Open…")).clicked() {
            self.ed.open_dialog();
            close = self.ed.path.is_some();
        }
        if ui.add(big("Open the demo board")).on_hover_text("A sample board with pages, frames, notes and a flowchart").clicked() {
            self.ed.load_demo();
            close = true;
        }
        let others: Vec<PathBuf> = self.recent.iter().filter(|p| p.exists() && Some(*p) != last.as_ref()).take(7).cloned().collect();
        if !others.is_empty() {
            ui.add_space(16.0);
            ui.label(egui::RichText::new("Recent boards").size(12.5).color(p.text_secondary));
            for path in others {
                let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let folder = path.parent().map(|f| f.display().to_string()).unwrap_or_default();
                let text = egui::RichText::new(format!("{name}    {folder}")).size(13.0).color(p.text);
                if ui.add(egui::Button::new(text).frame(false).truncate().min_size(vec2(w, 24.0))).on_hover_text(path.display().to_string()).clicked() {
                    if let Err(e) = self.ed.load(&path) {
                        self.ed.say(format!("Could not open {}: {e}", path.display()));
                    } else {
                        close = true;
                    }
                }
            }
        }
        ui.add_space(16.0);
        ui.label(egui::RichText::new("You can also drop a board or images on this window.").size(12.0).color(p.text_secondary));
        if close {
            self.start_open = false;
        }
    }

    fn update_title(&mut self, ui: &Ui) {
        // The app's name, then the board's: "Overlay — name", or "Overlay" alone with no board.
        let t = match self.ed.path.as_ref().and_then(|p| p.file_stem()) {
            Some(stem) => format!("Overlay — {}", stem.to_string_lossy()),
            None if self.ed.dirty => "Overlay — Untitled".to_owned(),
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
            Tab::Notes => "Notes",
            Tab::Frames => "Frames",
            Tab::Note(id) => return self.ed.board.note(*id).map(|n| n.title()).unwrap_or_else(|| "Deleted note".into()).chars().take(28).collect::<String>().into(),
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
                ui.label(theme::secondary(ui, "Tools are in the radial menu: hold the right mouse button on the board. View > Reset panel layout removes this tab."));
            }
            Tab::Calendar => crate::calendar::calendar_ui(ui, self.ed),
            Tab::Notes => panels::notes_ui(ui, self.ed),
            Tab::Frames => panels::frames_ui(ui, self.ed),
            Tab::Note(id) => panels::note_view(ui, self.ed, *id),
            Tab::Properties => panels::properties_ui(ui, self.ed),
            Tab::Layers => panels::layers_ui(ui, self.ed),
            Tab::Graph => {
                ui.label(theme::secondary(ui, "The Graph panel was removed. View > Reset panel layout removes this tab."));
            }
            Tab::History => {
                ui.label(theme::secondary(ui, "History was removed; Ctrl+Z and Ctrl+Y still undo and redo. View > Reset panel layout removes this tab."));
            }
        }
    }

    fn is_closeable(&self, tab: &Tab) -> bool {
        matches!(tab, Tab::Note(_))
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
            Tab::Tools | Tab::Draw | Tab::Shapes | Tab::Flowchart | Tab::Note(_) => {
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

        // Presenting takes the whole window, full screen, until Esc.
        let presenting = self.ed.presenting.is_some();
        if presenting != self.was_presenting {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(presenting));
            self.was_presenting = presenting;
        }
        if presenting {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(egui::Color32::BLACK)).show(ui, |ui| self.ed.present_ui(ui));
            return;
        }

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
            let (boards, rest): (Vec<PathBuf>, Vec<PathBuf>) = dropped.into_iter().partition(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("overlay")));
            // Markdown files become notes, each with a card where they were dropped.
            let (notes, images): (Vec<PathBuf>, Vec<PathBuf>) = rest.into_iter().partition(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown")));
            for (k, path) in notes.iter().enumerate() {
                if let Ok(text) = std::fs::read_to_string(path) {
                    let note = self.ed.add_note(&text, false);
                    self.ed.place_note(note, at + egui::vec2(32.0, 32.0) * k as f32);
                }
            }
            if let Some(b) = boards.first() {
                self.request(&ctx, Pending::OpenFile(b.clone()));
            }
            if !images.is_empty() {
                self.ed.import_image_files(&images, at);
            }
        }

        self.update_title(ui);
        if let Some(path) = self.ed.path.clone() {
            if self.recent.first() != Some(&path) {
                self.recent.retain(|r| *r != path);
                self.recent.insert(0, path);
                self.recent.truncate(10);
            }
            // A board opened by dropping it on the start view closes the start view.
            self.start_open = false;
        }
        if self.start_open {
            let p = pal(&ctx);
            egui::Panel::top("top-bar").frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::symmetric(8, 4))).show(ui, |ui| self.top_bar(ui));
            egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg)).show(ui, |ui| self.start_view(ui));
            crate::settings::show(&ctx, &mut self.settings_win, &mut self.ed, crate::settings::AppPrefs { appearance: &mut self.appearance, ui_scale: &mut self.ui_scale, start_view: &mut self.start_view });
            self.save_dialog(&ctx);
            return;
        }

        let p = pal(&ctx);
        egui::Panel::top("top-bar").frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::symmetric(8, 4))).show(ui, |ui| self.top_bar(ui));
        // New items go on the current page and layer; the item list stays in layer order.
        self.ed.board.fix_pages(&mut self.ed.page);
        self.ed.board.fix_layers(&mut self.ed.active_layer);
        // The Calendar's chosen days hide or fade the other items.
        self.ed.apply_day_filter();
        self.ed.canvas_visible = false;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg)).show(ui, |ui| {
            let style = theme::dock_style(ui.ctx());
            DockArea::new(&mut self.dock)
                .style(style)
                .show_close_buttons(true)
                .show_leaf_close_all_buttons(false)
                .show_leaf_collapse_buttons(false)
                .show_inside(ui, &mut Viewer { ed: &mut self.ed });
        });

        // The radial menu's Properties, and opening a note, bring that panel to the front.
        if std::mem::take(&mut self.ed.want_properties) {
            if let Some(path) = self.dock.find_tab(&Tab::Properties) {
                let _ = self.dock.set_active_tab(path);
            }
        }
        // A note to open goes in a tab beside the board (or comes to the front if already open).
        if let Some(id) = self.ed.open_note.take() {
            let tab = Tab::Note(id);
            if let Some(path) = self.dock.find_tab(&tab) {
                let _ = self.dock.set_active_tab(path);
            } else if let Some(path) = self.dock.find_tab(&Tab::Board) {
                if let Ok(leaf) = self.dock.leaf_mut(path.node_path()) {
                    leaf.append_tab(tab);
                    let last = leaf.tabs.len() - 1;
                    let _ = leaf.set_active_tab(last);
                }
            } else {
                self.dock.push_to_focused_leaf(tab);
            }
        }
        // Tabs of deleted notes (or notes of another board) close, and the board shows in their place.
        let notes: Vec<crate::board::Id> = self.ed.board.notes.iter().map(|n| n.id).collect();
        let stale = self.dock.iter_all_tabs().any(|(_, t)| matches!(t, Tab::Note(id) if !notes.contains(id)));
        if stale {
            self.dock.retain_tabs(|t| !matches!(t, Tab::Note(id) if !notes.contains(id)));
            if let Some(path) = self.dock.find_tab(&Tab::Board) {
                let _ = self.dock.set_active_tab(path);
            }
        }
        // A ring opened on the Notes list.
        if self.ed.radial.as_ref().is_some_and(|r| r.from_list()) {
            crate::radial::update(&mut self.ed, &ctx);
        }
        overview::show(&ctx, &mut self.ed);
        // egui_dock shows a grab cursor over tab titles (they can be dragged to re-dock), which Windows
        // draws as the four-way move arrows. Outside the board, keep the ordinary pointer until a tab
        // is actually being dragged.
        let off_board = ctx.input(|i| i.pointer.hover_pos()).is_some_and(|q| !self.ed.canvas_rect.contains(q));
        if off_board && !ctx.input(|i| i.pointer.any_down()) && ctx.output(|o| o.cursor_icon) == egui::CursorIcon::Grab {
            ctx.set_cursor_icon(egui::CursorIcon::Default);
        }
        crate::insert::show(&ctx, &mut self.ed);
        panels::comments_window(&ctx, &mut self.ed);
        panels::colour_picker(&ctx, &mut self.ed);
        crate::settings::show(&ctx, &mut self.settings_win, &mut self.ed, crate::settings::AppPrefs { appearance: &mut self.appearance, ui_scale: &mut self.ui_scale, start_view: &mut self.start_view });
        if (ctx.zoom_factor() - self.ui_scale).abs() > 1e-3 {
            ctx.set_zoom_factor(self.ui_scale);
        }
        if let Some(c) = crate::search::show(&ctx, &mut self.ed) {
            self.run_cmd(&ctx, c);
        }
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

    /// What a keyboard shortcut does.
    fn run_action(&mut self, ctx: &egui::Context, a: crate::keys::Action) {
        use crate::keys::Action;
        use crate::search::Cmd;
        let cmd = match a {
            Action::Tool(t) => Cmd::Tool(t),
            Action::NewBoard => Cmd::NewBoard,
            Action::Open => Cmd::Open,
            Action::Save => Cmd::Save,
            Action::SaveAs => Cmd::SaveAs,
            Action::Export => Cmd::Export,
            Action::Settings => Cmd::Settings,
            Action::Undo => Cmd::Undo,
            Action::Redo => Cmd::Redo,
            Action::SelectAll => Cmd::SelectAll,
            Action::Duplicate => Cmd::Duplicate,
            Action::Lock => Cmd::Lock,
            Action::Group => Cmd::Group,
            Action::Ungroup => Cmd::Ungroup,
            Action::InsertImage => Cmd::InsertImage,
            Action::ZoomFit => Cmd::ZoomFit,
            Action::ZoomSelection => Cmd::ZoomSelection,
            Action::ActualSize => Cmd::ActualSize,
            Action::Overview => Cmd::Overview,
            Action::Present => Cmd::Present,
            Action::Search => {
                self.ed.palette = if self.ed.palette.is_some() { None } else { Some(crate::search::Palette { focus: true, ..Default::default() }) };
                return;
            }
            Action::AddItem => {
                if self.ed.insert.is_none() {
                    self.ed.open_insert_here();
                }
                return;
            }
            Action::BringFront => return self.ed.reorder_selection(true),
            Action::SendBack => return self.ed.reorder_selection(false),
        };
        self.run_cmd(ctx, cmd);
    }

    /// A command picked in search everything (Ctrl+K).
    fn run_cmd(&mut self, ctx: &egui::Context, c: crate::search::Cmd) {
        use crate::search::{Cmd, Panel};
        let ed = &mut self.ed;
        match c {
            Cmd::NewBoard => self.request(ctx, Pending::New),
            Cmd::Open => self.request(ctx, Pending::Open),
            Cmd::Save => {
                ed.save(false);
            }
            Cmd::SaveAs => {
                ed.save(true);
            }
            Cmd::Export => self.export.open = true,
            Cmd::InsertImage => ed.pick_images(),
            Cmd::Undo => ed.undo(),
            Cmd::Redo => ed.redo(),
            Cmd::ZoomFit => ed.zoom_to_fit(),
            Cmd::ZoomSelection => ed.zoom_to_selection(),
            Cmd::ActualSize => ed.reset_zoom(),
            Cmd::Overview => ed.show_overview = !ed.show_overview,
            Cmd::Snapping => ed.snapping = !ed.snapping,
            Cmd::NoteLinks => ed.show_note_links = !ed.show_note_links,
            Cmd::NewNote => {
                ed.add_note("# New note\n\n", true);
            }
            Cmd::NewFolder => {
                ed.add_folder();
                if let Some(path) = self.dock.find_tab(&Tab::Notes) {
                    let _ = self.dock.set_active_tab(path);
                }
            }
            Cmd::NewPage => ed.add_page(),
            Cmd::NewLayer => {
                let (_, name) = ed.add_layer();
                ed.say(format!("Added {name}; new items go on it"));
            }
            Cmd::Present => ed.start_present(0),
            Cmd::Group => ed.group_selection(),
            Cmd::Ungroup => ed.ungroup_selection(),
            Cmd::Lock => ed.toggle_lock(),
            Cmd::Duplicate => ed.duplicate(),
            Cmd::Delete => ed.delete_selection(),
            Cmd::SelectAll => ed.select_all(),
            Cmd::Dark | Cmd::Light | Cmd::System => {
                let a = match c {
                    Cmd::Dark => Appearance::Dark,
                    Cmd::Light => Appearance::Light,
                    _ => Appearance::System,
                };
                self.appearance = a;
                theme::set_appearance(ctx, a);
            }
            Cmd::ResetLayout => self.dock = default_dock(),
            Cmd::Settings => self.settings_win.open = true,
            Cmd::Tool(t) => {
                ed.finish_edit();
                ed.tool = t;
            }
            Cmd::Show(p) => {
                let tab = match p {
                    Panel::Properties => Tab::Properties,
                    Panel::Notes => Tab::Notes,
                    Panel::Layers => Tab::Layers,
                    Panel::Frames => Tab::Frames,
                };
                if let Some(path) = self.dock.find_tab(&tab) {
                    let _ = self.dock.set_active_tab(path);
                }
            }
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
            show_overview: Some(self.ed.show_overview),
            snapping: Some(self.ed.snapping),
            poly_curved: Some(self.ed.poly_curved),
            show_note_links: Some(self.ed.show_note_links),
            show_grid: Some(self.ed.show_grid),
            radial_empty: Some(self.ed.radial_empty),
            radial_hold_ms: Some(self.ed.radial_hold_ms),
            undo_steps: Some(self.ed.undo_steps),
            note_mode: Some(self.ed.note_mode),
            ui_scale: Some(self.ui_scale),
            reopen_last: None,
            start_view: Some(self.start_view),
            custom_colors: Some(self.ed.custom_colors.clone()),
            recent: Some(self.recent.clone()),
            workspaces: Some(self.workspaces.clone()),
            keys: Some(self.ed.keys.clone()),
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
