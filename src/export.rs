//! Export the board, the selection or the visible area as a PNG or JPG.
//!
//! The board is painted exactly as on screen (same drawing code) onto a hidden layer, turned into
//! triangles by egui's tessellator, and filled in on the CPU, so the export can be any size.

use crate::canvas::Camera;
use crate::editor::Editor;
use crate::theme::{self, UiExt, pal};
use egui::epaint::{ClippedPrimitive, Primitive};
use egui::{Color32, Id, LayerId, Order, Pos2, Rect, TextureId, Vec2, vec2};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Area {
    #[default]
    Board,
    Selection,
    View,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Format {
    #[default]
    Png,
    Jpg,
}

impl Format {
    fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpg => "jpg",
        }
    }
}

/// Largest export side, in pixels.
const MAX_SIDE: f32 = 16384.0;
/// Space around the exported items, in world units.
const MARGIN: f32 = 32.0;

pub struct ExportSettings {
    pub open: bool,
    pub area: Area,
    pub format: Format,
    pub scale: u32,
    pub transparent: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self { open: false, area: Area::Board, format: Format::Png, scale: 2, transparent: false }
    }
}

/// The world rectangle an area covers, or why there is nothing to export.
fn area_rect(ed: &Editor, area: Area) -> Result<Rect, &'static str> {
    let hidden = ed.board.hidden_set();
    let visible = |ids: &mut dyn Iterator<Item = &crate::board::Object>| ids.filter(|o| !hidden.contains(&o.id)).map(|o| o.rect()).reduce(|a, b| a.union(b));
    match area {
        Area::Board => visible(&mut ed.board.objects.iter()).map(|r| r.expand(MARGIN)).ok_or("The board is empty."),
        Area::Selection => {
            let ids = ed.board.with_descendants(&ed.selection);
            visible(&mut ed.board.objects.iter().filter(|o| ids.contains(&o.id))).map(|r| r.expand(MARGIN)).ok_or("Nothing is selected.")
        }
        Area::View => {
            let c = ed.canvas_rect;
            Ok(Rect::from_min_max(ed.camera.to_world(c.min, c.min), ed.camera.to_world(c.min, c.max)))
        }
    }
}

/// Pixel size and the scale actually used (lowered if the image would be too large).
fn pixel_size(world: Rect, scale: f32) -> (Vec2, f32) {
    let s = scale.min(MAX_SIDE / world.width().max(1.0)).min(MAX_SIDE / world.height().max(1.0));
    ((world.size() * s).round().max(vec2(1.0, 1.0)), s)
}

/// The Export window.
pub fn window(ctx: &egui::Context, ed: &mut Editor, s: &mut ExportSettings) {
    if !s.open {
        return;
    }
    let mut open = true;
    let mut go = false;
    egui::Window::new(theme::semibold("Export").size(12.0))
        .id(egui::Id::new("export-window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(260.0)
        .default_pos(ed.canvas_rect.center() - vec2(130.0, 140.0))
        .show(ctx, |ui| {
            ui.set_width(260.0);
            theme::heading(ui, "AREA");
            ui.segmented(&mut s.area, &[(Area::Board, "Board"), (Area::Selection, "Selection"), (Area::View, "View")]);
            theme::heading(ui, "FORMAT");
            ui.segmented(&mut s.format, &[(Format::Png, "PNG"), (Format::Jpg, "JPG")]);
            theme::heading(ui, "SIZE");
            ui.segmented(&mut s.scale, &[(1, "1x"), (2, "2x"), (3, "3x"), (4, "4x")]);
            ui.add_space(4.0);
            if s.format == Format::Png {
                ui.switch(&mut s.transparent, "Transparent background");
            }
            ui.add_space(4.0);
            match area_rect(ed, s.area) {
                Ok(world) => {
                    let (size, used) = pixel_size(world, s.scale as f32);
                    let mut line = format!("{} x {} pixels", size.x, size.y);
                    if used < s.scale as f32 - 1e-3 {
                        line.push_str(&format!(", reduced to {:.2}x to stay under {} pixels a side", used, MAX_SIDE));
                    }
                    ui.label(theme::secondary(ui, line));
                    ui.add_space(4.0);
                    if ui.add_sized([ui.available_width(), 26.0], egui::Button::new("Export…")).clicked() {
                        go = true;
                    }
                }
                Err(why) => {
                    ui.label(theme::secondary(ui, why));
                }
            }
        });
    if !open {
        s.open = false;
    }
    if go {
        let name = ed.file_name();
        let stem = name.trim_end_matches(".overlay").trim_end_matches(".strata");
        let path = rfd::FileDialog::new()
            .add_filter(if s.format == Format::Png { "PNG image" } else { "JPG image" }, &[s.format.ext()])
            .set_file_name(format!("{stem}.{}", s.format.ext()))
            .save_file();
        if let Some(path) = path {
            match export_to(ctx, ed, s, &path) {
                Ok((w, h)) => {
                    ed.say(format!("Exported {} ({w} x {h})", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
                    s.open = false;
                }
                Err(e) => ed.say(format!("Could not export: {e}")),
            }
        }
    }
}

fn export_to(ctx: &egui::Context, ed: &mut Editor, s: &ExportSettings, path: &std::path::Path) -> Result<(u32, u32), String> {
    let world = area_rect(ed, s.area).map_err(|e| e.to_owned())?;
    let p = pal(ctx);
    let bg = if s.format == Format::Png && s.transparent { None } else { Some(p.canvas) };
    let (w, h, px) = render(ctx, ed, world, s.scale as f32, bg)?;
    match s.format {
        Format::Png => {
            let mut img = image::RgbaImage::new(w, h);
            for (dst, c) in img.pixels_mut().zip(&px) {
                *dst = image::Rgba(c.to_srgba_unmultiplied());
            }
            img.save_with_format(path, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        }
        Format::Jpg => {
            let mut img = image::RgbImage::new(w, h);
            for (dst, c) in img.pixels_mut().zip(&px) {
                *dst = image::Rgb([c.r(), c.g(), c.b()]);
            }
            let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 92);
            enc.encode_image(&img).map_err(|e| e.to_string())?;
        }
    }
    Ok((w, h))
}

/// Paint `world` at `scale` pixels per world unit. Returns premultiplied pixels, row by row.
pub fn render(ctx: &egui::Context, ed: &mut Editor, world: Rect, scale: f32, background: Option<Color32>) -> Result<(u32, u32, Vec<Color32>), String> {
    let (size, scale) = pixel_size(world, scale);
    let (w, h) = (size.x as u32, size.y as u32);
    let frame = Rect::from_min_size(Pos2::ZERO, size);

    // Paint with a camera that maps the area onto the image, and without selection or hover marks.
    let saved = (ed.camera, std::mem::take(&mut ed.selection), ed.selected_wire.take(), ed.hover_world.take(), ed.tool, ed.editing.take(), ed.editing_wire.take());
    ed.camera = Camera { pan: [-world.min.x * scale, -world.min.y * scale], zoom: scale };
    ed.tool = crate::tools::Tool::Select;
    let layer = LayerId::new(Order::Background, Id::new("strata-export"));
    let painter = egui::Painter::new(ctx.clone(), layer, frame);
    let look = ed.board.lookup();
    // Exports show items at full strength even while the Calendar fades some.
    let shown = std::mem::take(&mut ed.day_shown);
    ed.paint_scene(ctx, &painter, Pos2::ZERO, &look);
    ed.day_shown = shown;
    ed.paint_wired_ports(&painter, Pos2::ZERO, pal(ctx), &look);
    let shapes: Vec<egui::epaint::ClippedShape> = ctx.graphics_mut(|g| std::mem::take(g.entry(layer))).all_entries().cloned().collect();
    (ed.camera, ed.selection, ed.selected_wire, ed.hover_world, ed.tool, ed.editing, ed.editing_wire) = saved;

    let prims = ctx.tessellate(shapes, 1.0);

    // Textures the triangles sample: the font atlas, and each image at full size.
    let mut textures: HashMap<TextureId, Tex> = HashMap::new();
    textures.insert(TextureId::default(), Tex::from(&ctx.fonts(|f| f.image())));
    for o in &ed.board.objects {
        if let crate::board::Kind::Image { image, .. } = o.kind {
            let Some(Some(handle)) = ed.textures.get(&image) else { continue };
            if textures.contains_key(&handle.id()) {
                continue;
            }
            if let Some(img) = ed.board.images.get(&image).and_then(|a| image::load_from_memory(&a.bytes).ok()) {
                let rgba = img.to_rgba8();
                let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
                textures.insert(handle.id(), Tex::from(&ci));
            }
        }
    }

    let mut px = vec![background.unwrap_or(Color32::TRANSPARENT); (w * h) as usize];
    rasterize(&prims, &textures, w as usize, h as usize, &mut px);
    Ok((w, h, px))
}

/// A texture in floats (premultiplied, 0 to 1).
struct Tex {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl From<&egui::ColorImage> for Tex {
    fn from(c: &egui::ColorImage) -> Self {
        Tex { w: c.size[0], h: c.size[1], px: c.pixels.iter().map(|p| rgba_f(*p)).collect() }
    }
}

fn rgba_f(c: Color32) -> [f32; 4] {
    [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, c.a() as f32 / 255.0]
}

impl Tex {
    /// Bilinear sample at normalised `uv`.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = (u * self.w as f32 - 0.5).clamp(0.0, (self.w - 1) as f32);
        let y = (v * self.h as f32 - 0.5).clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |x: usize, y: usize| self.px[y * self.w + x];
        let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
        let mut out = [0.0; 4];
        for k in 0..4 {
            out[k] = (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy;
        }
        out
    }
}

/// Sub-pixel steps per pixel. Vertices snap to this grid so edge tests are exact integers.
const SUB: f32 = 256.0;

/// Twice the signed area of (a, b, p) on the sub-pixel grid.
fn edge_i(a: (i64, i64), b: (i64, i64), p: (i64, i64)) -> i64 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

/// Fill egui's triangles into `out`, blending premultiplied colour over what is there.
///
/// Like a GPU, it snaps vertices to a sub-pixel grid and tests coverage with exact integers plus the
/// top-left rule, so pixels on an edge shared by two triangles are filled exactly once. (With floats,
/// the thin triangles egui makes for rounded corners can overlap by rounding and leave visible seams.)
fn rasterize(prims: &[ClippedPrimitive], textures: &HashMap<TextureId, Tex>, w: usize, h: usize, out: &mut [Color32]) {
    let mut acc: Vec<[f32; 4]> = out.iter().map(|c| rgba_f(*c)).collect();
    let snap = |p: Pos2| ((p.x * SUB).round() as i64, (p.y * SUB).round() as i64);
    for prim in prims {
        let Primitive::Mesh(mesh) = &prim.primitive else { continue };
        let tex = textures.get(&mesh.texture_id);
        let clip = prim.clip_rect.intersect(Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32)));
        if !clip.is_positive() {
            continue;
        }
        for tri in mesh.indices.chunks_exact(3) {
            let [mut a, mut b, c] = [&mesh.vertices[tri[0] as usize], &mesh.vertices[tri[1] as usize], &mesh.vertices[tri[2] as usize]];
            let (mut pa, mut pb, pc) = (snap(a.pos), snap(b.pos), snap(c.pos));
            let mut area = edge_i(pa, pb, pc);
            if area == 0 {
                continue;
            }
            // Same winding for every triangle, so the top-left rule picks one owner per shared edge.
            if area < 0 {
                std::mem::swap(&mut a, &mut b);
                std::mem::swap(&mut pa, &mut pb);
                area = -area;
            }
            let min = a.pos.min(b.pos).min(c.pos).max(clip.min);
            let max = a.pos.max(b.pos).max(c.pos).min(clip.max);
            if min.x >= max.x || min.y >= max.y {
                continue;
            }
            let (ca, cb, cc) = (rgba_f(a.color), rgba_f(b.color), rgba_f(c.color));
            let owns = |p: (i64, i64), q: (i64, i64)| {
                let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                dy > 0 || (dy == 0 && dx < 0)
            };
            let (tl0, tl1, tl2) = (owns(pb, pc), owns(pc, pa), owns(pa, pb));
            let inv = 1.0 / area as f32;
            for y in (min.y.floor() as usize)..(max.y.ceil() as usize).min(h) {
                for x in (min.x.floor() as usize)..(max.x.ceil() as usize).min(w) {
                    if !clip.contains(Pos2::new(x as f32 + 0.5, y as f32 + 0.5)) {
                        continue;
                    }
                    let p = (x as i64 * SUB as i64 + SUB as i64 / 2, y as i64 * SUB as i64 + SUB as i64 / 2);
                    let (i0, i1, i2) = (edge_i(pb, pc, p), edge_i(pc, pa, p), edge_i(pa, pb, p));
                    let inside = |e: i64, tl: bool| e > 0 || (e == 0 && tl);
                    if !(inside(i0, tl0) && inside(i1, tl1) && inside(i2, tl2)) {
                        continue;
                    }
                    let (e0, e1, e2) = (i0 as f32 * inv, i1 as f32 * inv, i2 as f32 * inv);
                    let u = e0 * a.uv.x + e1 * b.uv.x + e2 * c.uv.x;
                    let v = e0 * a.uv.y + e1 * b.uv.y + e2 * c.uv.y;
                    let t = tex.map(|t| t.sample(u, v)).unwrap_or([1.0; 4]);
                    let mut src = [0.0f32; 4];
                    for k in 0..4 {
                        src[k] = (e0 * ca[k] + e1 * cb[k] + e2 * cc[k]) * t[k];
                    }
                    let d = &mut acc[y * w + x];
                    let keep = 1.0 - src[3];
                    for k in 0..4 {
                        d[k] = src[k] + d[k] * keep;
                    }
                }
            }
        }
    }
    for (o, c) in out.iter_mut().zip(acc) {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        *o = Color32::from_rgba_premultiplied(q(c[0]), q(c[1]), q(c[2]), q(c[3]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Ink, Kind, Shape, Style};

    /// A see-through rounded fill covers every pixel exactly once: no darker seams where triangles meet.
    #[test]
    fn fills_without_seams() {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput::default());
        let layer = LayerId::new(Order::Background, Id::new("t"));
        let painter = egui::Painter::new(ctx.clone(), layer, Rect::from_min_size(Pos2::ZERO, vec2(240.0, 140.0)));
        let r = Rect::from_min_max(Pos2::new(20.3, 20.7), Pos2::new(220.1, 120.4));
        painter.rect_filled(r, 4.0, Color32::from_rgba_premultiplied(40, 0, 0, 40));
        painter.circle_filled(Pos2::new(120.0, 70.0), 30.3, Color32::from_rgba_premultiplied(0, 0, 40, 40));
        let shapes: Vec<egui::epaint::ClippedShape> = ctx.graphics_mut(|g| std::mem::take(g.entry(layer))).all_entries().cloned().collect();
        let prims = ctx.tessellate(shapes, 1.0);
        let mut textures = HashMap::new();
        textures.insert(TextureId::default(), Tex::from(&ctx.fonts(|f| f.image())));
        let mut px = vec![Color32::TRANSPARENT; 240 * 140];
        rasterize(&prims, &textures, 240, 140, &mut px);
        ctx.end_pass().textures_delta.clear();
        let at = |x: usize, y: usize| px[y * 240 + x];
        // Inside the box, away from its feathered edge and from the circle.
        let box_only = (25..215).flat_map(|x| (25..116).map(move |y| (x, y))).filter(|&(x, y)| (x as f32 - 120.0).hypot(y as f32 - 70.0) > 32.0);
        assert!(box_only.into_iter().all(|(x, y)| at(x, y).a() == 40), "box fill is uneven");
        // Inside the circle both layers blend once each.
        let mid = at(120, 70);
        assert!(mid.a() > 40 && mid.a() < 80, "circle over box {mid:?}");
        assert!((100..140).flat_map(|x| (60..80).map(move |y| (x, y))).all(|(x, y)| at(x, y) == mid), "circle fill is uneven");
    }

    /// Render a filled red box with a black label on white, without a window.
    #[test]
    fn renders_shapes_and_text() {
        let ctx = egui::Context::default();
        ctx.set_theme(egui::Theme::Light);
        let mut ed = Editor::default();
        let style = Style { ink: Ink::Rgb([255, 0, 0]), width: 4.0, fill: true, font_size: 20.0, ..Style::default() };
        let id = ed.board.add(Rect::from_min_size(Pos2::new(0.0, 0.0), vec2(100.0, 50.0)), Kind::Shape { shape: Shape::Rectangle }, style);
        ed.board.get_mut(id).unwrap().label = "Hi".into();
        let world = Rect::from_min_size(Pos2::new(-10.0, -10.0), vec2(120.0, 70.0));
        ctx.begin_pass(egui::RawInput::default());
        let (w, h, px) = render(&ctx, &mut ed, world, 2.0, Some(Color32::WHITE)).unwrap();
        // No window here, so drop the font texture upload egui prepared for one.
        ctx.end_pass().textures_delta.clear();
        assert_eq!((w, h), (240, 140));
        let at = |x: u32, y: u32| px[(y * w + x) as usize];
        // Outside the box: background.
        assert_eq!(at(2, 2), Color32::WHITE);
        // On the border (world x = 0, so pixel 20): solid red.
        let border = at(20, 70);
        assert!(border.r() > 200 && border.g() < 60 && border.b() < 60, "border {border:?}");
        // Inside: a light red fill, not white and not solid.
        let fill = at(40, 30);
        assert!(fill.r() > 200 && fill.g() > 150 && fill.g() < 250, "fill {fill:?}");
        // The label (dark text in the light theme) puts dark pixels near the centre.
        let text = (100..140).flat_map(|x| (55..85).map(move |y| (x, y))).filter(|&(x, y)| at(x, y).g() < 100).count();
        assert!(text > 10, "expected text pixels, found {text}");
        // The fill is even: no seam along the diagonal where its triangles meet (away from the label).
        let inside: Vec<Color32> = (30..210).step_by(7).flat_map(|x| (30..45).step_by(3).map(move |y| (x, y))).map(|(x, y)| at(x, y)).collect();
        assert!(inside.iter().all(|c| *c == fill), "uneven fill");
        // Painting for the export left nothing behind and restored the view.
        assert_eq!(ed.camera.zoom, Camera::default().zoom);
    }
}
