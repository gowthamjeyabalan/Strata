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
            renaming = Some(ed.add_layer());
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

/// A note's text without markdown marks, for a short preview: everything after its title line.
fn note_snippet(text: &str) -> String {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("```") && !l.chars().all(|c| matches!(c, '-' | '|' | ':' | ' ' | '*' | '_')));
    lines.next();
    let plain: Vec<String> = lines
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

    let row_of = |ed: &Editor, n: &crate::board::Note| {
        let cards = ed.board.objects.iter().filter(|o| matches!(o.kind, Kind::Note { note } if note == n.id)).count();
        (n.id, n.title(), note_snippet(&n.text), cards, crate::history::short_time(&n.created))
    };
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
        ui.label(theme::secondary(ui, "No other note links here yet. Write [[ and a name to link notes."));
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
    let blocks = ed.board.note(nid).map(|n| crate::markdown::parse(&n.text)).unwrap_or_default();
    let mut clicked = None;
    ScrollArea::vertical().id_salt(("note-read", nid)).auto_shrink([false, false]).show(ui, |ui| {
        let width = (width - 8.0).max(40.0);
        let top = ui.cursor().min + vec2(0.0, 4.0);
        // Slightly larger type than on cards, for reading.
        let (h, hits) = crate::markdown::paint(ui.painter(), &blocks, top, width, 1.1, p, p.text);
        let resp = ui.allocate_rect(egui::Rect::from_min_size(top, vec2(width, h.max(20.0) + 12.0)), Sense::click());
        if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
            clicked = hits.into_iter().find(|(r, _)| r.contains(pos)).map(|(_, t)| t);
        }
    });
    match clicked {
        Some(crate::markdown::Target::Task(line)) => ed.toggle_note_task(nid, line),
        Some(crate::markdown::Target::Link(l)) => ed.follow_link(&l),
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
