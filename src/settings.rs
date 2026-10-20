//! The Settings window: everything that can be set, in sections, with the keyboard shortcuts last.

use crate::board::{self, Brush, Ink, Shape, WireStyle};
use crate::editor::Editor;
use crate::keys::{Action, Combo};
use crate::theme::{self, Appearance, UiExt, pal};
use egui::{CornerRadius, RichText, ScrollArea, Sense, Stroke, Ui, vec2};

const SECTIONS: [&str; 9] = ["Appearance", "Board", "Drawing", "Notes", "Radial menu", "Undo", "Files", "Keyboard shortcuts", "About"];

/// The window's own state, kept by the app.
#[derive(Default)]
pub struct SettingsWindow {
    pub open: bool,
    section: usize,
    /// The action waiting for its new keys.
    capturing: Option<Action>,
}

impl SettingsWindow {
    /// While a new shortcut is being pressed, keys belong to the window.
    pub fn capturing(&self) -> bool {
        self.open && self.capturing.is_some()
    }
}

/// Settings the app keeps itself (the rest live in the editor).
pub struct AppPrefs<'a> {
    pub appearance: &'a mut Appearance,
    pub ui_scale: &'a mut f32,
    pub reopen_last: &'a mut bool,
}

/// One setting: its name and a line about it on the left, the control on the right.
fn row(ui: &mut Ui, name: &str, about: &str, control: impl FnOnce(&mut Ui)) {
    let p = pal(ui.ctx());
    ui.horizontal(|ui| {
        let w = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width((w * 0.5).max(180.0));
            ui.label(RichText::new(name).size(13.5).color(p.text));
            if !about.is_empty() {
                ui.label(RichText::new(about).size(12.0).color(p.text_secondary));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| control(ui));
    });
    ui.add_space(12.0);
}

fn group(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    theme::heading(ui, title);
    ui.add_space(6.0);
}

/// The colour swatches (right to left, so the theme ink ends up first on the left).
fn ink_swatches(ui: &mut Ui, current: Ink) -> Option<Ink> {
    let p = pal(ui.ctx());
    let mut picked = None;
    ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
    for ink in board::swatches().into_iter().rev() {
        let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
        if ink == current {
            ui.painter().circle_stroke(r.center(), 9.5, Stroke::new(1.5, p.accent));
        } else if resp.hovered() {
            ui.painter().circle_stroke(r.center(), 9.5, Stroke::new(1.0, p.hairline));
        }
        ui.painter().circle(r.center(), 6.5, ink.color(p), Stroke::new(1.0, p.hairline));
        if resp.on_hover_text(if ink == Ink::Auto { "Ink (follows the theme)" } else { "Colour" }).clicked() {
            picked = Some(ink);
        }
    }
    picked
}

fn width_slider(ui: &mut Ui, w: &mut f32) -> bool {
    ui.spacing_mut().slider_width = 170.0;
    ui.add(egui::Slider::new(w, board::MIN_WIDTH..=board::MAX_WIDTH).logarithmic(true).max_decimals(2)).changed()
}

pub fn show(ctx: &egui::Context, win: &mut SettingsWindow, ed: &mut Editor, mut app: AppPrefs) {
    if !win.open {
        return;
    }
    let p = pal(ctx);
    let screen = ctx.content_rect();
    let size = vec2(840.0f32.min(screen.width() - 40.0), 600.0f32.min(screen.height() - 60.0));
    let frame = egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::same(0)).corner_radius(12);
    let modal = egui::Modal::new(egui::Id::new("settings")).frame(frame).show(ctx, |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // ── sections, on the left ──
            egui::Frame::new().fill(p.bg).corner_radius(CornerRadius { nw: 12, sw: 12, ne: 0, se: 0 }).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_width(176.0);
                    ui.set_min_height(size.y - 24.0);
                    ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                    ui.label(theme::semibold("Settings").size(16.0).color(p.text));
                    ui.add_space(12.0);
                    for (k, name) in SECTIONS.iter().enumerate() {
                        let sel = win.section == k;
                        let text = RichText::new(*name).size(13.0).color(if sel { p.text } else { p.text_secondary });
                        if ui.add(egui::Button::selectable(sel, text).min_size(vec2(176.0, 30.0))).clicked() {
                            win.section = k;
                            win.capturing = None;
                        }
                    }
                });
            });
            // ── the section, on the right ──
            ui.vertical(|ui| {
                ui.set_width(size.x - 200.0);
                ui.set_min_height(size.y);
                ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(theme::semibold(SECTIONS[win.section]).size(18.0).color(p.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(14.0);
                        if ui.add(egui::Button::new(RichText::new("×").size(17.0)).frame(false)).on_hover_text("Close (Esc)").clicked() {
                            win.open = false;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.separator();
                ScrollArea::vertical().id_salt(("settings-body", win.section)).auto_shrink([false, false]).show(ui, |ui| {
                    egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 22, top: 4, bottom: 16 }).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        match win.section {
                            0 => appearance(ui, &mut app),
                            1 => board_section(ui, ed),
                            2 => drawing(ui, ed),
                            3 => notes(ui, ed),
                            4 => radial(ui, ed),
                            5 => undo(ui, ed),
                            6 => files(ui, &mut app),
                            7 => shortcuts(ui, win, ed),
                            _ => about(ui),
                        }
                    });
                });
            });
        });
    });
    // Esc or a click outside closes it, unless Esc is cancelling a shortcut being set.
    if modal.should_close() && win.capturing.is_none() {
        win.open = false;
    }
    if !win.open {
        win.capturing = None;
    }
}

fn appearance(ui: &mut Ui, app: &mut AppPrefs) {
    group(ui, "LOOK");
    row(ui, "Mode", "Light, dark, or the same as Windows.", |ui| {
        ui.set_width(230.0);
        let mut a = *app.appearance;
        if ui.segmented(&mut a, &[(Appearance::System, "System"), (Appearance::Dark, "Dark"), (Appearance::Light, "Light")]) {
            *app.appearance = a;
            theme::set_appearance(ui.ctx(), a);
        }
    });
    row(ui, "Interface size", "Makes text, panels and controls larger or smaller. The board's own zoom is separate.", |ui| {
        let mut s = *app.ui_scale * 100.0;
        ui.spacing_mut().slider_width = 170.0;
        if ui.add(egui::Slider::new(&mut s, 75.0..=150.0).suffix("%").step_by(5.0)).changed() {
            *app.ui_scale = s / 100.0;
        }
    });
}

fn board_section(ui: &mut Ui, ed: &mut Editor) {
    group(ui, "SHOW");
    row(ui, "Grid dots", "The dots behind the board.", |ui| {
        ui.switch(&mut ed.show_grid, "");
    });
    row(ui, "Overview", "The small map of the whole board (M).", |ui| {
        ui.switch(&mut ed.show_overview, "");
    });
    row(ui, "Links between note cards", "Dotted lines join the cards of notes that link to each other with [[Name]].", |ui| {
        ui.switch(&mut ed.show_note_links, "");
    });
    group(ui, "MOVING");
    row(ui, "Snap to items", "Line items up with each other's edges, centres and gaps while moving or resizing. Hold Alt to skip it for one drag.", |ui| {
        ui.switch(&mut ed.snapping, "");
    });
    group(ui, "NEW ITEMS");
    row(ui, "Connections", "How new connections are drawn.", |ui| {
        ui.set_width(230.0);
        ui.segmented(&mut ed.wire_style, &[(WireStyle::Curve, "Curve"), (WireStyle::Straight, "Straight"), (WireStyle::Elbow, "Elbow")]);
    });
    row(ui, "Flowchart box", "The shape the Flowchart box tool (N) draws.", |ui| {
        egui::ComboBox::from_id_salt("settings-node-shape").width(180.0).selected_text(ed.node_shape.name()).show_ui(ui, |ui| {
            for s in Shape::FLOW {
                ui.selectable_value(&mut ed.node_shape, s, s.name());
            }
        });
    });
}

fn drawing(ui: &mut Ui, ed: &mut Editor) {
    group(ui, "COLOUR");
    row(ui, "Colour for new items", "Strokes, lines, shapes and text start in this colour.", |ui| {
        if let Some(ink) = ink_swatches(ui, ed.style.ink) {
            ed.style.ink = ink;
        }
    });
    group(ui, "BRUSHES");
    for b in Brush::ALL {
        let name = match b {
            Brush::Pen => "Pen width",
            Brush::Pencil => "Pencil width",
            Brush::Marker => "Marker width",
            Brush::Highlighter => "Highlighter width",
        };
        row(ui, name, "", |ui| {
            let mut w = ed.brush_widths[b.index()];
            if width_slider(ui, &mut w) {
                ed.brush_widths[b.index()] = w;
            }
        });
    }
    group(ui, "LINES, SHAPES AND TEXT");
    row(ui, "Line width", "For lines, arrows, polylines and shape outlines.", |ui| {
        let mut w = ed.style.width;
        if width_slider(ui, &mut w) {
            ed.style.width = w;
        }
    });
    row(ui, "Text size", "For new text and labels.", |ui| {
        ui.spacing_mut().slider_width = 170.0;
        ui.add(egui::Slider::new(&mut ed.style.font_size, 8.0..=72.0).step_by(1.0));
    });
    row(ui, "Polylines", "Straight segments, or a smooth curve through the points.", |ui| {
        ui.set_width(200.0);
        ui.segmented(&mut ed.poly_curved, &[(false, "Straight"), (true, "Curved")]);
    });
}

fn notes(ui: &mut Ui, ed: &mut Editor) {
    group(ui, "NOTE TABS");
    row(ui, "Open notes in", "What a note shows when it opens: the text to write, the formatted note, or both side by side.", |ui| {
        ui.set_width(230.0);
        ui.segmented(&mut ed.note_mode, &[(0, "Write"), (1, "Split"), (2, "Read")]);
    });
}

fn radial(ui: &mut Ui, ed: &mut Editor) {
    group(ui, "OPENING");
    row(ui, "Open on empty board", "Hold the right mouse button still on empty board to open the menu there. When off, the right button only pans on empty board; on an item it always opens the menu.", |ui| {
        ui.switch(&mut ed.radial_empty, "");
    });
    row(ui, "Hold time", "How long to hold the right button still on empty board before the menu opens.", |ui| {
        ui.spacing_mut().slider_width = 170.0;
        ui.add_enabled(ed.radial_empty, egui::Slider::new(&mut ed.radial_hold_ms, 150..=1000).suffix(" ms").step_by(50.0));
    });
}

fn undo(ui: &mut Ui, ed: &mut Editor) {
    group(ui, "UNDO");
    row(ui, "Changes kept", "How many changes Ctrl+Z can take back. They are kept while Overlay is open, not in the file.", |ui| {
        ui.spacing_mut().slider_width = 170.0;
        let mut n = ed.undo_steps;
        if ui.add(egui::Slider::new(&mut n, 20..=500).step_by(10.0)).changed() {
            ed.undo_steps = n;
            ed.history.set_limit(n);
        }
    });
}

fn files(ui: &mut Ui, app: &mut AppPrefs) {
    group(ui, "STARTING");
    row(ui, "Reopen the last board", "Open the board you had open when Overlay starts.", |ui| {
        ui.switch(app.reopen_last, "");
    });
}

fn shortcuts(ui: &mut Ui, win: &mut SettingsWindow, ed: &mut Editor) {
    let p = pal(ui.ctx());
    // A shortcut being set takes the next key pressed.
    if let Some(a) = win.capturing {
        let press = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        if let Some((key, m)) = press {
            win.capturing = None;
            if !(key == egui::Key::Escape && m.is_none()) {
                let c = Combo::from_press(key, m);
                if let Some(other) = ed.keys.user_of(&c, a) {
                    ed.keys.set(other, None);
                    ed.say(format!("{} moved from {} to {}", c.label(), other.name(), a.name()));
                }
                ed.keys.set(a, Some(c));
            }
        }
    }
    ui.label(RichText::new("Click a shortcut, then press the keys for it. A shortcut can only belong to one action: giving it to another moves it there.").size(12.0).color(p.text_secondary));
    ui.add_space(6.0);
    if ui.button("Reset all to the defaults").clicked() {
        ed.keys.reset_all();
        win.capturing = None;
    }
    for (title, actions) in crate::keys::sections() {
        group(ui, &title.to_uppercase());
        for a in actions {
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 300.0).max(80.0);
                ui.allocate_ui_with_layout(vec2(w, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(RichText::new(a.name()).size(13.0).color(p.text)).truncate());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Back to its default.
                    let changed = !ed.keys.is_default(a);
                    if ui.add_enabled(changed, egui::Button::new(RichText::new("Reset").size(11.5)).frame(false)).on_hover_text("Back to the default").clicked() {
                        let d = crate::keys::default_combo(a);
                        if let Some(c) = &d {
                            if let Some(other) = ed.keys.user_of(c, a) {
                                ed.keys.set(other, None);
                            }
                        }
                        ed.keys.set(a, d);
                    }
                    if ui.add_enabled(ed.keys.get(a).is_some(), egui::Button::new(RichText::new("Clear").size(11.5)).frame(false)).on_hover_text("No shortcut").clicked() {
                        ed.keys.set(a, None);
                    }
                    let waiting = win.capturing == Some(a);
                    let text = if waiting { "Press keys… (Esc cancels)".to_owned() } else { ed.keys.get(a).map(|c| c.label()).unwrap_or_else(|| "None".into()) };
                    let rt = RichText::new(text).size(12.5).color(if waiting { p.accent_text } else if ed.keys.get(a).is_some() { p.text } else { p.text_secondary });
                    let b = egui::Button::new(rt).min_size(vec2(150.0, 26.0)).corner_radius(CornerRadius::same(6)).stroke(Stroke::new(1.0, if waiting { p.accent } else { p.hairline }));
                    if ui.add(b).on_hover_text("Click, then press the new keys").clicked() {
                        win.capturing = if waiting { None } else { Some(a) };
                    }
                });
            });
            ui.add_space(4.0);
        }
    }
    group(ui, "FIXED KEYS");
    for (keys, what) in [
        ("Esc", "Stop: drop the selection, cancel a polyline, close lists and menus"),
        ("Delete, Backspace", "Delete the selection (or the picked notes)"),
        ("Enter", "Edit the selected item's label"),
        ("Arrow keys", "Nudge the selection (Shift for 10)"),
        ("Ctrl+C, Ctrl+X, Ctrl+V", "Copy, cut, paste"),
        ("Space + drag", "Pan the board"),
        ("Shift while drawing", "Keep lines at 45° steps, boxes square, frames 16:9"),
        ("Alt while moving", "Skip snapping"),
        ("Ctrl+Shift+Z", "Redo, as well as the Redo shortcut"),
    ] {
        ui.horizontal(|ui| {
            ui.label(RichText::new(what).size(13.0).color(p.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(keys).size(12.5).color(p.text_secondary));
            });
        });
        ui.add_space(4.0);
    }
}

fn about(ui: &mut Ui) {
    let p = pal(ui.ctx());
    ui.add_space(8.0);
    ui.label(theme::semibold(format!("Overlay {}", env!("CARGO_PKG_VERSION"))).size(15.0).color(p.text));
    ui.label(RichText::new("A whiteboard where every image, sketch, shape and note is a node.").size(13.0).color(p.text_secondary));
    ui.add_space(10.0);
    ui.label(RichText::new("Written in Rust with egui. The look follows the Grapht design system. Released under the MIT licence.").size(12.5).color(p.text_secondary));
    ui.add_space(6.0);
    ui.hyperlink_to("github.com/gowthamjeyabalan/overlay", "https://github.com/gowthamjeyabalan/overlay");
}
