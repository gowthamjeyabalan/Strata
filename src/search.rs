//! Search everything (Ctrl+K): one box that finds notes, text on the board, connections, frames,
//! layers and pages on every page, and runs any command by name.

use crate::board::{Id, Kind};
use crate::editor::Editor;
use crate::theme::{self, pal};
use crate::tools::Tool;
use egui::{CornerRadius, Key, Modifiers, Sense, Stroke, pos2, vec2};

#[derive(Clone, Default)]
pub struct Palette {
    pub query: String,
    pub index: usize,
    pub focus: bool,
}

/// A panel the palette can bring to the front.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Panel {
    Properties,
    Notes,
    Layers,
    Frames,
    Plot,
}

/// Commands, run by the app (some need its dialogs, settings or panels).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cmd {
    NewBoard,
    Open,
    Save,
    SaveAs,
    Export,
    InsertImage,
    Undo,
    Redo,
    ZoomFit,
    ZoomSelection,
    ActualSize,
    Overview,
    Snapping,
    NoteLinks,
    NewNote,
    NewFolder,
    NewPage,
    NewLayer,
    Present,
    Group,
    Ungroup,
    Lock,
    Duplicate,
    Delete,
    SelectAll,
    Dark,
    Light,
    System,
    ResetLayout,
    Settings,
    Tool(Tool),
    Show(Panel),
}

fn commands() -> Vec<(Cmd, String, &'static str)> {
    let mut v: Vec<(Cmd, String, &'static str)> = vec![
        (Cmd::NewBoard, "New board".into(), "file blank"),
        (Cmd::Open, "Open…".into(), "file load board"),
        (Cmd::Save, "Save".into(), "file write"),
        (Cmd::SaveAs, "Save as…".into(), "file copy"),
        (Cmd::Export, "Export image…".into(), "png jpg picture"),
        (Cmd::InsertImage, "Insert image…".into(), "picture photo add"),
        (Cmd::Undo, "Undo".into(), "back"),
        (Cmd::Redo, "Redo".into(), "forward"),
        (Cmd::ZoomFit, "Zoom to fit".into(), "view all whole board"),
        (Cmd::ZoomSelection, "Zoom to the selection".into(), "view selected focus"),
        (Cmd::ActualSize, "Actual size".into(), "zoom 100"),
        (Cmd::Overview, "Show or hide the Overview".into(), "map minimap view"),
        (Cmd::Snapping, "Turn snapping on or off".into(), "align guides"),
        (Cmd::NoteLinks, "Show or hide links between note cards".into(), "backlinks dotted"),
        (Cmd::NewNote, "New note".into(), "write markdown"),
        (Cmd::NewFolder, "New folder for notes".into(), "notes"),
        (Cmd::NewPage, "New page".into(), "board sheet"),
        (Cmd::NewLayer, "New layer".into(), "layers"),
        (Cmd::Present, "Present frames".into(), "slideshow slides full screen"),
        (Cmd::Group, "Group the selection".into(), "join sketch"),
        (Cmd::Ungroup, "Ungroup the selection".into(), "split"),
        (Cmd::Lock, "Lock or unlock the selection".into(), "pin"),
        (Cmd::Duplicate, "Duplicate the selection".into(), "copy"),
        (Cmd::Delete, "Delete the selection".into(), "remove"),
        (Cmd::SelectAll, "Select all".into(), "everything"),
        (Cmd::Dark, "Dark mode".into(), "theme appearance"),
        (Cmd::Light, "Light mode".into(), "theme appearance"),
        (Cmd::System, "Follow Windows' mode".into(), "theme appearance system"),
        (Cmd::ResetLayout, "Reset panel layout".into(), "panels dock"),
        (Cmd::Settings, "Settings".into(), "preferences options keyboard shortcuts keys"),
    ];
    for t in Tool::ALL {
        v.push((Cmd::Tool(t), format!("{} tool", t.name()), "tool draw"));
    }
    for (p, name) in [(Panel::Properties, "Properties"), (Panel::Notes, "Notes"), (Panel::Layers, "Layers"), (Panel::Frames, "Frames"), (Panel::Plot, "Plot")] {
        v.push((Cmd::Show(p), format!("Show {name}"), "panel"));
    }
    v
}

#[derive(Clone, Copy, PartialEq)]
enum Found {
    Cmd(Cmd),
    Note(Id),
    Item(Id),
    Wire(Id),
    Frame(Id),
    Layer(Id),
    Page(Id),
}

struct Row {
    found: Found,
    kind: &'static str,
    title: String,
    detail: String,
}

/// The first line of `text` that has every word, shortened around the first one.
fn snippet(text: &str, words: &[String]) -> String {
    let line = text.lines().find(|l| { let l = l.to_lowercase(); words.iter().all(|w| l.contains(w.as_str())) }).or_else(|| text.lines().find(|l| words.first().is_some_and(|w| l.to_lowercase().contains(w.as_str())))).unwrap_or("");
    let line = line.trim().trim_start_matches(['#', '-', '>', '*']).trim();
    if line.chars().count() > 70 { format!("{}…", line.chars().take(69).collect::<String>()) } else { line.to_owned() }
}

fn results(ed: &Editor, query: &str) -> Vec<Row> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_owned).collect();
    let has = |hay: &str| { let h = hay.to_lowercase(); words.iter().all(|w| h.contains(w.as_str())) };
    let page_name = |id: Id| ed.board.pages.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
    let mut out = Vec::new();
    for (c, name, keys) in commands() {
        if words.is_empty() || has(&format!("{name} {keys}")) {
            out.push(Row { found: Found::Cmd(c), kind: "Command", title: name, detail: String::new() });
        }
    }
    if words.is_empty() {
        out.truncate(10);
    }
    for p in &ed.board.pages {
        if words.is_empty() || has(&p.name) {
            out.push(Row { found: Found::Page(p.id), kind: "Page", title: p.name.clone(), detail: if p.id == ed.page { "showing".into() } else { String::new() } });
        }
    }
    if words.is_empty() {
        return out;
    }
    for o in &ed.board.objects {
        if let Kind::Frame { .. } = o.kind {
            if has(&o.label) {
                out.push(Row { found: Found::Frame(o.id), kind: "Frame", title: o.label.clone(), detail: page_name(o.page) });
            }
        }
    }
    for l in &ed.board.layers {
        if has(&l.name) {
            out.push(Row { found: Found::Layer(l.id), kind: "Layer", title: l.name.clone(), detail: String::new() });
        }
    }
    for n in &ed.board.notes {
        if has(&n.text) {
            out.push(Row { found: Found::Note(n.id), kind: "Note", title: n.title(), detail: snippet(&n.text, &words) });
        }
    }
    let inner = ed.board.sketch_inner();
    for o in ed.board.objects.iter().filter(|o| !inner.contains(&o.id) && !o.is_frame()) {
        let comments: String = o.comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join("\n");
        if !o.label.trim().is_empty() && has(&o.label) {
            out.push(Row { found: Found::Item(o.id), kind: o.kind.name(), title: snippet(&o.label, &words), detail: page_name(o.page) });
        } else if !comments.is_empty() && has(&comments) {
            out.push(Row { found: Found::Item(o.id), kind: "Comment", title: snippet(&comments, &words), detail: format!("on {} · {}", ed.board.display_name(o.id), page_name(o.page)) });
        }
    }
    for w in &ed.board.wires {
        if !w.label.is_empty() && has(&w.label) {
            let page = ed.board.get(w.from).map(|o| o.page).unwrap_or(0);
            out.push(Row { found: Found::Wire(w.id), kind: "Connection", title: w.label.clone(), detail: format!("{} → {} · {}", ed.board.display_name(w.from), ed.board.display_name(w.to), page_name(page)) });
        }
    }
    out.truncate(80);
    out
}

/// Draw the palette while it is open. Returns a command for the app to run.
pub fn show(ctx: &egui::Context, ed: &mut Editor) -> Option<Cmd> {
    let pal_state = ed.palette.clone()?;
    let p = pal(ctx);
    let rows = results(ed, &pal_state.query);
    let mut index = pal_state.index.min(rows.len().saturating_sub(1));
    let mut picked = None;
    let mut close = false;
    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) && !rows.is_empty() {
            index = (index + 1) % rows.len();
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowUp) && !rows.is_empty() {
            index = (index + rows.len() - 1) % rows.len();
        }
        if i.consume_key(Modifiers::NONE, Key::Enter) {
            picked = rows.get(index).map(|r| r.found);
        }
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            close = true;
        }
    });
    let screen = ctx.content_rect();
    let w = 560.0f32.min(screen.width() - 32.0);
    let at = pos2(screen.center().x - w * 0.5, screen.top() + 70.0);
    let mut query = pal_state.query.clone();
    let area = egui::Area::new(egui::Id::new("search-everything")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
        egui::Frame::new()
            .fill(p.window_fill)
            .stroke(Stroke::new(1.0, p.window_stroke))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(8))
            .shadow(ctx.global_style().visuals.popup_shadow)
            .show(ui, |ui| {
                ui.set_width(w);
                let edit = ui.add(egui::TextEdit::singleline(&mut query).hint_text("Search notes, the board, frames, pages, or type a command").font(egui::FontId::proportional(15.0)).desired_width(f32::INFINITY));
                if pal_state.focus {
                    edit.request_focus();
                }
                ui.add_space(6.0);
                if rows.is_empty() {
                    ui.label(theme::secondary(ui, "Nothing matches"));
                }
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    for (k, r) in rows.iter().enumerate() {
                        let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                        if resp.hovered() {
                            index = k;
                        }
                        if k == index {
                            ui.painter().rect_filled(row, CornerRadius::same(6), p.bg_control_hover);
                            if pal_state.index != index {
                                resp.scroll_to_me(None);
                            }
                        }
                        ui.painter().text(pos2(row.left() + 8.0, row.center().y), egui::Align2::LEFT_CENTER, r.kind, egui::FontId::proportional(11.0), p.text_secondary);
                        let title_w = row.width() - 96.0 - if r.detail.is_empty() { 0.0 } else { 150.0 };
                        let g = ui.painter().layout_no_wrap(r.title.clone(), egui::FontId::proportional(13.5), p.text);
                        let clip = ui.painter().with_clip_rect(egui::Rect::from_min_size(pos2(row.left() + 88.0, row.top()), vec2(title_w.max(40.0), row.height())));
                        clip.galley(pos2(row.left() + 88.0, row.center().y - g.size().y * 0.5), g, p.text);
                        if !r.detail.is_empty() {
                            let d = ui.painter().with_clip_rect(egui::Rect::from_min_max(pos2(row.right() - 150.0, row.top()), row.max));
                            d.text(pos2(row.right() - 8.0, row.center().y), egui::Align2::RIGHT_CENTER, &r.detail, egui::FontId::proportional(11.5), p.text_secondary);
                        }
                        if resp.clicked() {
                            picked = Some(r.found);
                        }
                    }
                });
            });
    });
    let clicked_outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|q| !area.response.rect.contains(q)));
    if close || clicked_outside {
        ed.palette = None;
        return None;
    }
    if let Some(found) = picked {
        ed.palette = None;
        match found {
            Found::Cmd(c) => return Some(c),
            Found::Note(id) => ed.open_note = Some(id),
            Found::Item(id) => ed.go_to(id),
            Found::Wire(id) => {
                if let Some(page) = ed.board.wire(id).and_then(|w| ed.board.get(w.from)).map(|o| o.page) {
                    ed.switch_page(page);
                    if let Some(c) = ed.board.wire(id).and_then(|w| ed.board.wire_points(w)).map(|pts| crate::board::midpoint(&pts)) {
                        ed.camera.look_at(ed.canvas_rect, c);
                    }
                    ed.selection.clear();
                    ed.clear_wires();
                    ed.selected_wire = Some(id);
                }
            }
            Found::Frame(id) => {
                if let Some(page) = ed.board.get(id).map(|o| o.page) {
                    ed.switch_page(page);
                    ed.fit_pending = false;
                    ed.show_frame(id);
                }
            }
            Found::Layer(id) => {
                ed.active_layer = id;
                let name = ed.board.layers.iter().find(|l| l.id == id).map(|l| l.name.clone()).unwrap_or_default();
                ed.say(format!("New items go on {name}"));
            }
            Found::Page(id) => ed.switch_page(id),
        }
        return None;
    }
    if let Some(s) = ed.palette.as_mut() {
        if query != s.query {
            index = 0;
        }
        s.query = query;
        s.index = index;
        s.focus = false;
    }
    None
}
