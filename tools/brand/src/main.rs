//! Strata brand kit generator.
//!
//! The mark is one strip folded into three layers: an S for Strata, the strata themselves, and the
//! blue dot (a node) where the strip ends. It follows the Graphsight rules: 10-unit round strokes in a
//! 100-unit box, one blue dot, and a heavier cut (12 stroke) for 32 px and smaller.
//!
//! Run from the repository root:  cargo run --release --manifest-path tools/brand/Cargo.toml
//! Everything is written to `brand/`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const NIGHT: &str = "#14161C";
const PAPER: &str = "#F5F5F7";
const BLUE: &str = "#0A84FF";
const WHITE: &str = "#FFFFFF";
const SECONDARY_DARK: &str = "#9898A2";
const SECONDARY_LIGHT: &str = "#6E6E73";
const TAGLINE: &str = "A whiteboard where every image, sketch and shape is a node.";

/// One cut of the mark, in a 100 x 100 box. The strip runs right to left along the top layer, turns
/// down on the left, runs back along the middle, turns down on the right, and runs left along the
/// bottom layer to just short of the dot.
struct Cut {
    stroke: f32,
    /// The three layers' heights (evenly spaced: each turn is half the spacing across).
    rows: [f32; 3],
    /// Where the top layer starts, where the turns are (left, right), and where the bottom layer stops.
    start: f32,
    left: f32,
    right: f32,
    end: f32,
    dot: (f32, f32),
    dot_radius: f32,
}

/// The cut used from 40 px up.
const REGULAR: Cut = Cut { stroke: 10.0, rows: [28.0, 50.0, 72.0], start: 74.5, left: 41.5, right: 63.5, end: 44.5, dot: (28.5, 72.0), dot_radius: 8.0 };
/// The heavier cut for 32 px and smaller.
const SMALL: Cut = Cut { stroke: 12.0, rows: [27.0, 50.0, 73.0], start: 75.0, left: 41.5, right: 63.5, end: 46.0, dot: (28.0, 73.0), dot_radius: 9.0 };

/// The regular mark's extent in its box: left, top, width, height.
const BOUNDS: (f32, f32, f32, f32) = (20.5, 23.0, 59.0, 54.0);

/// The mark as SVG elements: the folded strip in `ink`, the dot in `dot`.
fn mark(cut: &Cut, ink: &str, dot: &str) -> String {
    let [y0, y1, y2] = cut.rows;
    let t = (y1 - y0) * 0.5;
    let d = format!(
        "M{s:.2} {y0:.2} L{l:.2} {y0:.2} A{t} {t} 0 0 0 {l:.2} {y1:.2} L{r:.2} {y1:.2} A{t} {t} 0 0 1 {r:.2} {y2:.2} L{e:.2} {y2:.2}",
        s = cut.start,
        l = cut.left,
        r = cut.right,
        e = cut.end,
    );
    format!(
        r#"<path d="{d}" fill="none" stroke="{ink}" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"/><circle cx="{dx:.2}" cy="{dy:.2}" r="{dr}" fill="{dot}"/>"#,
        sw = cut.stroke,
        dx = cut.dot.0,
        dy = cut.dot.1,
        dr = cut.dot_radius,
    )
}

/// The mark alone, on a transparent background.
fn mark_svg(cut: &Cut, ink: &str, dot: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="5 5 90 90" width="1024" height="1024">{}</svg>"#, mark(cut, ink, dot))
}

/// The app icon: the mark on a rounded square.
fn icon_svg(cut: &Cut, bg: &str, ink: &str, dot: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="1024" height="1024"><rect width="100" height="100" rx="22.4" fill="{bg}"/><g transform="translate(5 5) scale(0.9)">{}</g></svg>"#,
        mark(cut, ink, dot)
    )
}

// ── text to paths ──

struct Font {
    data: Vec<u8>,
}

/// Collects a glyph outline as SVG path data, scaled and placed.
struct PathOut {
    d: String,
    scale: f32,
    x: f32,
    y: f32,
}

impl PathOut {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.y - y * self.scale)
    }
}

impl ttf_parser::OutlineBuilder for PathOut {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        let _ = write!(self.d, "M{x:.2} {y:.2}");
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        let _ = write!(self.d, "L{x:.2} {y:.2}");
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.p(x1, y1);
        let (x, y) = self.p(x, y);
        let _ = write!(self.d, "Q{x1:.2} {y1:.2} {x:.2} {y:.2}");
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.p(x1, y1);
        let (x2, y2) = self.p(x2, y2);
        let (x, y) = self.p(x, y);
        let _ = write!(self.d, "C{x1:.2} {y1:.2} {x2:.2} {y2:.2} {x:.2} {y:.2}");
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}

/// Text laid out as one path. `y` is the baseline. Returns the path data and its width.
struct Text {
    d: String,
    width: f32,
    /// Top of the lowercase letters above the baseline, and the deepest descender below it.
    x_height: f32,
    descent: f32,
}

impl Font {
    fn load(path: &Path) -> Font {
        Font { data: std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())) }
    }

    fn text(&self, s: &str, size: f32, x: f32, y: f32, tracking: f32) -> Text {
        let face = ttf_parser::Face::parse(&self.data, 0).expect("font parses");
        let scale = size / face.units_per_em() as f32;
        let mut out = PathOut { d: String::new(), scale, x, y };
        let mut descent: f32 = 0.0;
        for ch in s.chars() {
            let Some(g) = face.glyph_index(ch) else { continue };
            face.outline_glyph(g, &mut out);
            if let Some(b) = face.glyph_bounding_box(g) {
                descent = descent.max(-(b.y_min as f32) * scale);
            }
            out.x += face.glyph_hor_advance(g).unwrap_or(0) as f32 * scale + tracking * size;
        }
        let x_height = face.x_height().map(|h| h as f32 * scale).unwrap_or(size * 0.5);
        Text { d: out.d, width: out.x - x - tracking * size, x_height, descent }
    }
}

struct Fonts {
    display: Font,
    body: Font,
}

/// Mark plus wordmark side by side. The mark stands a little taller than the lowercase letters,
/// centred on them, like a capital S.
/// Returns (svg content, width, height) in user units, origin at the top left.
fn lockup_horizontal(f: &Fonts, ink: &str, dot: &str) -> (String, f32, f32) {
    let size = 100.0;
    let probe = f.display.text("strata", size, 0.0, 0.0, -0.01);
    let mark_h = probe.x_height * 1.45;
    let s = mark_h / BOUNDS.3;
    let top = 0.0;
    let baseline = top + mark_h * 0.5 + probe.x_height * 0.5;
    let text_x = BOUNDS.2 * s + mark_h * 0.3;
    let t = f.display.text("strata", size, text_x, baseline, -0.01);
    let height = (baseline + t.descent).max(mark_h);
    let body = format!(
        r#"<g transform="translate({tx:.2} {ty:.2}) scale({s:.4})">{m}</g><path d="{d}" fill="{ink}"/>"#,
        tx = -BOUNDS.0 * s,
        ty = top - BOUNDS.1 * s,
        m = mark(&REGULAR, ink, dot),
        d = t.d,
    );
    (body, text_x + t.width, height)
}

/// Mark above, wordmark below, centred.
fn lockup_stacked(f: &Fonts, ink: &str, dot: &str) -> (String, f32, f32) {
    let size = 100.0;
    let probe = f.display.text("strata", size, 0.0, 0.0, -0.01);
    let width = probe.width;
    let mark_h = width * 0.38;
    let s = mark_h / BOUNDS.3;
    let baseline = mark_h + size * 0.42 + probe.x_height;
    let t = f.display.text("strata", size, 0.0, baseline, -0.01);
    let body = format!(
        r#"<g transform="translate({tx:.2} {ty:.2}) scale({s:.4})">{m}</g><path d="{d}" fill="{ink}"/>"#,
        tx = (width - BOUNDS.2 * s) * 0.5 - BOUNDS.0 * s,
        ty = -BOUNDS.1 * s,
        m = mark(&REGULAR, ink, dot),
        d = t.d,
    );
    (body, width, baseline + t.descent)
}

fn wrap(body: &str, w: f32, h: f32, pad: f32, bg: Option<&str>) -> String {
    let (vw, vh) = (w + 2.0 * pad, h + 2.0 * pad);
    let bg = bg.map(|c| format!(r#"<rect x="{x}" y="{y}" width="{vw}" height="{vh}" fill="{c}"/>"#, x = -pad, y = -pad)).unwrap_or_default();
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x:.2} {y:.2} {vw:.2} {vh:.2}">{bg}{body}</svg>"#, x = -pad, y = -pad)
}

/// A wide header: horizontal lockup with the tagline under it, centred in `w` x `h`.
fn header(f: &Fonts, w: f32, h: f32, ink: &str, secondary: &str, bg: Option<&str>, lockup_w: f32) -> String {
    let (lock, lw, lh) = lockup_horizontal(f, ink, BLUE);
    let s = lockup_w / lw;
    let tag_size = lockup_w * 0.046;
    let probe = f.body.text(TAGLINE, tag_size, 0.0, 0.0, 0.0);
    let gap = tag_size * 1.5;
    let block_h = lh * s + gap + probe.x_height * 1.9;
    let y0 = (h - block_h) * 0.5;
    let tag = f.body.text(TAGLINE, tag_size, (w - probe.width) * 0.5, y0 + lh * s + gap + tag_size * 0.78, 0.0);
    let bg = bg.map(|c| format!(r#"<rect width="{w}" height="{h}" fill="{c}"/>"#)).unwrap_or_default();
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">{bg}<g transform="translate({x:.2} {y0:.2}) scale({s:.4})">{lock}</g><path d="{d}" fill="{secondary}"/></svg>"#,
        x = (w - lockup_w) * 0.5,
        d = tag.d,
    )
}

// ── rendering ──

fn render_png(svg: &str, w: u32, h: u32) -> Vec<u8> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).expect("valid svg");
    let size = tree.size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).expect("pixmap");
    let t = resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, t, &mut pixmap.as_mut());
    pixmap.encode_png().expect("png")
}

fn svg_aspect(svg: &str) -> f32 {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).expect("valid svg");
    tree.size().width() / tree.size().height()
}

struct Out {
    root: PathBuf,
    count: usize,
}

impl Out {
    fn write(&mut self, rel: &str, bytes: &[u8]) {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        self.count += 1;
    }
    fn svg(&mut self, name: &str, svg: &str) {
        self.write(&format!("svg/{name}.svg"), svg.as_bytes());
    }
    fn png(&mut self, name: &str, svg: &str, w: u32, h: u32) -> Vec<u8> {
        let png = render_png(svg, w, h);
        self.write(&format!("png/{name}.png"), &png);
        png
    }
}

fn ico(out: &mut Out, name: &str, regular: &str, small: &str) {
    let mut frames = Vec::new();
    for size in [16u32, 20, 24, 32, 40, 48, 64, 128, 256] {
        let svg = if size <= 32 { small } else { regular };
        let png = render_png(svg, size, size);
        frames.push(image::codecs::ico::IcoFrame::with_encoded(png, size, size, image::ExtendedColorType::Rgba8).expect("ico frame"));
    }
    let mut buf = Vec::new();
    image::codecs::ico::IcoEncoder::new(&mut buf).encode_images(&frames).expect("ico");
    out.write(&format!("ico/{name}.ico"), &buf);
}

fn main() {
    let root = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("brand"));
    let fonts_dir = std::env::args().nth(2).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("assets/fonts"));
    let fonts = Fonts { display: Font::load(&fonts_dir.join("Manrope-ExtraBold.ttf")), body: Font::load(&fonts_dir.join("Inter-Regular.ttf")) };
    let mut out = Out { root, count: 0 };

    // Marks.
    for (name, ink, dot) in [("color-on-dark", PAPER, BLUE), ("color-on-light", NIGHT, BLUE), ("white", WHITE, WHITE), ("ink", NIGHT, NIGHT)] {
        let svg = mark_svg(&REGULAR, ink, dot);
        out.svg(&format!("strata-mark-{name}"), &svg);
        out.png(&format!("strata-mark-{name}-1024"), &svg, 1024, 1024);
    }

    // App icons: Blue (the app icon, as Graphsight's) and Night, each with a small cut.
    let icons = [
        ("dark", icon_svg(&REGULAR, NIGHT, PAPER, BLUE), icon_svg(&SMALL, NIGHT, PAPER, BLUE)),
        ("blue", icon_svg(&REGULAR, BLUE, WHITE, WHITE), icon_svg(&SMALL, BLUE, WHITE, WHITE)),
    ];
    for (name, regular, small) in &icons {
        out.svg(&format!("strata-icon-{name}"), regular);
        out.svg(&format!("strata-icon-{name}-small"), small);
        for size in [16u32, 20, 24, 32, 40, 48, 64, 96, 128, 180, 192, 256, 512, 1024] {
            let svg = if size <= 32 { small } else { regular };
            out.png(&format!("strata-icon-{name}-{size}"), svg, size, size);
        }
    }
    out.svg("favicon", &icons[1].2);
    // Like Graphsight, the blue icon is the default and the Night one is the alternate.
    ico(&mut out, "strata", &icons[1].1, &icons[1].2);
    ico(&mut out, "strata-dark", &icons[0].1, &icons[0].2);

    // Wordmark alone.
    for (name, ink) in [("white", PAPER), ("ink", NIGHT)] {
        let t = fonts.display.text("strata", 100.0, 0.0, 100.0, -0.01);
        let body = format!(r#"<path d="{}" fill="{ink}"/>"#, t.d);
        out.svg(&format!("strata-wordmark-{name}"), &wrap(&body, t.width, 100.0 + t.descent, 2.0, None));
    }

    // Lockups.
    for (scheme, ink, bg) in [("on-dark", PAPER, NIGHT), ("on-light", NIGHT, PAPER)] {
        let (b, w, h) = lockup_horizontal(&fonts, ink, BLUE);
        let svg = wrap(&b, w, h, h * 0.12, None);
        out.svg(&format!("strata-lockup-horizontal-{scheme}"), &svg);
        let svg_bg = wrap(&b, w, h, h * 0.35, Some(bg));
        let a = svg_aspect(&svg_bg);
        out.png(&format!("strata-lockup-horizontal-{scheme}"), &svg_bg, 1200, (1200.0 / a).round() as u32);

        let (b, w, h) = lockup_stacked(&fonts, ink, BLUE);
        let svg = wrap(&b, w, h, w * 0.06, None);
        out.svg(&format!("strata-lockup-stacked-{scheme}"), &svg);
        let svg_bg = wrap(&b, w, h, w * 0.2, Some(bg));
        let a = svg_aspect(&svg_bg);
        out.png(&format!("strata-lockup-stacked-{scheme}"), &svg_bg, 1000, (1000.0 / a).round() as u32);
    }

    // README header (transparent, one per GitHub theme) and the social banner.
    let dark = header(&fonts, 1200.0, 300.0, PAPER, SECONDARY_DARK, None, 560.0);
    let light = header(&fonts, 1200.0, 300.0, NIGHT, SECONDARY_LIGHT, None, 560.0);
    out.svg("strata-readme-header-dark", &dark);
    out.svg("strata-readme-header-light", &light);
    out.png("strata-readme-header-dark", &dark, 2400, 600);
    out.png("strata-readme-header-light", &light, 2400, 600);
    let banner = header(&fonts, 1280.0, 640.0, PAPER, SECONDARY_DARK, Some(NIGHT), 640.0);
    out.svg("strata-social-banner", &banner);
    out.png("strata-social-banner-1280x640", &banner, 1280, 640);

    println!("Wrote {} files to {}", out.count, out.root.display());
}
