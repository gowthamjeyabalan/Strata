//! The floating toolbox and the dockable panels: Properties, Layers and Graph.

use crate::board::{self, Dash, Id, Ink, Kind, Shape, Style, WireStyle};
use crate::canvas::paint_shape;
use crate::editor::{Editor, Pick, plural};
use crate::shapes;
use crate::theme::{self, UiExt, pal};
use crate::tools::{self, Tool};
use egui::{Color32, CornerRadius, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2};

/// Icon button: a squircle tile. Selected uses the accent fill, the only chrome colour.
fn icon_button(ui: &mut Ui, size: f32, selected: bool, tip: &str, paint: impl FnOnce(&egui::Painter, egui::Rect, Color32, Color32)) -> egui::Response {
    let p = pal(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let bg = if selected {
        p.accent
    } else if resp.is_pointer_button_down_on() {
        p.bg_control_active
    } else if resp.hovered() {
        p.bg_control_hover
    } else {
        p.bg_control
    };
    ui.painter().rect_filled(rect, CornerRadius::same(10), bg);
    let (fg, dot) = if selected { (Color32::WHITE, Color32::WHITE) } else { (p.text, p.accent) };
    paint(ui.painter(), rect.shrink(size * 0.2), fg, dot);
    resp.on_hover_text(tip)
}

fn swatch(ui: &mut Ui, ink: Ink, selected: bool) -> egui::Response {
    let p = pal(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    let c = rect.center();
    if selected {
        ui.painter().circle_stroke(c, 9.5, Stroke::new(1.5, p.accent));
    } else if resp.hovered() {
        ui.painter().circle_stroke(c, 9.5, Stroke::new(1.0, p.hairline));
    }
    ui.painter().circle(c, 6.5, ink.color(p), Stroke::new(1.0, p.hairline));
    resp.on_hover_text(if ink == Ink::Auto { "Ink (follows the theme)" } else { "Colour" })
}

/// Swatch row. Returns the picked ink if it changed.
fn ink_row(ui: &mut Ui, current: Ink) -> Option<Ink> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for ink in board::swatches() {
            if swatch(ui, ink, ink == current).clicked() && ink != current {
                picked = Some(ink);
            }
        }
    });
    picked
}

const TOOL_PX: f32 = 32.0;

/// A wrapped grid of tool buttons.
fn tool_grid(ui: &mut Ui, ed: &mut Editor, list: &[Tool]) {
    ui.horizontal_wrapped(|ui| {
        for &t in list {
            let tip = format!("{} ({})", t.name(), t.key_name());
            if icon_button(ui, TOOL_PX, ed.tool == t, &tip, |p, r, c, d| tools::paint_icon(p, r, t, c, d)).clicked() {
                ed.finish_edit();
                ed.tool = t;
            }
        }
    });
}

/// "12", "2.5", "0.25": short width labels.
fn width_text(w: f32) -> String {
    if w >= 10.0 {
        format!("{w:.0}")
    } else {
        let s = format!("{w:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// Width of the current tool: presets from hairline to very thick, plus a logarithmic slider.
fn width_control(ui: &mut Ui, ed: &mut Editor) {
    let p = pal(ui.ctx());
    let current = ed.tool_width();
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        for w in [0.5f32, 2.0, 6.0, 16.0, 40.0, 100.0] {
            let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
            let sel = (current - w).abs() < 0.01;
            if sel {
                ui.painter().rect_filled(rect, CornerRadius::same(6), p.bg_control_active);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, CornerRadius::same(6), p.bg_control_hover);
            }
            // Dot size grows with the log of the width so the whole range fits in 20 px.
            let t = (w.ln() - board::MIN_WIDTH.ln()) / (board::MAX_WIDTH.ln() - board::MIN_WIDTH.ln());
            ui.painter().circle_filled(rect.center(), 1.0 + t * 7.5, p.text);
            if resp.on_hover_text(format!("Width {}", width_text(w))).clicked() {
                picked = Some(w);
            }
        }
    });
    let mut w = current;
    ui.spacing_mut().slider_width = (ui.available_width() - 4.0).max(40.0);
    let resp = ui.add(egui::Slider::new(&mut w, board::MIN_WIDTH..=board::MAX_WIDTH).logarithmic(true).show_value(false));
    if resp.changed() {
        picked = Some(w);
    }
    ui.label(theme::secondary(ui, format!("Width {}", width_text(picked.unwrap_or(current)))));
    if let Some(w) = picked {
        ed.set_tool_width(w);
        // A selected stroke or shape takes the new width too.
        apply_to_selection(ed,|s| s.width = w);
    }
}

fn pick_tool(ed: &mut Editor, t: Tool) {
    ed.finish_edit();
    ed.tool = t;
}

/// Width of four tool tiles and the gaps between them.
const TOOLBOX_W: f32 = 4.0 * TOOL_PX + 3.0 * 4.0;

/// The floating toolbox over the board: picking and view switches, then Draw, Shapes and Flowchart.
pub fn toolbox(ctx: &egui::Context, ed: &mut Editor, show_overview: &mut bool) {
    let canvas = ed.canvas_rect;
    if canvas.width() < 120.0 || canvas.height() < 120.0 {
        return;
    }
    let p = pal(ctx);
    let collapsed_id = egui::Id::new("toolbox-collapsed");
    let mut collapsed = ctx.data(|d| d.get_temp::<bool>(collapsed_id)).unwrap_or(false);
    let frame = egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::same(10));
    // A floating card: drag it by its title or any empty part.
    egui::Area::new(egui::Id::new("toolbox"))
        .default_pos(canvas.left_top() + vec2(12.0, 12.0))
        .constrain_to(canvas)
        .movable(true)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(TOOLBOX_W);
                ui.horizontal(|ui| {
                    ui.label(theme::semibold("Tools").size(12.0).color(p.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let tip = if collapsed { "Show the tools" } else { "Fold the toolbox to its title" };
                        // A small chevron: down while open, right while folded.
                        let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                        let c = if resp.hovered() { p.accent } else { p.text_secondary };
                        let m = r.center();
                        let pts = if collapsed { vec![m + vec2(-2.0, -4.0), m + vec2(2.0, 0.0), m + vec2(-2.0, 4.0)] } else { vec![m + vec2(-4.0, -2.0), m + vec2(0.0, 2.0), m + vec2(4.0, -2.0)] };
                        ui.painter().add(egui::Shape::line(pts, Stroke::new(1.6, c)));
                        if resp.on_hover_text(tip).clicked() {
                            collapsed = !collapsed;
                        }
                    });
                });
                if collapsed {
                    return;
                }
                ui.add_space(4.0);
                // Scrolls when the board is shorter than the toolbox.
                let room = (canvas.height() - 70.0).max(80.0);
                ScrollArea::vertical().max_height(room).min_scrolled_height(room.min(640.0)).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_width(TOOLBOX_W);
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            tool_grid(ui, ed, &[Tool::Select, Tool::Hand, Tool::Connect, Tool::Text]);
            ui.horizontal(|ui| {
                if icon_button(ui, TOOL_PX, false, "Insert image (Ctrl+I)", tools::paint_image_icon).clicked() {
                    ed.pick_images();
                }
                if icon_button(ui, TOOL_PX, false, "Zoom to fit (Shift+1)", tools::paint_fit_icon).clicked() {
                    ed.zoom_to_fit();
                }
                if icon_button(ui, TOOL_PX, *show_overview, "Overview (M)", tools::paint_overview_icon).clicked() {
                    *show_overview = !*show_overview;
                }
                let snap_tip = if ed.snapping { "Snapping is on: items line up with each other. Alt skips it for one drag." } else { "Snapping is off" };
                if icon_button(ui, TOOL_PX, ed.snapping, snap_tip, tools::paint_snap_icon).clicked() {
                    ed.snapping = !ed.snapping;
                }
            });

            theme::heading(ui, "DRAW");
            tool_grid(ui, ed, &[Tool::Pen, Tool::Pencil, Tool::Marker, Tool::Highlighter, Tool::Eraser]);
            ui.add_space(2.0);
            width_control(ui, ed);
            if let Some(ink) = ink_row(ui, ed.style.ink) {
                ed.style.ink = ink;
                apply_to_selection(ed, |s| s.ink = ink);
            }

            theme::heading(ui, "SHAPES");
            tool_grid(ui, ed, &[Tool::Line, Tool::Arrow, Tool::Polyline, Tool::Rectangle, Tool::Ellipse]);
            let mut curved = ed.poly_curved;
            if ui.segmented(&mut curved, &[(false, "Straight"), (true, "Curved")]) {
                ed.poly_curved = curved;
                pick_tool(ed, Tool::Polyline);
            }
            if ed.tool == Tool::Polyline {
                ui.label(theme::secondary(ui, "Click to add points. Double-click or Enter ends the line; click the first point to close it."));
            }

            theme::heading(ui, "FLOWCHART");
            ui.horizontal_wrapped(|ui| {
                // Choosing a shape switches to the flowchart box tool.
                for s in Shape::FLOW {
                    let sel = ed.tool == Tool::Node && ed.node_shape == s;
                    let resp = icon_button(ui, TOOL_PX, sel, s.name(), |p, r, c, _| {
                        let r = if matches!(s, Shape::Connector) { egui::Rect::from_center_size(r.center(), vec2(r.height(), r.height()) * 0.8) } else { r.shrink2(vec2(0.0, r.height() * 0.15)) };
                        paint_shape(p, s, r, 0.35, Color32::TRANSPARENT, Stroke::new(1.4, c));
                    });
                    if resp.clicked() {
                        ed.node_shape = s;
                        pick_tool(ed, Tool::Node);
                    }
                }
            });
            ui.add_space(2.0);
            egui::ComboBox::from_id_salt("flow-wire-style").width(TOOLBOX_W).selected_text(format!("{} connections", ed.wire_style.name())).show_ui(ui, |ui| {
                for s in [WireStyle::Curve, WireStyle::Straight, WireStyle::Elbow] {
                    ui.selectable_value(&mut ed.wire_style, s, s.name());
                }
            });
                });
            });
        });
    ctx.data_mut(|d| d.insert_temp(collapsed_id, collapsed));
}

/// Apply a style change to every selected object. A burst of changes (dragging a slider) becomes one state.
pub(crate) fn apply_to_selection(ed: &mut Editor, f: impl Fn(&mut Style)) {
    if ed.selection.is_empty() {
        return;
    }
    ed.mark_changed();
    // A sketch's look is its strokes' look.
    let sketches: Vec<Id> = ed.selection.iter().copied().filter(|id| ed.board.get(*id).is_some_and(|o| o.is_sketch())).collect();
    let ids: Vec<Id> = ed.selection.iter().copied().chain(ed.board.with_descendants(&sketches)).collect();
    for id in ids {
        if let Some(o) = ed.board.get_mut(id) {
            f(&mut o.style);
        }
    }
}

fn kind_summary(ed: &Editor) -> String {
    let kinds: Vec<&str> = ed.selection.iter().filter_map(|id| ed.board.get(*id)).map(|o| o.kind.name()).collect();
    if kinds.len() == 1 {
        return format!("1 {}", kinds[0].to_lowercase());
    }
    if !kinds.is_empty() && kinds.iter().all(|k| *k == kinds[0]) {
        let k = kinds[0].to_lowercase();
        let many = if k.ends_with('s') { format!("{k}es") } else { format!("{k}s") };
        return format!("{} {many}", kinds.len());
    }
    plural(kinds.len(), "object", "objects")
}

pub fn properties_ui(ui: &mut Ui, ed: &mut Editor) {
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        theme::panel_body(ui, |ui| {
            if let Some(w) = ed.selected_wire {
                wire_props(ui, ed, w);
            } else if ed.selection.is_empty() {
                new_object_props(ui, ed);
            } else {
                selection_props(ui, ed);
            }
        });
    });
}

fn wire_style_control(ui: &mut Ui, style: &mut WireStyle) -> bool {
    ui.segmented(style, &[(WireStyle::Curve, WireStyle::Curve.name()), (WireStyle::Straight, WireStyle::Straight.name()), (WireStyle::Elbow, WireStyle::Elbow.name())])
}

fn new_object_props(ui: &mut Ui, ed: &mut Editor) {
    theme::heading(ui, "NEW OBJECTS");
    ui.label(theme::secondary(ui, format!("{} tool. Settings here apply to what you draw next.", ed.tool.name())));
    if ed.tool.brush().is_some() {
        theme::heading(ui, "LINE");
        if let Some(ink) = ink_row(ui, ed.style.ink) {
            ed.style.ink = ink;
        }
        let mut w = ed.tool_width();
        if ui.add(egui::Slider::new(&mut w, board::MIN_WIDTH..=board::MAX_WIDTH).logarithmic(true).text("Width")).changed() {
            ed.set_tool_width(w);
        }
        ui.label(theme::secondary(ui, format!("Each drawing tool keeps its own width. This is the {}'s.", ed.tool.name().to_lowercase())));
    } else {
        let corners = match ed.tool {
            Tool::Node if shapes::has_corners(ed.node_shape) => Some(ed.node_shape),
            Tool::Rectangle => Some(Shape::Rectangle),
            _ => None,
        };
        let mut s = ed.style;
        style_editor(ui, &mut s, corners, 60.0, &mut ed.eyedropper, false);
        ed.style = s;
    }
    theme::heading(ui, "TEXT");
    ui.add(egui::Slider::new(&mut ed.style.font_size, 8.0..=72.0).text("Size"));
    theme::heading(ui, "FLOWCHART BOX");
    shape_combo(ui, "new-shape", &mut ed.node_shape, &Shape::FLOW);
    theme::heading(ui, "CONNECTIONS");
    wire_style_control(ui, &mut ed.wire_style);
}

fn dash_control(ui: &mut Ui, dash: &mut Dash) -> bool {
    ui.segmented(dash, &[(Dash::Solid, "Solid"), (Dash::Dashed, "Dashed"), (Dash::Dotted, "Dotted")])
}

/// Copy every field that differs between `before` and `after` onto `t`, leaving the rest of `t` as it
/// was (so editing one property of a mixed selection changes only that property).
fn copy_changes(before: &Style, after: &Style, t: &mut Style) {
    macro_rules! copy {
        ($($f:ident),*) => { $( if before.$f != after.$f { t.$f = after.$f; } )* };
    }
    copy!(ink, width, fill, font_size, fill_color, gradient, line, dash, radius, shadow, glass, sketch);
}

fn rgb_of(ink: Ink, p: &crate::theme::Pal) -> [u8; 3] {
    let c = ink.color(p);
    [c.r(), c.g(), c.b()]
}

/// A small pipette button. While on, the next click on the board takes a colour.
fn eyedropper_button(ui: &mut Ui, on: bool, tip: &str) -> egui::Response {
    icon_button(ui, 24.0, on, tip, |p, r, c, _| {
        let s = r.width() / 20.0;
        let at = |x: f32, y: f32| r.min + vec2(x, y) * s;
        let st = Stroke::new(1.5, c);
        p.line_segment([at(4.0, 16.0), at(12.0, 8.0)], st);
        p.add(egui::Shape::closed_line(vec![at(11.0, 5.0), at(15.0, 9.0), at(18.0, 6.0), at(14.0, 2.0)], st));
        p.circle_filled(at(4.0, 16.0), 1.6, c);
    })
}

/// Paged grid of ready-made looks. Returns the picked one.
fn preset_grid(ui: &mut Ui) -> Option<board::Preset> {
    let p = pal(ui.ctx());
    let pages = board::preset_pages();
    let page_id = ui.id().with("preset-page");
    let mut page: usize = ui.data(|d| d.get_temp(page_id)).unwrap_or(0).min(pages.len() - 1);
    let mut picked = None;
    // Two rows of 22 px; the arrows sit centred beside them.
    let row = vec2(ui.available_width(), 2.0 * 22.0 + 4.0);
    ui.allocate_ui_with_layout(row, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        if ui.add(egui::Button::new("‹").frame(false)).on_hover_text("Previous").clicked() {
            page = (page + pages.len() - 1) % pages.len();
        }
        let avail = ui.available_width() - 22.0;
        let cell = vec2(((avail - 3.0 * 4.0) / 4.0).max(24.0), 22.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for row in pages[page].1.chunks(4) {
                ui.horizontal(|ui| {
                    for preset in row {
                        let (rect, resp) = ui.allocate_exact_size(cell, Sense::click());
                        let ink = preset.ink.color(p);
                        let fill = if !preset.fill {
                            shapes::Fill::None
                        } else {
                            match preset.fill_color {
                                None => shapes::Fill::Solid(ink.gamma_multiply(0.18)),
                                Some(c) if preset.gradient => shapes::Fill::Gradient(c.color(p), c.color(p).lerp_to_gamma(p.canvas, 0.65)),
                                Some(c) => shapes::Fill::Solid(c.color(p)),
                            }
                        };
                        let look = shapes::Look { fill, stroke: Some(Stroke::new(if resp.hovered() { 2.0 } else { 1.2 }, ink)), ..shapes::Look::simple(Color32::TRANSPARENT, Stroke::NONE) };
                        shapes::paint(ui.painter(), Shape::Rectangle, rect.shrink(1.0), 1.0, &shapes::Look { radius: Some(4.0), ..look });
                        if resp.on_hover_text(pages[page].0).clicked() {
                            picked = Some(*preset);
                        }
                    }
                });
            }
        });
        if ui.add(egui::Button::new("›").frame(false)).on_hover_text("Next").clicked() {
            page = (page + 1) % pages.len();
        }
    });
    // Page dots.
    ui.horizontal(|ui| {
        let w = pages.len() as f32 * 12.0;
        ui.add_space(((ui.available_width() - w) * 0.5).max(0.0));
        for k in 0..pages.len() {
            let (r, resp) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::click());
            if k == page {
                ui.painter().circle_filled(r.center(), 3.5, p.accent);
            } else {
                ui.painter().circle_stroke(r.center(), 3.0, Stroke::new(1.0, p.text_secondary));
            }
            if resp.on_hover_text(pages[k].0).clicked() {
                page = k;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(page_id, page));
    picked
}

/// Style, fill, line and effects of a shape. `corners` is the shape if its corners can be rounded.
fn style_editor(ui: &mut Ui, s: &mut Style, corners: Option<Shape>, max_radius: f32, eyedropper: &mut Option<Pick>, can_pick: bool) {
    let p = pal(ui.ctx());

    theme::heading(ui, "STYLE");
    if let Some(preset) = preset_grid(ui) {
        preset.apply(s);
    }

    theme::heading(ui, "FILL");
    ui.switch(&mut s.fill, "Fill");
    if s.fill {
        let mut auto = s.fill_color.is_none();
        ui.horizontal(|ui| {
            ui.scope(|ui| {
                ui.set_width(150.0);
                if ui.segmented(&mut auto, &[(true, "Automatic"), (false, "Colour")]) {
                    s.fill_color = if auto { None } else { Some(Ink::Rgb(rgb_of(s.ink, p))) };
                }
            });
            let mut rgb = rgb_of(s.fill_color.unwrap_or(s.ink), p);
            if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).on_hover_text("Fill colour").changed() {
                s.fill_color = Some(Ink::Rgb(rgb));
            }
            if can_pick {
                let on = *eyedropper == Some(Pick::Fill);
                if eyedropper_button(ui, on, "Take the fill colour from the board").clicked() {
                    *eyedropper = if on { None } else { Some(Pick::Fill) };
                }
            }
        });
        if auto {
            ui.label(theme::secondary(ui, "A light tint of the line colour."));
        }
        ui.switch(&mut s.gradient, "Gradient");
    }

    theme::heading(ui, "LINE");
    ui.switch(&mut s.line, "Line");
    if s.line {
        ui.horizontal(|ui| {
            if let Some(ink) = ink_row(ui, s.ink) {
                s.ink = ink;
            }
        });
        ui.horizontal(|ui| {
            let mut rgb = rgb_of(s.ink, p);
            if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).on_hover_text("Line colour").changed() {
                s.ink = Ink::Rgb(rgb);
            }
            if can_pick {
                let on = *eyedropper == Some(Pick::Line);
                if eyedropper_button(ui, on, "Take the line colour from the board").clicked() {
                    *eyedropper = if on { None } else { Some(Pick::Line) };
                }
            }
            ui.label(theme::secondary(ui, "Custom"));
        });
        dash_control(ui, &mut s.dash);
        ui.add(egui::Slider::new(&mut s.width, board::MIN_WIDTH..=40.0).logarithmic(true).text("Width"));
    }
    if eyedropper.is_some() && can_pick {
        ui.label(egui::RichText::new("Click anything on the board to take its colour. Esc cancels.").color(p.accent_text));
    }

    theme::heading(ui, "EFFECTS");
    let default_round = corners.map(|sh| shapes::default_radius(sh) > 0.0).unwrap_or(false);
    let mut rounded = s.radius.map(|r| r > 0.0).unwrap_or(default_round);
    let col = ((ui.available_width() - 12.0) * 0.5).max(90.0);
    egui::Grid::new(ui.id().with("effects")).num_columns(2).spacing(vec2(12.0, 7.0)).min_col_width(col).show(ui, |ui| {
        ui.add_enabled_ui(corners.is_some(), |ui| {
            if ui.switch(&mut rounded, "Rounded").changed() {
                s.radius = Some(if rounded { corners.map(shapes::default_radius).filter(|r| *r > 2.0 && r.is_finite()).unwrap_or(12.0) } else { 0.0 });
            }
        });
        ui.switch(&mut s.sketch, "Sketch");
        ui.end_row();
        ui.switch(&mut s.glass, "Glass");
        ui.switch(&mut s.shadow, "Shadow");
        ui.end_row();
    });
    if rounded {
        if let Some(sh) = corners {
            let mut r = s.radius.unwrap_or_else(|| shapes::default_radius(sh)).min(max_radius);
            if ui.add(egui::Slider::new(&mut r, 0.0..=max_radius.max(1.0)).text("Corner")).changed() {
                s.radius = Some(r.round());
            }
        }
    }
}

fn shape_combo(ui: &mut Ui, id: &str, shape: &mut Shape, options: &[Shape]) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(shape.name()).show_ui(ui, |ui| {
        for s in options {
            changed |= ui.selectable_value(shape, *s, s.name()).changed();
        }
    });
    changed
}

fn selection_props(ui: &mut Ui, ed: &mut Editor) {
    let first = ed.selection[0];
    let Some(o) = ed.board.get(first).cloned() else { return };
    let single = ed.selection.len() == 1;
    let is_group = o.is_group() && !o.is_sketch();
    theme::heading(ui, if single && is_group { "GROUP" } else if single && o.is_sketch() { "SKETCH" } else { "SELECTION" });
    if single && o.is_sketch() {
        let n = ed.board.members(first).len();
        ui.label(theme::secondary(ui, format!("{} joined into one. Ctrl+Shift+G splits it.", plural(n, "stroke", "strokes"))));
    } else if single && is_group {
        let n = ed.board.members(first).len();
        ui.label(theme::secondary(ui, format!("Holds {}", plural(n, "item", "items"))));
    } else {
        ui.label(theme::secondary(ui, kind_summary(ed)));
    }

    if single {
        let heading = match o.kind {
            Kind::Image { .. } | Kind::Path { .. } | Kind::Group { sketch: true, .. } => "CAPTION",
            Kind::Group { .. } => "NAME",
            _ => "LABEL",
        };
        theme::heading(ui, heading);
        let mut label = o.label.clone();
        let resp = if matches!(o.kind, Kind::Text) {
            ui.add(egui::TextEdit::multiline(&mut label).desired_rows(3).desired_width(f32::INFINITY))
        } else {
            ui.add(egui::TextEdit::singleline(&mut label).desired_width(f32::INFINITY))
        };
        if resp.changed() {
            ed.mark_changed();
            if let Some(x) = ed.board.get_mut(first) {
                x.label = label;
            }
        }
        if let Kind::Image { image } = o.kind {
            if let Some(a) = ed.board.images.get(&image) {
                ui.label(theme::secondary(ui, format!("{} x {} pixels, shown at {:.0} x {:.0}", a.size[0], a.size[1], o.rect[2], o.rect[3])));
            }
        }
    }

    let has = |f: &dyn Fn(&Kind) -> bool| ed.selection.iter().any(|id| ed.board.get(*id).is_some_and(|o| f(&o.kind)));
    let any_shape = has(&|k| matches!(k, Kind::Shape { .. }));
    let any_group = has(&|k| matches!(k, Kind::Group { sketch: false, .. }));
    let any_stroke = has(&|k| matches!(k, Kind::Shape { .. } | Kind::Path { .. } | Kind::Group { sketch: true, .. }));
    let any_text = has(&|k| matches!(k, Kind::Shape { .. } | Kind::Text | Kind::Group { sketch: false, .. }));
    let any_path = has(&|k| matches!(k, Kind::Path { closed: false, .. }));
    // A sketch shows the look of its first stroke.
    let o_style = if o.is_sketch() { ed.board.with_descendants(&[first]).iter().filter_map(|id| ed.board.get(*id)).find(|x| !x.is_group()).map(|x| x.style).unwrap_or(o.style) } else { o.style };

    if let (true, Kind::Shape { shape }) = (single, &o.kind) {
        theme::heading(ui, "SHAPE");
        let mut s = *shape;
        if shape_combo(ui, "sel-shape", &mut s, &Shape::ALL) {
            ed.mark_changed();
            if let Some(x) = ed.board.get_mut(first) {
                x.kind = Kind::Shape { shape: s };
            }
        }
    }

    if any_shape {
        // Edit the first shape's style; whatever changes is copied to every selected item.
        let shape_obj = ed.selection.iter().filter_map(|id| ed.board.get(*id)).find(|o| matches!(o.kind, Kind::Shape { .. })).cloned();
        if let Some(so) = shape_obj {
            let corners = match so.kind {
                Kind::Shape { shape } if shapes::has_corners(shape) => Some(shape),
                _ => None,
            };
            let before = so.style;
            let mut s = before;
            let max_radius = so.rect[2].min(so.rect[3]) * 0.5;
            style_editor(ui, &mut s, corners, max_radius, &mut ed.eyedropper, true);
            if s != before {
                apply_to_selection(ed, |t| copy_changes(&before, &s, t));
            }
        }
    } else if any_stroke {
        theme::heading(ui, "LINE");
        if let Some(ink) = ink_row(ui, o_style.ink) {
            apply_to_selection(ed, |s| s.ink = ink);
        }
        let mut dash = o_style.dash;
        if dash_control(ui, &mut dash) {
            apply_to_selection(ed, |s| s.dash = dash);
        }
        let mut w = o_style.width;
        if ui.add(egui::Slider::new(&mut w, board::MIN_WIDTH..=board::MAX_WIDTH).logarithmic(true).text("Width")).changed() {
            apply_to_selection(ed, |s| s.width = w);
        }
    } else if any_text {
        theme::heading(ui, "COLOUR");
        if let Some(ink) = ink_row(ui, o.style.ink) {
            apply_to_selection(ed, |s| s.ink = ink);
        }
        if any_group {
            let mut fill = o.style.fill;
            if ui.switch(&mut fill, "Fill").changed() {
                apply_to_selection(ed, |s| s.fill = fill);
            }
        }
    }
    if let (true, Kind::Path { closed, curved, .. }) = (single, &o.kind) {
        // Polylines: a smooth curve or straight corners; a closed one can be filled.
        if *closed || *curved || matches!(o.kind, Kind::Path { ref points, .. } if points.len() > 2 && points.len() < 64) {
            let mut c = *curved;
            if ui.switch(&mut c, "Curved").changed() {
                ed.mark_changed();
                if let Some(Kind::Path { curved: x, .. }) = ed.board.get_mut(first).map(|o| &mut o.kind) {
                    *x = c;
                }
            }
        }
        if *closed {
            let mut fill = o.style.fill;
            if ui.switch(&mut fill, "Fill").changed() {
                apply_to_selection(ed, |s| s.fill = fill);
            }
        }
    }
    if any_path {
        let mut arrow = matches!(o.kind, Kind::Path { arrow: true, .. });
        if ui.switch(&mut arrow, "Arrow at the end").changed() {
            ed.mark_changed();
            for id in ed.selection.clone() {
                if let Some(Kind::Path { arrow: a, .. }) = ed.board.get_mut(id).map(|o| &mut o.kind) {
                    *a = arrow;
                }
            }
        }
    }
    if any_text {
        theme::heading(ui, "TEXT");
        let mut fs = o.style.font_size;
        if ui.add(egui::Slider::new(&mut fs, 8.0..=72.0).text("Size")).changed() {
            apply_to_selection(ed,|s| s.font_size = fs);
        }
    }

    theme::heading(ui, "VISIBILITY");
    let mut opacity = o.opacity * 100.0;
    if ui.add(egui::Slider::new(&mut opacity, 5.0..=100.0).suffix("%").text("Opacity")).changed() {
        ed.mark_changed();
        for id in ed.selection.clone() {
            if let Some(x) = ed.board.get_mut(id) {
                x.opacity = opacity / 100.0;
            }
        }
    }
    let mut locked = ed.selection.iter().all(|id| ed.is_locked(*id));
    if ui.switch(&mut locked, "Locked").on_hover_text("Ctrl+L. Locked items cannot be moved, edited, erased or deleted.").changed() {
        ed.toggle_lock();
    }
    if single {
        if let Kind::Group { collapsed, sketch: false, .. } = o.kind {
            let mut c = collapsed;
            if ui.switch(&mut c, "Collapsed").changed() {
                ed.toggle_collapsed(first);
            }
        }
    }

    if single {
        theme::heading(ui, "COMMENTS");
        let n = o.comments.len();
        ui.horizontal(|ui| {
            ui.label(theme::secondary(ui, if n == 0 { "None yet".to_owned() } else { plural(n, "comment", "comments") }));
            let open = ed.comments_open == Some(first);
            if ui.button(if open { "Close" } else if n == 0 { "Add" } else { "Open" }).clicked() {
                ed.comments_open = if open { None } else { Some(first) };
                ed.comment_draft.clear();
            }
        });
    }

    theme::heading(ui, "ARRANGE");
    ui.horizontal_wrapped(|ui| {
        if ui.button("Group").on_hover_text("Ctrl+G").clicked() {
            ed.group_selection();
        }
        let label = if single && o.is_sketch() { "Split sketch" } else { "Ungroup" };
        if ed.any_group_selected() && ui.button(label).on_hover_text("Ctrl+Shift+G").clicked() {
            ed.ungroup_selection();
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Bring to front").clicked() {
            ed.reorder_selection(true);
        }
        if ui.button("Send to back").clicked() {
            ed.reorder_selection(false);
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Duplicate").clicked() {
            ed.duplicate();
        }
        let danger = pal(ui.ctx()).danger;
        if ui.button(RichText::new("Delete").color(danger)).clicked() {
            ed.delete_selection();
        }
    });
    if any_group {
        ui.label(theme::secondary(ui, "Deleting a group deletes what is in it. Ungroup to keep the contents."));
    }
}

fn wire_props(ui: &mut Ui, ed: &mut Editor, wid: Id) {
    let Some(w) = ed.board.wire(wid).cloned() else { return };
    theme::heading(ui, "CONNECTION");
    let from = ed.board.display_name(w.from);
    let to = ed.board.display_name(w.to);
    theme::inner_card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(theme::secondary(ui, "From"));
        if ui.link(&from).clicked() {
            ed.selection = vec![w.from];
            ed.selected_wire = None;
        }
        ui.label(theme::secondary(ui, "To"));
        if ui.link(&to).clicked() {
            ed.selection = vec![w.to];
            ed.selected_wire = None;
        }
    });
    theme::heading(ui, "LABEL");
    let mut label = w.label.clone();
    if ui.add(egui::TextEdit::singleline(&mut label).hint_text("For example: yes").desired_width(f32::INFINITY)).changed() {
        ed.mark_changed();
        if let Some(x) = ed.board.wire_mut(wid) {
            x.label = label;
        }
    }
    theme::heading(ui, "LINE");
    let mut style = w.style;
    if wire_style_control(ui, &mut style) {
        ed.mark_changed();
        if let Some(x) = ed.board.wire_mut(wid) {
            x.style = style;
        }
        ed.wire_style = style;
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Reverse").clicked() {
            ed.mark_changed();
            if let Some(x) = ed.board.wire_mut(wid) {
                std::mem::swap(&mut x.from, &mut x.to);
                std::mem::swap(&mut x.from_side, &mut x.to_side);
            }
        }
        let danger = pal(ui.ctx()).danger;
        if ui.button(RichText::new("Delete").color(danger)).clicked() {
            ed.delete_selection();
        }
    });
}

/// Height of one state's row and width of one branch's lane in the history graph.
const ROW: f32 = 24.0;
const LANE: f32 = 20.0;
const DOT: f32 = 5.0;

/// A branch's colour in the history graph: the first branch follows the theme, the rest take the
/// categorical palette in order.
fn lane_color(lane: usize, p: &crate::theme::Pal) -> Color32 {
    if lane == 0 {
        p.text_secondary
    } else {
        let [r, g, b] = crate::palettes::cat_color(lane - 1);
        Color32::from_rgb(r, g, b)
    }
}

/// The History tab: every state as a dot, every branch as a lane, newest at the top.
/// Something selected in the History graph.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HistSel {
    State(u32),
    Branch(u32),
}

/// What the History card is asking you to confirm, for the current selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HistAction {
    Delete,
    KeepOnly,
}

/// Click to select; Ctrl or Shift + click to select more.
fn toggle_select(sels: &mut Vec<HistSel>, item: HistSel, add: bool) {
    if add {
        if let Some(i) = sels.iter().position(|s| *s == item) {
            sels.remove(i);
        } else {
            sels.push(item);
        }
    } else {
        *sels = vec![item];
    }
}

/// Hover a dot to see what it is; click to select it (Ctrl-click for more), double-click to go back to it.
pub fn history_ui(ui: &mut Ui, ed: &mut Editor) {
    use crate::history::short_time;
    let p = pal(ui.ctx());
    let cur = ed.history.current;
    let cur_branch = ed.history.current().branch;
    let n_states = ed.history.states.len();
    let n_branches = ed.history.branches.len();
    let mods = ui.input(|i| i.modifiers);
    let adding = mods.command || mods.shift;

    let sel_id = ui.id().with("history-selection");
    let confirm_id = ui.id().with("history-confirm");
    let mut sels: Vec<HistSel> = ui.data(|d| d.get_temp(sel_id)).unwrap_or_default();
    let mut confirm: Option<HistAction> = ui.data(|d| d.get_temp(confirm_id));
    // Forget anything that has gone.
    sels.retain(|s| match s {
        HistSel::State(id) => ed.history.state(*id).is_some(),
        HistSel::Branch(id) => ed.history.branch(*id).is_some(),
    });
    if sels.is_empty() {
        confirm = None;
    }
    let before = (sels.clone(), confirm);

    ui.label(theme::secondary(ui, format!("{}, {}. Click a dot or a branch name to select it, Ctrl-click to select more; double-click a dot to go back to it.", plural(n_states, "state", "states"), plural(n_branches, "branch", "branches"))));
    ui.add_space(4.0);

    let mut jump = None;
    let mut run: Option<HistAction> = None;

    let sel_states: Vec<u32> = sels.iter().filter_map(|s| if let HistSel::State(id) = s { Some(*id) } else { None }).collect();
    let sel_branches: Vec<u32> = sels.iter().filter_map(|s| if let HistSel::Branch(id) = s { Some(*id) } else { None }).collect();

    // The card for whatever is selected (or the branch you are on).
    theme::inner_card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match sels.as_slice() {
            [] => {
                ui.label(theme::secondary(ui, "You are on"));
                branch_info(ui, ed, cur_branch, &mut jump);
            }
            [HistSel::State(id)] => {
                let s = ed.history.state(*id).unwrap();
                let here = *id == cur;
                let branch_name = ed.history.branch(s.branch).map(|b| b.name.clone()).unwrap_or_default();
                ui.label(theme::semibold(&s.label));
                ui.label(theme::secondary(ui, format!("{}  ·  {}{}", short_time(&s.time), branch_name, if here { "  ·  you are here" } else { "" })));
                if ui.add_enabled(!here, egui::Button::new("Go to this state")).clicked() {
                    jump = Some(*id);
                }
            }
            [HistSel::Branch(id)] => branch_info(ui, ed, *id, &mut jump),
            _ => {
                ui.label(theme::semibold(format!("{} and {} selected", plural(sel_states.len(), "state", "states"), plural(sel_branches.len(), "branch", "branches"))));
            }
        }
        if sels.is_empty() {
            ui.label(theme::secondary(ui, "Select states or branches to delete them, or to keep only them."));
            return;
        }
        ui.add_space(2.0);
        let deletable = sel_states.iter().any(|id| ed.history.state(*id).is_some_and(|s| s.parent.is_some())) || sel_branches.iter().any(|b| *b != 0);
        match confirm {
            Some(action) => {
                let what = match (sels.as_slice(), action) {
                    ([HistSel::State(_)], HistAction::Delete) => "Delete this state? The states before and after it will be joined.".to_owned(),
                    ([HistSel::Branch(b)], HistAction::Delete) => {
                        let n = ed.history.states.iter().filter(|s| s.branch == *b).count();
                        let name = ed.history.branch(*b).map(|x| x.name.clone()).unwrap_or_default();
                        format!("Delete {name} and its {}? Branches that grew from it are kept.", plural(n, "state", "states"))
                    }
                    (_, HistAction::Delete) => format!("Delete {} and {}? States around deleted states are joined; branches that grew from deleted branches are kept.", plural(sel_states.len(), "state", "states"), plural(sel_branches.len(), "branch", "branches")),
                    (_, HistAction::KeepOnly) => "Keep only the selection? Everything else is deleted, except the states needed to reach it from the start.".to_owned(),
                };
                ui.label(RichText::new(format!("{what} This can't be undone.")).color(p.danger));
                ui.horizontal(|ui| {
                    let label = if action == HistAction::Delete { "Delete" } else { "Keep only these" };
                    if ui.add(egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(p.danger)).clicked() {
                        run = Some(action);
                    }
                    if ui.button("Cancel").clicked() {
                        confirm = None;
                    }
                });
            }
            None => {
                ui.horizontal_wrapped(|ui| {
                    let tip = "The first state and the first branch cannot be deleted";
                    if ui.add_enabled(deletable, egui::Button::new(RichText::new("Delete").color(if deletable { p.danger } else { p.text_secondary }))).on_disabled_hover_text(tip).clicked() {
                        confirm = Some(HistAction::Delete);
                    }
                    let keep = if sels.len() == 1 { "Keep only this" } else { "Keep only these" };
                    if ui.button(keep).on_hover_text("Delete everything else, except the states needed to reach the selection").clicked() {
                        confirm = Some(HistAction::KeepOnly);
                    }
                });
            }
        }
    });
    ui.add_space(6.0);

    // Rows newest first; one lane per branch, in the order the branches were made.
    let mut rows: Vec<&crate::history::State> = ed.history.states.iter().collect();
    rows.sort_by(|a, b| b.id.cmp(&a.id));
    let lane_of: std::collections::HashMap<u32, usize> = ed.history.branches.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    let row_of: std::collections::HashMap<u32, usize> = rows.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
    let heads: std::collections::HashMap<u32, u32> = ed.history.branches.iter().filter_map(|b| ed.history.head(b.id).map(|h| (h, b.id))).collect();
    let lanes_w = n_branches as f32 * LANE + 12.0;

    ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        let width = (lanes_w + 150.0).max(ui.available_width());
        let (area, _) = ui.allocate_exact_size(vec2(width, rows.len() as f32 * ROW + 8.0), Sense::hover());
        let painter = ui.painter_at(area);
        let at = |id: u32| -> Option<Pos2> {
            let row = *row_of.get(&id)?;
            let lane = *lane_of.get(&ed.history.state(id)?.branch)?;
            Some(pos2(area.left() + 12.0 + lane as f32 * LANE, area.top() + 4.0 + ROW * (row as f32 + 0.5)))
        };
        let visible = ui.clip_rect().expand(ROW * 2.0);

        // Lines first, under the dots. A child is always above its parent.
        for s in &rows {
            let (Some(parent), Some(c)) = (s.parent, at(s.id)) else { continue };
            let Some(pp) = at(parent) else { continue };
            if !visible.intersects(Rect::from_two_pos(c, pp)) {
                continue;
            }
            let color = lane_color(lane_of.get(&s.branch).copied().unwrap_or(0), p);
            let stroke = Stroke::new(if sel_branches.contains(&s.branch) { 3.5 } else { 2.0 }, color);
            if (c.x - pp.x).abs() < 0.5 {
                painter.line_segment([pp, c], stroke);
            } else {
                // A branch splits off just above where it grew from, then runs up its own lane.
                let bend = pos2(c.x, pp.y - ROW);
                let curve = crate::board::sample_curve(&[pp, pos2(pp.x, pp.y - ROW * 0.6), pos2(c.x, pp.y - ROW * 0.4), bend], 16);
                painter.add(egui::Shape::line(curve, stroke));
                if bend.y > c.y {
                    painter.line_segment([bend, c], stroke);
                }
            }
        }

        // Dots, branch names at each branch's newest state, and hover details.
        let saved_delay = ui.style().interaction.tooltip_delay;
        ui.style_mut().interaction.tooltip_delay = 0.0;
        for s in &rows {
            let Some(c) = at(s.id) else { continue };
            if !visible.contains(c) {
                continue;
            }
            let lane = lane_of.get(&s.branch).copied().unwrap_or(0);
            let color = lane_color(lane, p);
            let resp = ui.interact(Rect::from_center_size(c, vec2(ROW, ROW)), ui.id().with(("state", s.id)), Sense::click());
            let here = s.id == cur;
            if sel_states.contains(&s.id) {
                // The selected state: its whole row is highlighted.
                let row = Rect::from_min_max(pos2(area.left(), c.y - ROW * 0.5), pos2(area.right(), c.y + ROW * 0.5));
                painter.rect_filled(row, CornerRadius::same(6), p.bg_control.gamma_multiply(0.8));
            }
            if here {
                // Where you are: a ring in the accent colour.
                painter.circle_stroke(c, DOT + 3.5, Stroke::new(2.0, p.accent));
            }
            let r = if resp.hovered() { DOT + 1.5 } else { DOT };
            painter.circle(c, r, if here || resp.hovered() { color } else { p.bg_elevated }, Stroke::new(2.0, color));

            if let Some(bid) = heads.get(&s.id) {
                if let Some(b) = ed.history.branch(*bid) {
                    let g = painter.layout_no_wrap(b.name.clone(), egui::FontId::new(11.5, theme::semibold_family()), if b.id == cur_branch { p.text_strong } else { color });
                    let chip = Rect::from_min_size(pos2(area.left() + lanes_w + 4.0, c.y - g.size().y * 0.5 - 3.0), g.size() + vec2(14.0, 6.0));
                    // Clicking a branch's name selects the branch.
                    let chip_resp = ui.interact(chip, ui.id().with(("branch", b.id)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let picked = sel_branches.contains(&b.id);
                    let edge = if picked { Stroke::new(2.0, p.text_strong) } else { Stroke::new(1.0, color.gamma_multiply(if chip_resp.hovered() { 1.0 } else { 0.6 })) };
                    painter.rect(chip, CornerRadius::same(9), color.gamma_multiply(if picked { 0.3 } else { 0.16 }), edge, egui::StrokeKind::Inside);
                    painter.galley(chip.min + vec2(7.0, 3.0), g, color);
                    // A thin guide from the dot to its name.
                    painter.line_segment([pos2(c.x + DOT + 3.0, c.y), pos2(chip.left() - 2.0, c.y)], Stroke::new(1.0, color.gamma_multiply(0.35)));
                    if chip_resp.clicked() {
                        toggle_select(&mut sels, HistSel::Branch(b.id), adding);
                    }
                }
            }

            let branch_name = ed.history.branch(s.branch).map(|b| b.name.clone()).unwrap_or_default();
            let forks: Vec<String> = ed.history.branches.iter().filter(|b| b.from == Some(s.id)).map(|b| b.name.clone()).collect();
            let resp = resp.on_hover_ui(|ui| {
                ui.label(theme::semibold(&s.label));
                ui.label(theme::secondary(ui, format!("{}  ·  {}", short_time(&s.time), branch_name)));
                if !forks.is_empty() {
                    ui.label(theme::secondary(ui, format!("{} grew from here", forks.join(", "))));
                }
                if here {
                    ui.label(egui::RichText::new("You are here").color(p.accent_text));
                }
            });
            if resp.clicked() {
                toggle_select(&mut sels, HistSel::State(s.id), adding);
            }
            if resp.double_clicked() && !here {
                jump = Some(s.id);
            }
            let first = s.parent.is_none();
            resp.context_menu(|ui| {
                if ui.add_enabled(!here, egui::Button::new("Go to this state")).clicked() {
                    jump = Some(s.id);
                    ui.close();
                }
                if ui.button(format!("Go to the latest on {branch_name}")).clicked() {
                    jump = ed.history.head(s.branch);
                    ui.close();
                }
                ui.separator();
                if ui.add_enabled(!first, egui::Button::new(RichText::new("Delete this state…").color(p.danger))).clicked() {
                    sels = vec![HistSel::State(s.id)];
                    confirm = Some(HistAction::Delete);
                    ui.close();
                }
                if ui.add_enabled(s.branch != 0, egui::Button::new(RichText::new(format!("Delete {branch_name}…")).color(p.danger))).clicked() {
                    sels = vec![HistSel::Branch(s.branch)];
                    confirm = Some(HistAction::Delete);
                    ui.close();
                }
                if ui.button(format!("Keep only {branch_name}…")).clicked() {
                    sels = vec![HistSel::Branch(s.branch)];
                    confirm = Some(HistAction::KeepOnly);
                    ui.close();
                }
            });
        }
        ui.style_mut().interaction.tooltip_delay = saved_delay;
    });

    // Changing the selection cancels a question about the old one.
    if sels != before.0 && confirm == before.1 {
        confirm = None;
    }

    if let Some(id) = jump {
        ed.jump_to_state(id);
    }
    match run {
        Some(HistAction::Delete) => {
            ed.delete_history_selection(&sel_states, &sel_branches);
            sels.clear();
            confirm = None;
        }
        Some(HistAction::KeepOnly) => {
            ed.keep_history_only(&sel_states, &sel_branches);
            sels.clear();
            confirm = None;
        }
        None => {}
    }
    ui.data_mut(|d| {
        d.insert_temp(sel_id, sels);
        if let Some(c) = confirm {
            d.insert_temp(confirm_id, c);
        } else {
            d.remove::<HistAction>(confirm_id);
        }
    });
}

/// A branch in the History card: rename it, see where it came from, go to its latest state.
fn branch_info(ui: &mut Ui, ed: &mut Editor, id: u32, jump: &mut Option<u32>) {
    use crate::history::short_time;
    let p = pal(ui.ctx());
    let lane = ed.history.branches.iter().position(|b| b.id == id).unwrap_or(0);
    let n = ed.history.states.iter().filter(|s| s.branch == id).count();
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 4.5, lane_color(lane, p));
        if let Some(b) = ed.history.branch_mut(id) {
            let mut name = b.name.clone();
            if ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY)).on_hover_text("Branch name").changed() {
                b.name = name;
                ed.dirty = true;
            }
        }
    });
    let origin = ed.history.branch(id).and_then(|b| b.from).and_then(|f| ed.history.state(f)).map(|s| {
        let parent = ed.history.branch(s.branch).map(|b| b.name.clone()).unwrap_or_default();
        format!("grew from {parent} at {}", short_time(&s.time))
    });
    ui.label(theme::secondary(ui, match origin {
        Some(o) => format!("{}, {o}", plural(n, "state", "states")),
        None => format!("{}, the first branch", plural(n, "state", "states")),
    }));
    let head = ed.history.head(id);
    if ui.add_enabled(head.is_some_and(|h| h != ed.history.current), egui::Button::new("Go to latest")).clicked() {
        *jump = head;
    }
}

/// The comment thread of one item, in a small floating window next to its bubble.
pub fn comments_window(ctx: &egui::Context, ed: &mut Editor) {
    let Some(id) = ed.comments_open else { return };
    let Some(o) = ed.board.get(id).cloned() else {
        ed.comments_open = None;
        return;
    };
    let p = pal(ctx);
    let canvas = ed.canvas_rect;
    let anchor = ed.camera.to_screen(canvas.min, o.rect().right_top()) + vec2(24.0, -10.0);
    let mut open = true;
    let title = format!("Comments · {}", ed.board.display_name(id));
    egui::Window::new(theme::semibold(title).size(12.0))
        .id(egui::Id::new("comments-window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(280.0)
        .current_pos(anchor)
        .constrain_to(canvas)
        .show(ctx, |ui| {
            ui.set_width(280.0);
            let mut remove = None;
            if o.comments.is_empty() {
                ui.label(theme::secondary(ui, "No comments yet."));
            }
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for (k, c) in o.comments.iter().enumerate() {
                    theme::inner_card(ui).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(&c.text);
                        ui.horizontal(|ui| {
                            ui.label(theme::secondary(ui, crate::history::short_time(&c.time)).small());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new("Delete").small().color(p.text_secondary)).frame(false)).clicked() {
                                    remove = Some(k);
                                }
                            });
                        });
                    });
                }
            });
            ui.add_space(4.0);
            let edit = ui.add(egui::TextEdit::multiline(&mut ed.comment_draft).hint_text("Add a comment").desired_rows(2).desired_width(f32::INFINITY));
            let send_key = edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
            ui.horizontal(|ui| {
                ui.label(theme::secondary(ui, "Ctrl+Enter to post").small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ready = !ed.comment_draft.trim().is_empty();
                    if (ui.add_enabled(ready, egui::Button::new("Comment")).clicked() || send_key) && ready {
                        let text = std::mem::take(&mut ed.comment_draft);
                        ed.add_comment(id, &text);
                    }
                });
            });
            if let Some(k) = remove {
                ed.delete_comment(id, k);
            }
        });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !ctx.egui_wants_keyboard_input() {
        ed.comments_open = None;
    }
}

enum LayerAction {
    Activate(Id),
    Rename(Id, String),
    Up(usize),
    Down(usize),
    MoveHere(Id),
    Delete(Id),
}

/// Layers the user makes. The top of the list is in front; new items go on the highlighted layer.
pub fn layers_ui(ui: &mut Ui, ed: &mut Editor) {
    let p = pal(ui.ctx());
    let rename_id = ui.id().with("layer-rename");
    let mut renaming: Option<(Id, String)> = ui.data(|d| d.get_temp(rename_id));
    let mut action = None;
    ui.horizontal(|ui| {
        if ui.button("New layer").on_hover_text("Adds a layer above the current one").clicked() {
            let id = ed.board.new_id();
            let n = (1..).find(|n| !ed.board.layers.iter().any(|l| l.name == format!("Layer {n}"))).unwrap_or(1);
            let at = ed.board.layers.iter().position(|l| l.id == ed.active_layer).map(|i| i + 1).unwrap_or(ed.board.layers.len());
            ed.mark_changed();
            ed.board.layers.insert(at, board::Layer { id, name: format!("Layer {n}") });
            ed.active_layer = id;
            renaming = Some((id, format!("Layer {n}")));
        }
    });
    ui.label(theme::secondary(ui, "Top is in front. New items go on the highlighted layer. Double-click a name to rename it."));
    ui.add_space(4.0);
    let layers = ed.board.layers.clone();
    let n = layers.len();
    let has_sel = !ed.selection.is_empty();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, l) in layers.iter().enumerate().rev() {
            let active = l.id == ed.active_layer;
            let count = ed.board.objects.iter().filter(|o| o.layer == l.id && !o.is_group()).count();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                let small = |ui: &mut Ui, text: &str, enabled: bool, tip: &str| ui.add_enabled(enabled, egui::Button::new(RichText::new(text).size(11.0)).frame(false).min_size(vec2(18.0, 22.0))).on_hover_text(tip).clicked();
                if small(ui, "▲", i + 1 < n, "Move the layer up (in front)") {
                    action = Some(LayerAction::Up(i));
                }
                if small(ui, "▼", i > 0, "Move the layer down (behind)") {
                    action = Some(LayerAction::Down(i));
                }
                match &mut renaming {
                    Some((rid, text)) if *rid == l.id => {
                        let r = ui.add(egui::TextEdit::singleline(text).desired_width((ui.available_width() - 4.0).max(40.0)));
                        r.request_focus();
                        if r.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            let name = text.trim().to_owned();
                            if !name.is_empty() && name != l.name {
                                action = Some(LayerAction::Rename(l.id, name));
                            }
                            renaming = None;
                        }
                    }
                    _ => {
                        let right = if has_sel { 40.0 } else { 0.0 } + if n > 1 { 22.0 } else { 0.0 } + 28.0;
                        let w = (ui.available_width() - right).max(40.0);
                        let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
                        let bg = if active { p.accent.gamma_multiply(0.22) } else if resp.hovered() { p.bg_control_hover } else { Color32::TRANSPARENT };
                        ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
                        if active {
                            ui.painter().rect_filled(Rect::from_min_size(rect.min + vec2(0.0, 5.0), vec2(3.0, rect.height() - 10.0)), CornerRadius::same(2), p.accent);
                        }
                        let font = egui::FontId::proportional(13.0);
                        let color = if active { p.text } else { p.text_secondary };
                        ui.painter().text(pos2(rect.left() + 10.0, rect.center().y), egui::Align2::LEFT_CENTER, short_label(&l.name), font, color);
                        let resp = resp.on_hover_text(if active { "New items go on this layer" } else { "Click to put new items on this layer" });
                        if resp.double_clicked() {
                            renaming = Some((l.id, l.name.clone()));
                        } else if resp.clicked() {
                            action = Some(LayerAction::Activate(l.id));
                        }
                        ui.add_sized([28.0, 22.0], egui::Label::new(RichText::new(count.to_string()).size(11.0).color(p.text_secondary))).on_hover_text(plural(count, "item", "items"));
                        if has_sel && ui.add(egui::Button::new(RichText::new("Move").size(11.0)).min_size(vec2(36.0, 20.0))).on_hover_text("Move the selected items to this layer").clicked() {
                            action = Some(LayerAction::MoveHere(l.id));
                        }
                        if n > 1 && small(ui, "×", true, if count == 0 { "Delete the layer" } else { "Delete the layer and the items on it" }) {
                            action = Some(LayerAction::Delete(l.id));
                        }
                    }
                }
            });
        }
    });
    ui.data_mut(|d| match &renaming {
        Some(r) => {
            d.insert_temp(rename_id, r.clone());
        }
        None => d.remove::<(Id, String)>(rename_id),
    });
    let Some(action) = action else { return };
    match action {
        LayerAction::Activate(id) => ed.active_layer = id,
        LayerAction::Rename(id, name) => {
            ed.mark_changed();
            if let Some(l) = ed.board.layers.iter_mut().find(|l| l.id == id) {
                l.name = name;
            }
        }
        LayerAction::Up(i) => {
            ed.mark_changed();
            ed.board.layers.swap(i, i + 1);
        }
        LayerAction::Down(i) => {
            ed.mark_changed();
            ed.board.layers.swap(i, i - 1);
        }
        LayerAction::MoveHere(id) => {
            ed.mark_changed();
            let sel = ed.selection.clone();
            ed.board.set_layer(&sel, id);
            ed.say(format!("Moved {} to {}", plural(sel.len(), "item", "items"), ed.board.layers.iter().find(|l| l.id == id).map(|l| l.name.clone()).unwrap_or_default()));
        }
        LayerAction::Delete(id) => {
            ed.mark_changed();
            let on: Vec<Id> = ed.board.objects.iter().filter(|o| o.layer == id).map(|o| o.id).collect();
            let all = ed.board.with_descendants(&on);
            ed.board.remove(&all);
            ed.board.layers.retain(|l| l.id != id);
            let b = &ed.board;
            ed.selection.retain(|s| b.get(*s).is_some());
        }
    }
}

fn short_label(s: &str) -> String {
    let line = s.lines().next().unwrap_or_default();
    if line.chars().count() > 28 { format!("{}…", line.chars().take(27).collect::<String>()) } else { line.to_owned() }
}

/// True if following wires can lead back to where it started.
fn has_cycle(b: &board::Board) -> bool {
    use std::collections::HashMap;
    let mut indeg: HashMap<Id, usize> = b.objects.iter().map(|o| (o.id, 0)).collect();
    for w in &b.wires {
        *indeg.entry(w.to).or_default() += 1;
    }
    let mut queue: Vec<Id> = indeg.iter().filter(|(_, d)| **d == 0).map(|(id, _)| *id).collect();
    let mut seen = 0;
    while let Some(id) = queue.pop() {
        seen += 1;
        for w in b.wires.iter().filter(|w| w.from == id) {
            let d = indeg.get_mut(&w.to).unwrap();
            *d -= 1;
            if *d == 0 {
                queue.push(w.to);
            }
        }
    }
    seen < indeg.len()
}

pub fn graph_ui(ui: &mut Ui, ed: &mut Editor) {
    let b = &ed.board;
    let n = b.objects.len();
    let m = b.wires.len();
    ui.label(theme::secondary(ui, format!("{}, {}", plural(n, "object", "objects"), plural(m, "connection", "connections"))));
    if m > 0 {
        let sources = b.objects.iter().filter(|o| b.inputs(o.id).next().is_none() && b.outputs(o.id).next().is_some()).count();
        let cyc = if has_cycle(b) { "has a loop" } else { "no loops" };
        ui.label(theme::secondary(ui, format!("{}, {cyc}", plural(sources, "start point", "start points"))));
    }
    let mut select: Option<Id> = None;
    let mut select_wire: Option<Id> = None;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if ed.selection.len() == 1 {
            let id = ed.selection[0];
            theme::heading(ui, "INCOMING");
            let ins: Vec<Id> = b.inputs(id).collect();
            if ins.is_empty() {
                ui.label(theme::secondary(ui, "None. Drag from another item's port to this one."));
            }
            for i in ins {
                if ui.link(b.display_name(i)).clicked() {
                    select = Some(i);
                }
            }
            theme::heading(ui, "OUTGOING");
            let outs: Vec<Id> = b.outputs(id).collect();
            if outs.is_empty() {
                ui.label(theme::secondary(ui, "None"));
            }
            for o in outs {
                if ui.link(b.display_name(o)).clicked() {
                    select = Some(o);
                }
            }
        }
        theme::heading(ui, "ALL CONNECTIONS");
        if m == 0 {
            ui.label(theme::secondary(ui, "No connections yet. Drag from any port, or use the Connect tool (C)."));
        }
        for w in &b.wires {
            let mut text = format!("{}  →  {}", short_label(&b.display_name(w.from)), short_label(&b.display_name(w.to)));
            if !w.label.is_empty() {
                text.push_str(&format!("  ({})", short_label(&w.label)));
            }
            let sel = ed.selected_wire == Some(w.id);
            if ui.add_sized([ui.available_width(), 22.0], egui::Button::selectable(sel, text).frame_when_inactive(false)).clicked() {
                select_wire = Some(w.id);
            }
        }
    });
    if let Some(id) = select {
        ed.selection = vec![id];
        ed.selected_wire = None;
    }
    if let Some(w) = select_wire {
        ed.selection.clear();
        ed.selected_wire = Some(w);
    }
}
