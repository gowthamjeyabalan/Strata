//! Drawing shapes with a full look: fill (solid or gradient), outline (solid, dashed or dotted),
//! rounded corners, and the shadow, glass and sketch effects.
//!
//! Every shape is first turned into an outline polygon in screen space, so all the options work the
//! same way for every shape.

use crate::board::{Dash, Shape};
use egui::{Color32, Mesh, Painter, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
pub enum Fill {
    None,
    Solid(Color32),
    /// Top colour, bottom colour.
    Gradient(Color32, Color32),
}

#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub fill: Fill,
    pub stroke: Option<Stroke>,
    pub dash: Dash,
    /// Corner radius in screen pixels; `None` uses the shape's own.
    pub radius: Option<f32>,
    pub shadow: bool,
    pub glass: bool,
    /// Seed for the hand-drawn wobble, if sketchy.
    pub sketch: Option<u64>,
}

impl Look {
    pub fn simple(fill: Color32, stroke: Stroke) -> Self {
        Look {
            fill: if fill == Color32::TRANSPARENT { Fill::None } else { Fill::Solid(fill) },
            stroke: Some(stroke),
            dash: Dash::Solid,
            radius: None,
            shadow: false,
            glass: false,
            sketch: None,
        }
    }
}

/// The corner radius a shape has unless told otherwise, in world units. `f32::INFINITY` means as
/// round as it can be (a pill).
pub fn default_radius(shape: Shape) -> f32 {
    match shape {
        Shape::Rectangle | Shape::Subprocess => 2.0,
        Shape::Node => 8.0,
        Shape::Process => 10.0,
        Shape::Terminal => f32::INFINITY,
        _ => 0.0,
    }
}

/// Can this shape's corners be rounded?
pub fn has_corners(shape: Shape) -> bool {
    !matches!(shape, Shape::Ellipse | Shape::Connector | Shape::Document | Shape::Database)
}

fn ellipse(c: Pos2, rx: f32, ry: f32, from: f32, to: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let a = from + (to - from) * i as f32 / n as f32;
            pos2(c.x + rx * a.cos(), c.y + ry * a.sin())
        })
        .collect()
}

/// Replace each corner of a closed polygon with a curve of radius `rad` (smaller where edges are short).
pub fn round_corners(pts: &[Pos2], rad: f32) -> Vec<Pos2> {
    if rad <= 0.05 || pts.len() < 3 {
        return pts.to_vec();
    }
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 8);
    for i in 0..n {
        let (a, v, b) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let (la, lb) = (a.distance(v), b.distance(v));
        let r = rad.min(la * 0.5).min(lb * 0.5);
        if r <= 0.05 {
            out.push(v);
            continue;
        }
        let p1 = v + (a - v) / la * r;
        let p2 = v + (b - v) / lb * r;
        // Quadratic curve with the corner as control point.
        let steps = ((r / 2.0).ceil() as usize).clamp(3, 12);
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            let u = 1.0 - t;
            out.push(pos2(u * u * p1.x + 2.0 * u * t * v.x + t * t * p2.x, u * u * p1.y + 2.0 * u * t * v.y + t * t * p2.y));
        }
    }
    out
}

/// A shape's outline in screen space, and whether it is convex.
pub fn outline(shape: Shape, r: Rect, z: f32, radius: Option<f32>) -> (Vec<Pos2>, bool) {
    let (l, t, rr, b) = (r.left(), r.top(), r.right(), r.bottom());
    let (w, h) = (r.width(), r.height());
    let rad = radius.unwrap_or_else(|| default_radius(shape) * z).min(w.min(h) * 0.5);
    let poly = |pts: Vec<Pos2>| (round_corners(&pts, rad), true);
    match shape {
        Shape::Rectangle | Shape::Node | Shape::Process | Shape::Subprocess | Shape::Terminal => {
            poly(vec![pos2(l, t), pos2(rr, t), pos2(rr, b), pos2(l, b)])
        }
        Shape::Decision => {
            let c = r.center();
            poly(vec![pos2(c.x, t), pos2(rr, c.y), pos2(c.x, b), pos2(l, c.y)])
        }
        Shape::Data => {
            let k = (h * 0.25).min(w * 0.2);
            poly(vec![pos2(l + k, t), pos2(rr, t), pos2(rr - k, b), pos2(l, b)])
        }
        Shape::ManualInput => poly(vec![pos2(l, t + h * 0.3), pos2(rr, t), pos2(rr, b), pos2(l, b)]),
        Shape::Note => {
            let k = w.min(h) * 0.2;
            poly(vec![pos2(l, t), pos2(rr - k, t), pos2(rr, t + k), pos2(rr, b), pos2(l, b)])
        }
        Shape::Ellipse => {
            let mut pts = ellipse(r.center(), w * 0.5, h * 0.5, 0.0, 2.0 * PI, 64);
            pts.pop();
            (pts, true)
        }
        Shape::Connector => {
            let rad = w.min(h) * 0.5;
            let mut pts = ellipse(r.center(), rad, rad, 0.0, 2.0 * PI, 48);
            pts.pop();
            (pts, true)
        }
        Shape::Document => {
            let a = h * 0.08;
            let mut pts = vec![pos2(l, t), pos2(rr, t)];
            for i in 0..=24 {
                let f = i as f32 / 24.0;
                pts.push(pos2(rr - f * w, b - a + a * (f * 2.0 * PI).sin()));
            }
            (pts, false)
        }
        Shape::Database => {
            let e = (h * 0.12).min(w * 0.25);
            let (cx, rx) = (r.center().x, w * 0.5);
            let mut pts = ellipse(pos2(cx, t + e), rx, e, PI, 2.0 * PI, 24);
            pts.extend(ellipse(pos2(cx, b - e), rx, e, 0.0, PI, 24));
            (pts, true)
        }
    }
}

/// Extra lines drawn inside some shapes (open polylines).
fn details(shape: Shape, r: Rect, z: f32) -> Vec<Vec<Pos2>> {
    let (l, t, rr, b) = (r.left(), r.top(), r.right(), r.bottom());
    let (w, h) = (r.width(), r.height());
    match shape {
        Shape::Subprocess => {
            let k = (w * 0.08).min(16.0 * z);
            vec![vec![pos2(l + k, t), pos2(l + k, b)], vec![pos2(rr - k, t), pos2(rr - k, b)]]
        }
        Shape::Note => {
            let k = w.min(h) * 0.2;
            vec![vec![pos2(rr - k, t), pos2(rr - k, t + k), pos2(rr, t + k)]]
        }
        Shape::Database => {
            let e = (h * 0.12).min(w * 0.25);
            vec![ellipse(pos2(r.center().x, t + e), w * 0.5, e, 0.0, PI, 24)]
        }
        _ => Vec::new(),
    }
}

/// A repeatable wobble in -1..1.
fn noise(seed: u64, i: usize) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (i as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x % 20_001) as f32 / 10_000.0 - 1.0
}

/// Resample a polyline every `step` pixels and push each point sideways a little.
fn wobble(pts: &[Pos2], closed: bool, step: f32, amount: f32, seed: u64) -> Vec<Pos2> {
    let mut path = pts.to_vec();
    if closed && !pts.is_empty() {
        path.push(pts[0]);
    }
    let mut out = Vec::new();
    let mut k = 0;
    for s in path.windows(2) {
        let (a, b) = (s[0], s[1]);
        let len = a.distance(b);
        let n = (len / step).ceil().max(1.0) as usize;
        let normal = if len > 0.0 { vec2(-(b - a).y, (b - a).x) / len } else { Vec2::ZERO };
        for j in 0..n {
            let p = a + (b - a) * (j as f32 / n as f32);
            out.push(p + normal * noise(seed, k) * amount);
            k += 1;
        }
    }
    if let Some(last) = path.last() {
        out.push(*last + vec2(noise(seed, k), noise(seed, k + 1)) * amount * 0.5);
    }
    out
}

/// Fill a polygon that is star-shaped around its centre with a triangle fan; `color_at` gives each
/// vertex its colour (for gradients).
fn fan(painter: &Painter, pts: &[Pos2], color_at: impl Fn(Pos2) -> Color32) {
    if pts.len() < 3 {
        return;
    }
    let c = pts.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2()) / pts.len() as f32;
    let c = c.to_pos2();
    let mut m = Mesh::default();
    m.colored_vertex(c, color_at(c));
    for p in pts {
        m.colored_vertex(*p, color_at(*p));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        m.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(egui::Shape::mesh(m));
}

fn fill_solid(painter: &Painter, pts: &[Pos2], convex: bool, color: Color32) {
    if color.a() == 0 {
        return;
    }
    if convex {
        painter.add(egui::Shape::convex_polygon(pts.to_vec(), color, Stroke::NONE));
    } else {
        fan(painter, pts, |_| color);
    }
}

/// Draw a polyline in the given dash style.
pub fn stroke_line(painter: &Painter, pts: &[Pos2], closed: bool, stroke: Stroke, dash: Dash) {
    if pts.len() < 2 || stroke.width <= 0.0 {
        return;
    }
    let mut path = pts.to_vec();
    match dash {
        Dash::Solid => {
            if closed {
                painter.add(egui::Shape::closed_line(path, stroke));
            } else {
                painter.add(egui::Shape::line(path, stroke));
            }
        }
        Dash::Dashed => {
            if closed {
                path.push(pts[0]);
            }
            let w = stroke.width;
            painter.extend(egui::Shape::dashed_line(&path, stroke, 3.0 * w + 4.0, 2.0 * w + 3.0));
        }
        Dash::Dotted => {
            if closed {
                path.push(pts[0]);
            }
            let w = stroke.width;
            painter.extend(egui::Shape::dotted_line(&path, stroke.color, 2.0 * w + 2.0, w * 0.55));
        }
    }
}

/// Draw a shape with a full look.
pub fn paint(painter: &Painter, shape: Shape, r: Rect, z: f32, look: &Look) {
    let (mut pts, convex) = outline(shape, r, z, look.radius);
    if pts.len() < 3 {
        return;
    }
    let extra = details(shape, r, z);
    let (top, bottom) = (r.top(), r.bottom().max(r.top() + 1.0));

    // Shadow: two soft offset copies under the shape.
    if look.shadow {
        let s = z.sqrt().clamp(0.5, 2.0);
        for (k, a) in [(1.0, 34u8), (0.55, 26u8)] {
            let off = vec2(3.0, 5.0) * s * k;
            let moved: Vec<Pos2> = pts.iter().map(|p| *p + off).collect();
            fill_solid(painter, &moved, convex, Color32::from_black_alpha(a));
        }
    }

    // A hand-drawn shape wobbles its fill a little too.
    if let Some(seed) = look.sketch {
        let amount = (1.2 * z.sqrt()).clamp(0.6, 3.0);
        pts = wobble(&pts, true, 12.0, amount, seed);
        pts.pop();
    }

    match look.fill {
        Fill::None => {}
        Fill::Solid(c) => fill_solid(painter, &pts, convex && look.sketch.is_none(), c),
        Fill::Gradient(a, b) => fan(painter, &pts, |p| a.lerp_to_gamma(b, ((p.y - top) / (bottom - top)).clamp(0.0, 1.0))),
    }

    if look.glass {
        let mid = top + (bottom - top) * 0.5;
        fan(painter, &pts, |p| {
            let t = ((p.y - top) / (mid - top)).clamp(0.0, 1.0);
            Color32::from_white_alpha((80.0 * (1.0 - t)) as u8)
        });
    }

    // Without an outline, a hairline in the fill colour smooths the fill's edge.
    if look.stroke.is_none() {
        let edge = match look.fill {
            Fill::None => None,
            Fill::Solid(c) => Some(c),
            Fill::Gradient(a, b) => Some(a.lerp_to_gamma(b, 0.5)),
        };
        if let Some(c) = edge {
            painter.add(egui::Shape::closed_line(pts.clone(), Stroke::new(1.0, c)));
        }
    }

    if let Some(stroke) = look.stroke {
        match look.sketch {
            Some(seed) => {
                // Two slightly different passes, like a pen going round twice.
                let amount = (1.0 * z.sqrt()).clamp(0.5, 2.5);
                let thin = Stroke::new((stroke.width * 0.8).max(0.75), stroke.color);
                for pass in 0..2u64 {
                    let line = wobble(&pts, true, 10.0, amount, seed.wrapping_add(pass * 7919));
                    stroke_line(painter, &line, false, thin, look.dash);
                    for (i, d) in extra.iter().enumerate() {
                        let dl = wobble(d, false, 10.0, amount, seed.wrapping_add(pass * 7919 + i as u64 + 1));
                        stroke_line(painter, &dl, false, thin, look.dash);
                    }
                }
            }
            None => {
                stroke_line(painter, &pts, true, stroke, look.dash);
                for d in &extra {
                    stroke_line(painter, d, false, stroke, look.dash);
                }
            }
        }
    }
}

/// Black or white, whichever reads better on `bg`.
pub fn contrast_text(bg: Color32) -> Color32 {
    let l = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if l > 150.0 { Color32::from_rgb(20, 22, 28) } else { Color32::WHITE }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_stays_inside_and_keeps_square_square() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        let (sq, _) = outline(Shape::Rectangle, r, 1.0, Some(0.0));
        assert_eq!(sq.len(), 4);
        let (round, convex) = outline(Shape::Rectangle, r, 1.0, Some(10.0));
        assert!(convex && round.len() > 4);
        assert!(round.iter().all(|p| r.expand(0.01).contains(*p)));
        // A pill: radius is capped at half the height.
        let (pill, _) = outline(Shape::Terminal, r, 1.0, None);
        assert!(pill.iter().all(|p| r.expand(0.01).contains(*p)));
        assert!(!pill.contains(&pos2(0.0, 0.0)), "corners are cut");
    }

    #[test]
    fn noise_is_repeatable_and_bounded() {
        for i in 0..200 {
            let a = noise(42, i);
            assert_eq!(a, noise(42, i));
            assert!((-1.0..=1.0).contains(&a));
        }
        assert_ne!(noise(1, 5), noise(2, 5));
    }

    #[test]
    fn readable_label_colour() {
        assert_eq!(contrast_text(Color32::from_rgb(237, 201, 72)), Color32::from_rgb(20, 22, 28));
        assert_eq!(contrast_text(Color32::from_rgb(40, 60, 120)), Color32::WHITE);
    }
}

/// Fill any simple polygon (concave too) by ear clipping. `pts` is in screen space; a repeated last
/// point is ignored.
pub fn fill_polygon(painter: &Painter, pts: &[Pos2], color: Color32) {
    let mut pts = pts.to_vec();
    if pts.len() > 1 && pts[0].distance(pts[pts.len() - 1]) < 0.01 {
        pts.pop();
    }
    if pts.len() < 3 || color.a() == 0 {
        return;
    }
    // Work counter-clockwise (in screen space, y down).
    let area: f32 = (0..pts.len()).map(|i| { let (a, b) = (pts[i], pts[(i + 1) % pts.len()]); a.x * b.y - b.x * a.y }).sum();
    let mut idx: Vec<usize> = (0..pts.len()).collect();
    if area < 0.0 {
        idx.reverse();
    }
    let cross = |a: Pos2, b: Pos2, c: Pos2| (b - a).x * (c - a).y - (b - a).y * (c - a).x;
    let mut mesh = egui::Mesh::default();
    for p in &pts {
        mesh.colored_vertex(*p, color);
    }
    let mut guard = 0;
    while idx.len() > 3 && guard < pts.len() * pts.len() {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for i in 0..n {
            let (ia, ib, ic) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(a, b, c) <= 0.0 {
                continue;
            }
            let ear = idx.iter().all(|&j| j == ia || j == ib || j == ic || !(cross(a, b, pts[j]) >= 0.0 && cross(b, c, pts[j]) >= 0.0 && cross(c, a, pts[j]) >= 0.0));
            if ear {
                mesh.add_triangle(ia as u32, ib as u32, ic as u32);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            // Self-crossing outline: fan out what is left rather than loop forever.
            break;
        }
    }
    for k in 1..idx.len().saturating_sub(1) {
        mesh.add_triangle(idx[0] as u32, idx[k] as u32, idx[k + 1] as u32);
    }
    painter.add(egui::Shape::mesh(mesh));
}
