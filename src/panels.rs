//! The dockable panels: Properties, Layers and Graph. Tools are in the radial menu.

use crate::board::{self, Dash, Id, Ink, Kind, Shape, Style, WireStyle};
use crate::editor::{Editor, Pick, plural};
use crate::shapes;
use crate::theme::{self, UiExt, pal};
use crate::tools::Tool;
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
    ui.add(egui::Slider::new(&mut ed.style.font_size, 6.0..=400.0).logarithmic(true).max_decimals(0).text("Size"));
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
        if let Kind::Image { image, .. } = o.kind {
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
        if ui.add(egui::Slider::new(&mut fs, 6.0..=400.0).logarithmic(true).max_decimals(0).text("Size")).changed() {
            apply_to_selection(ed,|s| s.font_size = fs);
        }
    }

    if single && !matches!(o.kind, Kind::Group { .. } | Kind::Frame { .. }) {
        theme::heading(ui, "ROTATION");
        ui.horizontal(|ui| {
            let mut deg = o.rotation.to_degrees();
            if ui.add(egui::DragValue::new(&mut deg).range(-180.0..=180.0).speed(1.0).suffix("°")).changed() {
                ed.mark_changed();
                if let Some(x) = ed.board.get_mut(first) {
                    x.rotation = deg.to_radians();
                }
            }
            if ui.add_enabled(o.rotation != 0.0, egui::Button::new("Reset")).clicked() {
                ed.mark_changed();
                if let Some(x) = ed.board.get_mut(first) {
                    x.rotation = 0.0;
                }
            }
        });
        ui.label(theme::secondary(ui, "Or drag the round handle above the item; Shift keeps 15° steps."));
    }
    if let (true, Kind::Image { crop, .. }) = (single, &o.kind) {
        theme::heading(ui, "CROP");
        ui.horizontal(|ui| {
            let cropping = ed.cropping == Some(first);
            if ui.add_enabled(!cropping, egui::Button::new("Crop")).on_hover_text("Or double-click the image").clicked() {
                ed.start_crop(first);
            }
            if cropping && ui.button("Done").clicked() {
                ed.cropping = None;
            }
            if ui.add_enabled(crop.is_some(), egui::Button::new("Reset crop")).on_hover_text("Show the whole picture again").clicked() {
                ed.mark_changed();
                if let Some(x) = ed.board.get_mut(first) {
                    let full = x.image_full_rect();
                    x.set_rect(full);
                    if let Kind::Image { crop, .. } = &mut x.kind {
                        *crop = None;
                    }
                }
            }
        });
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
    theme::heading(ui, "COLOUR");
    if let Some(ink) = ink_row(ui, w.color.unwrap_or(Ink::Auto)) {
        ed.mark_changed();
        if let Some(x) = ed.board.wire_mut(wid) {
            x.color = if ink == Ink::Auto { None } else { Some(ink) };
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
    Toggle(Id),
    Hide(Id),
    Lock(Id),
    Color(Id, Option<Ink>),
    SelectAll(Id),
    /// Items dropped on a layer.
    Drop(Id, Id),
    GoTo(Id),
}

/// An item being dragged from one layer's list to another layer.
#[derive(Clone, Copy)]
struct LayerItem(Id);

/// A small line icon in the layer rows: an eye (shown) or a padlock.
fn layer_icon(ui: &mut Ui, kind: &str, on: bool, tip: &str) -> bool {
    let p = pal(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(20.0, 22.0), Sense::click());
    let c = if resp.hovered() { p.accent } else if on { p.text } else { p.text_secondary.gamma_multiply(0.6) };
    let m = r.center();
    let st = Stroke::new(1.3, c);
    match kind {
        "eye" => {
            let pts = |s: f32| (0..=12).map(move |k| { let t = k as f32 / 12.0; m + vec2(-6.5 + 13.0 * t, s * 4.0 * (t * std::f32::consts::PI).sin()) }).collect::<Vec<_>>();
            ui.painter().add(egui::Shape::line(pts(-1.0), st));
            ui.painter().add(egui::Shape::line(pts(1.0), st));
            ui.painter().circle_filled(m, 1.8, c);
            if !on {
                ui.painter().line_segment([m + vec2(-6.0, 5.0), m + vec2(6.0, -5.0)], st);
            }
        }
        _ => {
            let body = Rect::from_center_size(m + vec2(0.0, 2.5), vec2(10.0, 7.0));
            ui.painter().rect_stroke(body, 1.5, st, egui::StrokeKind::Middle);
            let lift = if on { 0.0 } else { -2.5 };
            ui.painter().add(egui::Shape::line((0..=10).map(|k| { let a = std::f32::consts::PI * (1.0 + k as f32 / 10.0); m + vec2(3.0 * a.cos(), -1.0 + lift + 3.5 * a.sin()) }).collect(), st));
        }
    }
    resp.on_hover_text(tip).clicked()
}

/// Layers the user makes. The top of the list is in front; new items go on the highlighted layer.
/// Each layer can be opened to list its items, hidden, locked, given a colour, and right-clicked for
/// more; items can be dragged from one layer's list onto another layer.
pub fn layers_ui(ui: &mut Ui, ed: &mut Editor) {
    let p = pal(ui.ctx());
    let rename_id = ui.id().with("layer-rename");
    let mut renaming: Option<(Id, String)> = ui.data(|d| d.get_temp(rename_id));
    let mut action = None;
    ui.horizontal(|ui| {
        if ui.button("New layer").on_hover_text("Adds a layer above the current one").clicked() {
            renaming = Some(ed.add_layer());
        }
        if !ed.selection.is_empty() {
            ui.label(theme::secondary(ui, "Drag items, or right-click a layer"));
        }
    });
    ui.label(theme::secondary(ui, "Top is in front; new items go on the highlighted layer. Items without a colour of their own take their layer's colour."));
    ui.add_space(4.0);
    let layers = ed.board.layers.clone();
    let n = layers.len();
    let has_sel = !ed.selection.is_empty();
    // Items inside groups are listed with their group, not on their own.
    let inner: std::collections::HashSet<Id> = ed.board.objects.iter().filter_map(|o| if let Kind::Group { members, .. } = &o.kind { Some(members.clone()) } else { None }).flatten().collect();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, l) in layers.iter().enumerate().rev() {
            let active = l.id == ed.active_layer;
            let open = ed.layers_open.contains(&l.id);
            let count = ed.board.objects.iter().filter(|o| o.layer == l.id && !o.is_group()).count();
            let row = ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                // Open or close the list of its items.
                let (ar, aresp) = ui.allocate_exact_size(vec2(16.0, 22.0), Sense::click());
                let m = ar.center();
                let pts = if open { vec![m + vec2(-4.0, -2.0), m + vec2(0.0, 2.0), m + vec2(4.0, -2.0)] } else { vec![m + vec2(-2.0, -4.0), m + vec2(2.0, 0.0), m + vec2(-2.0, 4.0)] };
                ui.painter().add(egui::Shape::line(pts, Stroke::new(1.5, if aresp.hovered() { p.accent } else { p.text_secondary })));
                if aresp.on_hover_text(if open { "Hide its items" } else { "List its items" }).clicked() {
                    action = Some(LayerAction::Toggle(l.id));
                }
                if layer_icon(ui, "eye", !l.hidden, if l.hidden { "Show the layer" } else { "Hide the layer" }) {
                    action = Some(LayerAction::Hide(l.id));
                }
                if layer_icon(ui, "lock", l.locked, if l.locked { "Unlock the layer" } else { "Lock the layer" }) {
                    action = Some(LayerAction::Lock(l.id));
                }
                // The layer's colour, picked from a small menu.
                let dot = l.color.map(|c| c.color(p)).unwrap_or(Color32::TRANSPARENT);
                let (dr, dresp) = ui.allocate_exact_size(vec2(18.0, 22.0), Sense::click());
                ui.painter().circle(dr.center(), 5.5, dot, Stroke::new(1.0, if dresp.hovered() { p.accent } else { p.hairline }));
                egui::Popup::menu(&dresp.on_hover_text("The layer's colour")).show(|ui| {
                    ui.label(theme::secondary(ui, "Colour for its items that have none of their own"));
                    if let Some(ink) = ink_row(ui, l.color.unwrap_or(Ink::Auto)) {
                        action = Some(LayerAction::Color(l.id, if ink == Ink::Auto { None } else { Some(ink) }));
                    }
                });
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
                        None
                    }
                    _ => {
                        let w = (ui.available_width() - 30.0).max(40.0);
                        let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
                        let dropping = resp.dnd_hover_payload::<LayerItem>().is_some();
                        let bg = if dropping { p.accent.gamma_multiply(0.3) } else if active { p.accent.gamma_multiply(0.22) } else if resp.hovered() { p.bg_control_hover } else { Color32::TRANSPARENT };
                        ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
                        if active {
                            ui.painter().rect_filled(Rect::from_min_size(rect.min + vec2(0.0, 5.0), vec2(3.0, rect.height() - 10.0)), CornerRadius::same(2), p.accent);
                        }
                        let color = if l.hidden { p.text_secondary.gamma_multiply(0.6) } else if active { p.text } else { p.text_secondary };
                        ui.painter().text(pos2(rect.left() + 10.0, rect.center().y), egui::Align2::LEFT_CENTER, short_label(&l.name), egui::FontId::proportional(13.0), color);
                        ui.add_sized([26.0, 22.0], egui::Label::new(RichText::new(count.to_string()).size(11.0).color(p.text_secondary))).on_hover_text(plural(count, "item", "items"));
                        if let Some(item) = resp.dnd_release_payload::<LayerItem>() {
                            action = Some(LayerAction::Drop(item.0, l.id));
                        }
                        let resp = resp.on_hover_text(if active { "New items go on this layer. Right-click for more." } else { "Click to put new items on this layer. Right-click for more." });
                        if resp.double_clicked() {
                            renaming = Some((l.id, l.name.clone()));
                        } else if resp.clicked() {
                            action = Some(LayerAction::Activate(l.id));
                        }
                        Some(resp)
                    }
                }
            });
            // Right-click: everything a layer can do.
            if let Some(resp) = row.inner {
                resp.context_menu(|ui| {
                    if ui.button("Select its items").clicked() {
                        action = Some(LayerAction::SelectAll(l.id));
                    }
                    if ui.add_enabled(!active, egui::Button::new("Make it the current layer")).clicked() {
                        action = Some(LayerAction::Activate(l.id));
                    }
                    if ui.add_enabled(has_sel, egui::Button::new("Move the selection here")).clicked() {
                        action = Some(LayerAction::MoveHere(l.id));
                    }
                    ui.separator();
                    if ui.button(if l.hidden { "Show" } else { "Hide" }).clicked() {
                        action = Some(LayerAction::Hide(l.id));
                    }
                    if ui.button(if l.locked { "Unlock" } else { "Lock" }).clicked() {
                        action = Some(LayerAction::Lock(l.id));
                    }
                    ui.menu_button("Colour", |ui| {
                        if let Some(ink) = ink_row(ui, l.color.unwrap_or(Ink::Auto)) {
                            action = Some(LayerAction::Color(l.id, if ink == Ink::Auto { None } else { Some(ink) }));
                        }
                    });
                    ui.separator();
                    if ui.button("Rename").clicked() {
                        renaming = Some((l.id, l.name.clone()));
                    }
                    if ui.add_enabled(i + 1 < n, egui::Button::new("Move up (in front)")).clicked() {
                        action = Some(LayerAction::Up(i));
                    }
                    if ui.add_enabled(i > 0, egui::Button::new("Move down (behind)")).clicked() {
                        action = Some(LayerAction::Down(i));
                    }
                    ui.separator();
                    let label = if count == 0 { "Delete the layer" } else { "Delete the layer and its items" };
                    if ui.add_enabled(n > 1, egui::Button::new(RichText::new(label).color(p.danger))).clicked() {
                        action = Some(LayerAction::Delete(l.id));
                    }
                });
            }
            // Its items, when open: click one to go to it, drag one onto another layer.
            if open {
                let items: Vec<(Id, String, &'static str)> = ed.board.objects.iter().rev().filter(|o| o.layer == l.id && o.page == ed.page && !inner.contains(&o.id)).take(300).map(|o| (o.id, ed.board.name_of(o), o.kind.name())).collect();
                if items.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(38.0);
                        ui.label(theme::secondary(ui, "Nothing on this page"));
                    });
                }
                for (id, name, kind) in items {
                    ui.horizontal(|ui| {
                        ui.add_space(38.0);
                        let sel = ed.selection.contains(&id);
                        let text = RichText::new(format!("{}   {}", short_label(&name), kind)).size(12.0).color(if sel { p.text } else { p.text_secondary });
                        let resp = ui.add(egui::Button::selectable(sel, text).min_size(vec2(ui.available_width(), 20.0)).sense(Sense::click_and_drag()));
                        resp.dnd_set_drag_payload(LayerItem(id));
                        if resp.on_hover_text("Click to go to it; drag it onto another layer to move it there").clicked() {
                            action = Some(LayerAction::GoTo(id));
                        }
                    });
                }
            }
        }
    });
    ui.data_mut(|d| match &renaming {
        Some(r) => {
            d.insert_temp(rename_id, r.clone());
        }
        None => d.remove::<(Id, String)>(rename_id),
    });
    let Some(action) = action else { return };
    let layer_name = |ed: &Editor, id: Id| ed.board.layers.iter().find(|l| l.id == id).map(|l| l.name.clone()).unwrap_or_default();
    match action {
        LayerAction::Activate(id) => ed.active_layer = id,
        LayerAction::Toggle(id) => {
            if !ed.layers_open.remove(&id) {
                ed.layers_open.insert(id);
            }
        }
        LayerAction::Rename(id, name) => {
            ed.mark_changed();
            if let Some(l) = ed.board.layers.iter_mut().find(|l| l.id == id) {
                l.name = name;
            }
        }
        LayerAction::Hide(id) | LayerAction::Lock(id) => {
            ed.mark_changed();
            let hide = matches!(action, LayerAction::Hide(_));
            if let Some(l) = ed.board.layers.iter_mut().find(|l| l.id == id) {
                if hide {
                    l.hidden = !l.hidden;
                } else {
                    l.locked = !l.locked;
                }
            }
        }
        LayerAction::Color(id, c) => {
            ed.mark_changed();
            if let Some(l) = ed.board.layers.iter_mut().find(|l| l.id == id) {
                l.color = c;
            }
        }
        LayerAction::SelectAll(id) => {
            // Its items on this page, as whole groups and sketches (what a click on them would pick).
            let page = ed.page;
            let mut sel: Vec<Id> = ed.board.objects.iter().filter(|o| o.layer == id && o.page == page && !ed.board.veil.contains(&o.id)).map(|o| ed.board.outer_sketch(o.id)).filter(|x| !inner.contains(x) || ed.board.get(*x).is_some_and(|o| o.is_sketch())).collect();
            sel.dedup();
            sel.retain(|x| !inner.contains(x));
            ed.finish_edit();
            let count = sel.len();
            ed.selection = sel;
            ed.selected_wire = None;
            ed.say(format!("Selected {} on {}", plural(count, "item", "items"), layer_name(ed, id)));
        }
        LayerAction::Drop(item, id) => {
            // Dragging a selected item moves the whole selection.
            let ids = if ed.selection.contains(&item) { ed.selection.clone() } else { vec![item] };
            ed.mark_changed();
            ed.board.set_layer(&ids, id);
            ed.say(format!("Moved {} to {}", plural(ids.len(), "item", "items"), layer_name(ed, id)));
        }
        LayerAction::GoTo(item) => ed.go_to(item),
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
            ed.say(format!("Moved {} to {}", plural(sel.len(), "item", "items"), layer_name(ed, id)));
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

/// A note's text without markdown marks, for a short preview: everything after its title line.
fn note_snippet(text: &str) -> String {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("```") && !l.chars().all(|c| matches!(c, '-' | '|' | ':' | ' ' | '*' | '_')));
    lines.next();
    // Three short lines of preview never need more than the first few lines of the note.
    let plain: Vec<String> = lines
        .take(8)
        .map(|l| {
            let l = l.trim_start_matches('#').trim_start_matches('>').trim();
            let l = l.strip_prefix("- [ ] ").or_else(|| l.strip_prefix("- [x] ")).or_else(|| l.strip_prefix("- ")).or_else(|| l.strip_prefix("* ")).unwrap_or(l);
            // The words only: emphasis, code and link marks go.
            crate::markdown::inline(&l.replace('|', " ")).into_iter().map(|s| s.text).collect::<String>()
        })
        .collect();
    plain.join("  ")
}

/// A note being dragged in the Notes list (onto a folder).
#[derive(Clone, Copy)]
struct NoteDrag(Id);

/// What a click in the Notes list asks for, done once the list is drawn.
#[derive(Default)]
struct ListActs {
    open: Option<Id>,
    ring: Option<(Pos2, Id)>,
    drop: Option<(Id, Id)>,
    rename: Option<(Id, String)>,
    delete_folder: Option<Id>,
}

/// One note as a small card: title, the start of its text, when it was made.
fn note_card(ui: &mut Ui, ed: &mut Editor, n: &(Id, String, String, usize, String), indent: f32, acts: &mut ListActs) {
    let p = pal(ui.ctx());
    let (id, title, snippet, cards, time) = n;
    let w = ui.available_width() - indent;
    let pad = 10.0;
    let title_g = ui.painter().layout(title.clone(), egui::FontId::new(13.5, theme::semibold_family()), p.text, w - pad * 2.0);
    let mut job = egui::text::LayoutJob::simple(snippet.clone(), egui::FontId::proportional(12.0), p.text_secondary, w - pad * 2.0);
    job.wrap.max_rows = 3;
    job.wrap.overflow_character = Some('…');
    let snip_g = ui.painter().layout_job(job);
    let snip_h = if snippet.is_empty() { 0.0 } else { snip_g.size().y + 4.0 };
    let h = pad + title_g.size().y + snip_h + 18.0 + pad * 0.6;
    let (row, resp) = ui.allocate_exact_size(vec2(w + indent, h), Sense::click_and_drag());
    let rect = Rect::from_min_max(row.min + vec2(indent, 0.0), row.max);
    resp.dnd_set_drag_payload(NoteDrag(*id));
    let sel = ed.notes_sel.contains(id);
    let fill = if sel { p.accent.gamma_multiply(0.16) } else if resp.hovered() { p.bg_control_hover } else { p.inner_card_fill };
    let stroke = Stroke::new(if sel { 1.5 } else { 1.0 }, if sel { p.accent } else { p.inner_card_stroke });
    ui.painter().rect(rect, CornerRadius::same(8), fill, stroke, egui::StrokeKind::Inside);
    let th = title_g.size().y;
    ui.painter().galley(pos2(rect.left() + pad, rect.top() + pad), title_g, p.text);
    if !snippet.is_empty() {
        ui.painter().galley(pos2(rect.left() + pad, rect.top() + pad + th + 4.0), snip_g, p.text_secondary);
    }
    let foot = if *cards > 0 { format!("{time}  ·  on board") } else { time.clone() };
    ui.painter().text(pos2(rect.left() + pad, rect.bottom() - pad * 0.6), egui::Align2::LEFT_BOTTOM, foot, egui::FontId::proportional(11.0), p.text_secondary);
    if resp.double_clicked() {
        acts.open = Some(*id);
    } else if resp.clicked() {
        if ui.input(|i| i.modifiers.command || i.modifiers.shift) {
            if sel {
                ed.notes_sel.retain(|x| x != id);
            } else {
                ed.notes_sel.push(*id);
            }
        } else {
            ed.notes_sel = vec![*id];
        }
        ed.notes_focus = true;
    }
    // Holding the right button on a note opens its ring of options.
    if resp.hovered() && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary)) {
        acts.ring = ui.input(|i| i.pointer.hover_pos()).map(|s| (s, *id));
    }
}

/// A folder's row: fold arrow, name, count. Click to pick it (new notes go in it), double-click to
/// rename, drop notes on it to move them in. `id` 0 is the row for notes outside any folder.
fn folder_row(ui: &mut Ui, ed: &mut Editor, id: Id, name: &str, count: usize, acts: &mut ListActs) {
    let p = pal(ui.ctx());
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    let picked = ed.notes_folder == id;
    let dropping = resp.dnd_hover_payload::<NoteDrag>().is_some();
    let fill = if dropping { p.accent.gamma_multiply(0.25) } else if picked { p.accent.gamma_multiply(0.12) } else if resp.hovered() { p.bg_control_hover } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    let folded = ed.folded.contains(&id);
    // Fold arrow.
    let a = rect.left_center() + vec2(10.0, 0.0);
    let pts = if folded { vec![a + vec2(-2.0, -4.0), a + vec2(2.0, 0.0), a + vec2(-2.0, 4.0)] } else { vec![a + vec2(-4.0, -2.0), a + vec2(0.0, 2.0), a + vec2(4.0, -2.0)] };
    ui.painter().add(egui::Shape::line(pts, Stroke::new(1.5, p.text_secondary)));
    // Folder shape.
    if id != 0 {
        let f = Rect::from_min_size(rect.left_center() + vec2(20.0, -6.0), vec2(15.0, 11.0));
        ui.painter().rect_stroke(f, CornerRadius::same(2), Stroke::new(1.3, if picked { p.accent } else { p.text_secondary }), egui::StrokeKind::Middle);
        ui.painter().line_segment([f.left_top() + vec2(1.0, 0.0), f.left_top() + vec2(6.0, 0.0)], Stroke::new(2.4, if picked { p.accent } else { p.text_secondary }));
    }
    let text_x = rect.left() + if id != 0 { 42.0 } else { 22.0 };
    let renaming = ed.renaming_folder.as_ref().is_some_and(|(r, _)| *r == id);
    if renaming {
        let mut name = ed.renaming_folder.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
        let edit_rect = Rect::from_min_max(pos2(text_x - 4.0, rect.top() + 2.0), pos2(rect.right() - 30.0, rect.bottom() - 2.0));
        let te = ui.put(edit_rect, egui::TextEdit::singleline(&mut name).id_salt(("folder-name", id)));
        te.request_focus();
        let done = te.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter));
        ed.renaming_folder = Some((id, name.clone()));
        if done {
            acts.rename = Some((id, name));
        }
    } else {
        ui.painter().text(pos2(text_x, rect.center().y), egui::Align2::LEFT_CENTER, name, egui::FontId::new(13.0, theme::semibold_family()), if picked { p.text } else { p.text_secondary });
    }
    ui.painter().text(pos2(rect.right() - 26.0, rect.center().y), egui::Align2::RIGHT_CENTER, count.to_string(), egui::FontId::proportional(11.0), p.text_secondary);
    // Delete, on hover, for real folders.
    if id != 0 && resp.hovered() && !renaming {
        let x = Rect::from_center_size(pos2(rect.right() - 11.0, rect.center().y), vec2(16.0, 16.0));
        let over = ui.input(|i| i.pointer.hover_pos()).is_some_and(|q| x.contains(q));
        ui.painter().text(x.center(), egui::Align2::CENTER_CENTER, "×", egui::FontId::proportional(14.0), if over { p.danger } else { p.text_secondary });
        if over && resp.clicked() {
            acts.delete_folder = Some(id);
            return;
        }
    }
    if let Some(drag) = resp.dnd_release_payload::<NoteDrag>() {
        acts.drop = Some((drag.0, id));
    } else if resp.double_clicked() && id != 0 {
        ed.renaming_folder = Some((id, name.to_owned()));
    } else if resp.clicked() {
        let on_arrow = ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| q.x < rect.left() + 18.0);
        if on_arrow || picked {
            if !ed.folded.remove(&id) {
                ed.folded.insert(id);
            }
        }
        ed.notes_folder = id;
    }
}

/// The Notes list, on the left: folders, and every note as a small card with its title and the
/// start of its text. Click to pick (Ctrl-click for more, Delete deletes), double-click to open it in
/// a tab, drag onto a folder to move it, hold the right button for its options.
pub fn notes_ui(ui: &mut Ui, ed: &mut Editor) {
    ui.horizontal(|ui| {
        if ui.button("New note").on_hover_text("In the picked folder").clicked() {
            let id = ed.add_note("# New note\n\n", true);
            ed.notes_sel = vec![id];
        }
        if ui.button("New folder").clicked() {
            ed.add_folder();
        }
    });
    ui.add(egui::TextEdit::singleline(&mut ed.note_query).hint_text("Search notes").desired_width(f32::INFINITY));
    ui.add_space(6.0);

    // How many cards each note has, counted in one pass over the board.
    let mut card_count: std::collections::HashMap<Id, usize> = std::collections::HashMap::new();
    for o in &ed.board.objects {
        if let Kind::Note { note } = o.kind {
            *card_count.entry(note).or_default() += 1;
        }
    }
    let row_of = |_: &Editor, n: &crate::board::Note| (n.id, n.title(), note_snippet(&n.text), card_count.get(&n.id).copied().unwrap_or(0), crate::history::short_time(&n.created));
    if ed.board.notes.is_empty() && ed.board.folders.is_empty() {
        ui.label(theme::secondary(ui, "No notes yet. New note starts one; pasted markdown and dropped .md files become notes too."));
        return;
    }
    let q = ed.note_query.trim().to_lowercase();
    let mut acts = ListActs::default();
    ScrollArea::vertical().id_salt("notes-list").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
        if !q.is_empty() {
            // Searching: every match, wherever it is filed.
            let rows: Vec<_> = ed.board.notes.iter().rev().filter(|n| n.text.to_lowercase().contains(&q)).map(|n| row_of(ed, n)).collect();
            if rows.is_empty() {
                ui.label(theme::secondary(ui, "No note matches."));
            }
            for r in &rows {
                note_card(ui, ed, r, 0.0, &mut acts);
            }
            return;
        }
        let folders = ed.board.folders.clone();
        for f in &folders {
            let rows: Vec<_> = ed.board.notes.iter().rev().filter(|n| n.folder == f.id).map(|n| row_of(ed, n)).collect();
            folder_row(ui, ed, f.id, &f.name, rows.len(), &mut acts);
            if !ed.folded.contains(&f.id) {
                for r in &rows {
                    note_card(ui, ed, r, 14.0, &mut acts);
                }
            }
        }
        let loose: Vec<_> = ed.board.notes.iter().rev().filter(|n| !folders.iter().any(|f| f.id == n.folder)).map(|n| row_of(ed, n)).collect();
        // With folders, notes outside them get a row of their own (also a place to drop notes out).
        if !folders.is_empty() {
            folder_row(ui, ed, 0, "Not in a folder", loose.len(), &mut acts);
        }
        if folders.is_empty() || !ed.folded.contains(&0) {
            for r in &loose {
                note_card(ui, ed, r, 0.0, &mut acts);
            }
        }
    });
    if let Some(id) = acts.open {
        ed.open_note = Some(id);
    }
    if let Some((note, folder)) = acts.drop {
        // Dragging a picked note moves every picked note.
        let ids = if ed.notes_sel.contains(&note) { ed.notes_sel.clone() } else { vec![note] };
        ed.move_notes(&ids, folder);
    }
    if let Some((id, name)) = acts.rename {
        ed.renaming_folder = None;
        let name = name.trim().to_owned();
        if let Some(f) = ed.board.folders.iter_mut().find(|f| f.id == id).filter(|f| !name.is_empty() && f.name != name) {
            f.name = name;
            ed.mark_changed();
        }
    }
    if let Some(id) = acts.delete_folder {
        ed.delete_folder(id);
    }
    if let Some((s, id)) = acts.ring {
        if !ed.notes_sel.contains(&id) {
            ed.notes_sel = vec![id];
        }
        ed.notes_focus = true;
        let at = ed.paste_point();
        ed.radial = Some(crate::radial::Radial::for_note(s, at, id));
    }
}

/// A note open in its own tab: written in markdown, read formatted, or both side by side.
pub fn note_view(ui: &mut Ui, ed: &mut Editor, nid: Id) {
    let p = pal(ui.ctx());
    if ed.board.note(nid).is_none() {
        ui.label(theme::secondary(ui, "This note was deleted."));
        return;
    }
    let cards = ed.board.objects.iter().filter(|o| matches!(o.kind, Kind::Note { note } if note == nid)).count();
    ui.horizontal(|ui| {
        let mut mode = ed.note_mode;
        ui.allocate_ui(vec2(240.0, 26.0), |ui| {
            if ui.segmented(&mut mode, &[(0, "Write"), (1, "Split"), (2, "Read")]) {
                ed.note_mode = mode;
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Delete").on_hover_text(if cards == 0 { "Delete the note" } else { "Delete the note and its cards" }).clicked() {
                ed.delete_notes(&[nid]);
            }
            if ui.button("Insert image…").on_hover_text("Put a picture in the note (it is kept in the board)").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Pictures", &["png", "jpg", "jpeg", "gif", "bmp", "webp", "tif", "tiff"]).pick_file() {
                    match ed.embed_note_picture(&path) {
                        Some(line) => {
                            if let Some(n) = ed.board.note_mut(nid) {
                                if !n.text.ends_with('\n') {
                                    n.text.push('\n');
                                }
                                n.text.push_str(&line);
                                n.text.push('\n');
                            }
                        }
                        None => ed.say("That file is not a picture Overlay can read"),
                    }
                }
            }
            let place = if cards == 0 { "Place on board" } else { "Place another card" };
            if ui.button(place).on_hover_text("A card showing this note, in the middle of the view").clicked() {
                let at = ed.paste_point();
                ed.place_note(nid, at);
            }
            if cards > 0 {
                ui.label(theme::secondary(ui, plural(cards, "card on the board", "cards on the board")));
            }
        });
    });
    if ed.board.note(nid).is_none() {
        return;
    }
    // What links here, and what only names this note.
    let (linked, mentioned) = ed.backlinks(nid);
    let mut go = None;
    for (label, ids) in [("Linked from", &linked), ("Mentioned in", &mentioned)] {
        if ids.is_empty() {
            continue;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::secondary(ui, label));
            for id in ids.iter() {
                let t = ed.board.note(*id).map(|n| n.title()).unwrap_or_default();
                if ui.link(t).on_hover_text("Open this note").clicked() {
                    go = Some(*id);
                }
            }
        });
    }
    if linked.is_empty() && mentioned.is_empty() {
        ui.label(theme::secondary(ui, "No other note links here yet. Write [[ and a name to link notes; Ctrl+click a link to open it, or make the note if it is new."));
    }
    if let Some(id) = go {
        ed.open_note = Some(id);
    }
    ui.add_space(6.0);
    ui.separator();

    let avail = ui.available_width();
    match ed.note_mode {
        0 => centred(ui, avail, |ui, w| note_editor(ui, ed, nid, w)),
        2 => centred(ui, avail, |ui, w| note_reader(ui, ed, nid, w, &p)),
        _ => {
            ui.columns(2, |cols| {
                let w0 = cols[0].available_width();
                note_editor(&mut cols[0], ed, nid, w0);
                let w1 = cols[1].available_width();
                note_reader(&mut cols[1], ed, nid, w1, &p);
            });
        }
    }
}

/// A readable column, at most 760 wide, in the middle of `avail`.
fn centred(ui: &mut Ui, avail: f32, f: impl FnOnce(&mut Ui, f32)) {
    let w = avail.min(760.0);
    ui.horizontal_top(|ui| {
        ui.add_space(((avail - w) * 0.5).max(0.0));
        ui.vertical(|ui| {
            ui.set_width(w);
            f(ui, w);
        });
    });
}

fn note_editor(ui: &mut Ui, ed: &mut Editor, nid: Id, width: f32) {
    let p = pal(ui.ctx());
    let mut text = ed.board.note(nid).map(|n| n.text.clone()).unwrap_or_default();
    let te_id = ui.make_persistent_id(("note-text", nid));
    let ac_id = te_id.with("complete");
    // [[ completion: while its list is showing it takes the arrow keys, Enter, Tab and Esc.
    let (mut pick, list_len) = ui.data(|d| d.get_temp::<(usize, usize)>(ac_id)).unwrap_or((0, 0));
    let mut accept = false;
    let mut dismiss = false;
    if list_len > 0 {
        ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                pick = (pick + 1) % list_len;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                pick = (pick + list_len - 1) % list_len;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
                accept = true;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                dismiss = true;
            }
        });
    }
    if dismiss {
        ui.data_mut(|d| d.insert_temp(ac_id.with("off"), true));
    }
    let mut changed = false;
    let mut out = None;
    ScrollArea::vertical().id_salt(("note-edit", nid)).auto_shrink([false, false]).show(ui, |ui| {
        let rows = ((ui.available_height() / 19.0) as usize).max(6);
        let te = egui::TextEdit::multiline(&mut text).id(te_id).font(egui::FontId::proportional(14.5)).frame(egui::Frame::NONE).desired_width(width - 8.0).desired_rows(rows).lock_focus(true);
        let o = te.show(ui);
        changed = o.response.changed();
        out = Some(o);
    });
    let Some(out) = out else { return };
    if changed {
        ui.data_mut(|d| d.remove::<bool>(ac_id.with("off")));
    }

    // The words typed since an unclosed [[ before the cursor.
    let cursor = out.cursor_range.map(|c| c.primary.index.0);
    let chars: Vec<char> = text.chars().collect();
    let open = cursor.filter(|_| out.response.has_focus()).and_then(|c| {
        let before: String = chars[..c.min(chars.len())].iter().collect();
        let start = before.rfind("[[")?;
        let typed = &before[start + 2..];
        (!typed.contains("]]") && !typed.contains('\n')).then(|| (before[..start].chars().count() + 2, typed.to_owned(), c))
    });
    let off = ui.data(|d| d.get_temp::<bool>(ac_id.with("off"))).unwrap_or(false);
    let titles: Vec<String> = match &open {
        Some((_, typed, _)) if !off => {
            let q = typed.to_lowercase();
            let mut t: Vec<String> = ed.board.notes.iter().filter(|n| n.id != nid).map(|n| n.title()).filter(|t| q.is_empty() || t.to_lowercase().contains(&q)).collect();
            t.sort_by_key(|t| !t.to_lowercase().starts_with(&q));
            t.truncate(8);
            t
        }
        _ => Vec::new(),
    };
    let pick = pick.min(titles.len().saturating_sub(1));
    let mut chosen = accept.then(|| titles.get(pick).cloned()).flatten();
    if let (Some((from, _, to)), false) = (&open, titles.is_empty()) {
        // The list, just under the cursor.
        let at = out.galley_pos + out.galley.pos_from_cursor(egui::text::CCursor::new(*to)).left_bottom().to_vec2() + vec2(0.0, 4.0);
        egui::Area::new(ac_id.with("list")).order(egui::Order::Foreground).fixed_pos(at).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(p.window_fill).stroke(Stroke::new(1.0, p.window_stroke)).corner_radius(8).inner_margin(egui::Margin::same(4)).shadow(ui.ctx().global_style().visuals.popup_shadow).show(ui, |ui| {
                ui.set_min_width(200.0);
                for (k, t) in titles.iter().enumerate() {
                    if ui.add(egui::Button::selectable(k == pick, short_label(t)).min_size(vec2(200.0, 22.0))).clicked() {
                        chosen = Some(t.clone());
                    }
                }
            });
        });
        if let Some(t) = &chosen {
            // Replace what was typed after [[ with the name and the closing ]], cursor after it.
            let mut new: String = chars[..*from].iter().collect();
            new.push_str(t);
            new.push_str("]]");
            let after = chars[*to..].iter().collect::<String>();
            let after = after.strip_prefix("]]").unwrap_or(&after).to_owned();
            let cursor_at = new.chars().count();
            new.push_str(&after);
            text = new;
            changed = true;
            let mut state = out.state.clone();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(cursor_at))));
            state.store(ui.ctx(), te_id);
        }
    }
    if out.response.clicked() && ui.input(|i| i.modifiers.command) {
        if let Some(c) = cursor {
            let line_start = chars[..c.min(chars.len())].iter().rposition(|ch| *ch == '\n').map(|k| k + 1).unwrap_or(0);
            let line_end = chars[c.min(chars.len())..].iter().position(|ch| *ch == '\n').map(|k| k + c).unwrap_or(chars.len());
            let line: String = chars[line_start..line_end].iter().collect();
            let at = c - line_start;
            // The [[...]] around the click, on that line.
            let mut k = 0;
            for name in crate::markdown::wiki_links(&line) {
                let pat = format!("[[{name}");
                if let Some(s) = line[k..].find(&pat).map(|s| s + k) {
                    let start = line[..s].chars().count();
                    let end = start + name.chars().count() + 4;
                    if at >= start && at <= end {
                        ed.follow_link(&crate::markdown::Link::Wiki(name.clone()), Some(nid));
                        break;
                    }
                    k = s + pat.len();
                }
            }
        }
    }
    let shown = if chosen.is_some() { 0 } else { titles.len() };
    ui.data_mut(|d| d.insert_temp(ac_id, (pick, shown)));
    if changed {
        if let Some(n) = ed.board.note_mut(nid) {
            n.text = text;
        }
        ed.mark_changed();
    }
}

fn note_reader(ui: &mut Ui, ed: &mut Editor, nid: Id, width: f32, p: &crate::theme::Pal) {
    let blocks = ed.board.note(nid).map(|n| ed.note_blocks(&n.text)).unwrap_or_default();
    let mut clicked = None;
    ScrollArea::vertical().id_salt(("note-read", nid)).auto_shrink([false, false]).show(ui, |ui| {
        let width = (width - 8.0).max(40.0);
        let top = ui.cursor().min + vec2(0.0, 4.0);
        // Slightly larger type than on cards, for reading.
        let ctx = ui.ctx().clone();
        let (h, hits) = crate::markdown::paint(ui.painter(), &blocks, top, width, 1.1, p, p.text, &|src| ed.note_picture(&ctx, src));
        let resp = ui.allocate_rect(egui::Rect::from_min_size(top, vec2(width, h.max(20.0) + 12.0)), Sense::click());
        if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
            clicked = hits.into_iter().find(|(r, _)| r.contains(pos)).map(|(_, t)| t);
        }
    });
    match clicked {
        Some(crate::markdown::Target::Task(line)) => ed.toggle_note_task(nid, line),
        Some(crate::markdown::Target::Link(l)) => ed.follow_link(&l, Some(nid)),
        None => {}
    }
}

/// The Frames panel: the page's frames in slide order. Click one to go to it, reorder with the
/// arrows, present from the start or from any frame.
pub fn frames_ui(ui: &mut Ui, ed: &mut Editor) {
    let p = pal(ui.ctx());
    let frames = ed.board.frames(ed.page);
    ui.horizontal(|ui| {
        if ui.add_enabled(!frames.is_empty(), egui::Button::new("Present")).on_hover_text("Full screen, frame by frame (F5). Arrow keys move, Esc stops.").clicked() {
            ed.start_present(0);
        }
        if ui.button("New frame").on_hover_text("The Frame tool (F): drag on the board, Shift keeps 16:9").clicked() {
            ed.finish_edit();
            ed.tool = Tool::Frame;
        }
    });
    if frames.is_empty() {
        ui.label(theme::secondary(ui, "No frames on this page. Frames mark areas of the board, and become slides when presenting. Use the Frame tool (F) or Add > Shapes > Frame."));
        return;
    }
    ui.label(theme::secondary(ui, "In slide order. Double-click a name to rename it on the board."));
    ui.add_space(4.0);
    let mut swap = None;
    let mut go = None;
    let mut present = None;
    let mut rename = None;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (k, id) in frames.iter().enumerate() {
            let name = ed.board.get(*id).map(|o| o.label.clone()).unwrap_or_default();
            let sel = ed.selection.contains(id);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                ui.add_sized([22.0, 22.0], egui::Label::new(RichText::new(format!("{}", k + 1)).size(11.0).color(p.text_secondary)));
                let small = |ui: &mut Ui, t: &str, on: bool, tip: &str| ui.add_enabled(on, egui::Button::new(RichText::new(t).size(11.0)).frame(false).min_size(vec2(18.0, 22.0))).on_hover_text(tip).clicked();
                let w = (ui.available_width() - 64.0).max(40.0);
                let resp = ui.add(egui::Button::selectable(sel, short_label(if name.is_empty() { "Frame" } else { &name })).min_size(vec2(w, 22.0)));
                if resp.double_clicked() {
                    rename = Some(*id);
                } else if resp.clicked() {
                    go = Some(*id);
                }
                if small(ui, "▲", k > 0, "Earlier in the slideshow") {
                    swap = Some((k, k - 1));
                }
                if small(ui, "▼", k + 1 < frames.len(), "Later in the slideshow") {
                    swap = Some((k, k + 1));
                }
                if small(ui, "▶", true, "Present from here") {
                    present = Some(k);
                }
            });
        }
    });
    if let Some((a, b)) = swap {
        let slide_of = |ed: &Editor, id: Id| if let Some(Kind::Frame { slide }) = ed.board.get(id).map(|o| &o.kind) { *slide } else { 0 };
        let (sa, sb) = (slide_of(ed, frames[a]), slide_of(ed, frames[b]));
        // Equal numbers (older boards) are spread out first so the swap changes the order.
        let (sa, sb) = if sa == sb { (a as u32 + 1, b as u32 + 1) } else { (sa, sb) };
        ed.mark_changed();
        for (id, s) in [(frames[a], sb), (frames[b], sa)] {
            if let Some(o) = ed.board.get_mut(id) {
                o.kind = Kind::Frame { slide: s };
            }
        }
    }
    if let Some(id) = go {
        ed.show_frame(id);
    }
    if let Some(id) = rename {
        ed.show_frame(id);
        ed.start_edit(id);
    }
    if let Some(k) = present {
        ed.start_present(k);
    }
}

/// Use a colour for the selection and for what is drawn next. A colour that is not one of the
/// presets is remembered with the recent colours.
pub(crate) fn use_colour(ed: &mut Editor, ink: Ink) {
    if let Ink::Rgb(c) = ink {
        if !board::swatches().contains(&ink) {
            ed.custom_colors.retain(|x| *x != c);
            ed.custom_colors.insert(0, c);
            ed.custom_colors.truncate(10);
        }
    }
    ed.style.ink = ink;
    apply_to_selection(ed, |s| s.ink = ink);
}

/// The colour picker window: a saturation and brightness square with a hue strip, the red, green,
/// blue and hex values, the colour before and after, and the colours picked lately.
pub fn colour_picker(ctx: &egui::Context, ed: &mut Editor) {
    let Some(start) = ed.colour_pick else { return };
    let p = pal(ctx);
    let id = egui::Id::new("colour-picker");
    let start_c = start.color(p);
    let mut c: Color32 = ctx.data(|d| d.get_temp(id)).unwrap_or(start_c);
    let mut open = true;
    let mut apply = false;
    let mut cancel = false;
    egui::Window::new(theme::semibold("Colour").size(13.0))
        .id(id.with("window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            ui.spacing_mut().slider_width = 240.0;
            // The square and the hue strip only; the values are below, once.
            let mut hsva = egui::ecolor::Hsva::from(c);
            if egui::color_picker::color_picker_hsva_2d(ui, &mut hsva, egui::color_picker::Alpha::Opaque) {
                c = Color32::from(hsva);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                // Before and after, side by side.
                let (r, _) = ui.allocate_exact_size(vec2(64.0, 28.0), Sense::hover());
                let (a, b) = (Rect::from_min_max(r.min, pos2(r.center().x, r.max.y)), Rect::from_min_max(pos2(r.center().x, r.min.y), r.max));
                ui.painter().rect_filled(a, CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 }, start_c);
                ui.painter().rect_filled(b, CornerRadius { nw: 0, sw: 0, ne: 6, se: 6 }, c);
                ui.painter().rect_stroke(r, 6.0, Stroke::new(1.0, p.hairline), egui::StrokeKind::Inside);
                ui.add_space(8.0);
                let mut rgb = [c.r(), c.g(), c.b()];
                let mut changed = false;
                for (k, name) in ["R", "G", "B"].iter().enumerate() {
                    ui.label(theme::secondary(ui, *name));
                    changed |= ui.add(egui::DragValue::new(&mut rgb[k]).range(0..=255).speed(1.0)).changed();
                }
                if changed {
                    c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                }
            });
            ui.horizontal(|ui| {
                ui.label(theme::secondary(ui, "Hex"));
                let hex_id = id.with("hex");
                let mut hex: String = ui.data(|d| d.get_temp(hex_id)).unwrap_or_else(|| format!("{:02X}{:02X}{:02X}", c.r(), c.g(), c.b()));
                let r = ui.add(egui::TextEdit::singleline(&mut hex).desired_width(90.0).font(egui::FontId::monospace(13.0)));
                if r.changed() {
                    let h = hex.trim().trim_start_matches('#');
                    if h.len() == 6 {
                        if let Ok(v) = u32::from_str_radix(h, 16) {
                            c = Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
                        }
                    }
                }
                if r.has_focus() {
                    ui.data_mut(|d| d.insert_temp(hex_id, hex));
                } else {
                    ui.data_mut(|d| d.remove::<String>(hex_id));
                }
            });
            if !ed.custom_colors.is_empty() {
                ui.add_space(4.0);
                ui.label(theme::secondary(ui, "Recent"));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for rc in ed.custom_colors.clone() {
                        let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        ui.painter().rect(r.shrink(2.0), 4.0, Color32::from_rgb(rc[0], rc[1], rc[2]), Stroke::new(1.0, if resp.hovered() { p.accent } else { p.hairline }), egui::StrokeKind::Inside);
                        if resp.clicked() {
                            c = Color32::from_rgb(rc[0], rc[1], rc[2]);
                        }
                    }
                });
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("Apply").color(Color32::WHITE)).fill(p.accent).min_size(vec2(80.0, 28.0))).clicked() {
                    apply = true;
                }
                if ui.add(egui::Button::new("Cancel").min_size(vec2(80.0, 28.0))).clicked() {
                    cancel = true;
                }
            });
        });
    if apply {
        use_colour(ed, Ink::Rgb([c.r(), c.g(), c.b()]));
    }
    if apply || cancel || !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        ed.colour_pick = None;
        ctx.data_mut(|d| d.remove::<Color32>(id));
    } else {
        ctx.data_mut(|d| d.insert_temp(id, c));
    }
}
