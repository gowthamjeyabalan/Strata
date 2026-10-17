//! Snapping while you move or resize items: to other items' edges and centres, and to equal gaps
//! between items in a row or column. Pure geometry, in world units, so it can be tested on its own.

use egui::{Pos2, Rect, Vec2, pos2};

/// A guide to draw while snapped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    /// Two things line up along this line.
    Line(Pos2, Pos2),
    /// A gap that equals another gap; drawn as a measured span.
    Gap(Pos2, Pos2),
}

/// The best correction along one axis so far.
#[derive(Clone, Copy)]
struct Best {
    shift: f32,
    dist: f32,
}

impl Best {
    fn none(tol: f32) -> Self {
        Best { shift: 0.0, dist: tol }
    }
    /// Offer moving by `shift`; keep it if it is the smallest so far within tolerance.
    fn offer(&mut self, shift: f32) {
        if shift.abs() < self.dist {
            self.dist = shift.abs();
            self.shift = shift;
        }
    }
    fn found(&self, tol: f32) -> bool {
        self.dist < tol
    }
}

/// Lines of a rect along x (left, centre, right) or y (top, centre, bottom).
fn lines(r: Rect, x: bool) -> [f32; 3] {
    if x { [r.left(), r.center().x, r.right()] } else { [r.top(), r.center().y, r.bottom()] }
}

/// Do two rects share some extent along the other axis (so they sit in one row for `x`, one column for `!x`)?
fn in_line(a: Rect, b: Rect, x: bool) -> bool {
    if x { a.top() < b.bottom() && b.top() < a.bottom() } else { a.left() < b.right() && b.left() < a.right() }
}

/// Snap a rect that is being moved. Returns how far to shift it, and the guides to show.
/// `tol` is the snapping distance in world units.
pub fn snap_move(moving: Rect, targets: &[Rect], tol: f32) -> (Vec2, Vec<Guide>) {
    let mut shift = Vec2::ZERO;
    for x in [true, false] {
        let mut best = Best::none(tol);
        // Edges and centres.
        for t in targets {
            for m in lines(moving, x) {
                for l in lines(*t, x) {
                    best.offer(l - m);
                }
            }
        }
        // Equal gaps with items in the same row (x) or column (y).
        let row: Vec<Rect> = targets.iter().copied().filter(|t| in_line(*t, moving, x)).collect();
        for start in gap_candidates(moving, &row, x) {
            best.offer(start - if x { moving.left() } else { moving.top() });
        }
        if best.found(tol) {
            if x { shift.x = best.shift } else { shift.y = best.shift }
        }
    }
    let placed = moving.translate(shift);
    (shift, guides_for(placed, targets, tol * 0.05))
}

/// Where the moving rect's leading edge could go so that a gap matches a gap already in the row.
fn gap_candidates(moving: Rect, row: &[Rect], x: bool) -> Vec<f32> {
    let size = if x { moving.width() } else { moving.height() };
    let span = |r: &Rect| if x { (r.left(), r.right()) } else { (r.top(), r.bottom()) };
    let mut sorted: Vec<(f32, f32)> = row.iter().map(span).collect();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    for w in sorted.windows(2) {
        let (a, c) = (w[0], w[1]);
        let gap = c.0 - a.1;
        if gap <= 0.0 {
            continue;
        }
        // After the pair, before it, and centred between them.
        out.push(c.1 + gap);
        out.push(a.0 - gap - size);
        let inner = c.0 - a.1 - size;
        if inner > 0.0 {
            out.push(a.1 + inner * 0.5);
        }
    }
    out
}

fn guides_for(placed: Rect, targets: &[Rect], eps: f32) -> Vec<Guide> {
    let eps = eps.max(0.01);
    let mut out = Vec::new();
    for x in [true, false] {
        for m in lines(placed, x) {
            let matching: Vec<&Rect> = targets.iter().filter(|t| lines(**t, x).iter().any(|l| (l - m).abs() <= eps)).collect();
            if matching.is_empty() {
                continue;
            }
            let span = matching.iter().fold(placed, |acc, t| acc.union(**t));
            out.push(if x { Guide::Line(pos2(m, span.top()), pos2(m, span.bottom())) } else { Guide::Line(pos2(span.left(), m), pos2(span.right(), m)) });
        }
        // Equal gaps: show every gap in the row that equals the moving rect's gap to its neighbour.
        let row: Vec<Rect> = targets.iter().copied().filter(|t| in_line(*t, placed, x)).collect();
        let mut spans: Vec<(f32, f32, f32)> = Vec::new(); // (start, end, cross position)
        let mut all: Vec<Rect> = row.clone();
        all.push(placed);
        all.sort_by(|a, b| if x { a.left().total_cmp(&b.left()) } else { a.top().total_cmp(&b.top()) });
        for w in all.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (s, e) = if x { (a.right(), b.left()) } else { (a.bottom(), b.top()) };
            if e > s {
                let cross = if x { (a.center().y + b.center().y) * 0.5 } else { (a.center().x + b.center().x) * 0.5 };
                spans.push((s, e, cross));
            }
        }
        let mine: Vec<f32> = spans.iter().zip(all.windows(2)).filter(|(_, w)| w[0] == placed || w[1] == placed).map(|(s, _)| s.1 - s.0).collect();
        for g in mine {
            let equal: Vec<&(f32, f32, f32)> = spans.iter().filter(|s| ((s.1 - s.0) - g).abs() <= eps).collect();
            if equal.len() >= 2 {
                for s in equal {
                    out.push(if x { Guide::Gap(pos2(s.0, s.2), pos2(s.1, s.2)) } else { Guide::Gap(pos2(s.2, s.0), pos2(s.2, s.1)) });
                }
            }
        }
    }
    out
}

/// Snap the edges a resize handle is moving (`left`, `right`, `top`, `bottom` say which) to other
/// items' edges and centres. Returns the adjusted rect and guides.
pub fn snap_resize(r: Rect, moving: [bool; 4], targets: &[Rect], tol: f32) -> (Rect, Vec<Guide>) {
    let [left, right, top, bottom] = moving;
    let mut out = r;
    let mut guides = Vec::new();
    let snap_edge = |value: f32, x: bool| -> f32 {
        let mut best = Best::none(tol);
        for t in targets {
            for l in lines(*t, x) {
                best.offer(l - value);
            }
        }
        if best.found(tol) { value + best.shift } else { value }
    };
    if left {
        out.min.x = snap_edge(r.left(), true).min(r.right() - 1.0);
    }
    if right {
        out.max.x = snap_edge(r.right(), true).max(out.min.x + 1.0);
    }
    if top {
        out.min.y = snap_edge(r.top(), false).min(r.bottom() - 1.0);
    }
    if bottom {
        out.max.y = snap_edge(r.bottom(), false).max(out.min.y + 1.0);
    }
    for (on, value, x) in [(left, out.left(), true), (right, out.right(), true), (top, out.top(), false), (bottom, out.bottom(), false)] {
        if !on {
            continue;
        }
        let matching: Vec<&Rect> = targets.iter().filter(|t| lines(**t, x).iter().any(|l| (l - value).abs() <= 0.01)).collect();
        if !matching.is_empty() {
            let span = matching.iter().fold(out, |acc, t| acc.union(**t));
            guides.push(if x { Guide::Line(pos2(value, span.top()), pos2(value, span.bottom())) } else { Guide::Line(pos2(span.left(), value), pos2(span.right(), value)) });
        }
    }
    (out, guides)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }

    #[test]
    fn snaps_edges_and_centres() {
        let target = r(0.0, 0.0, 100.0, 50.0);
        // Left edges 3 apart: snaps; a guide shows the shared line.
        let (s, g) = snap_move(r(3.0, 200.0, 60.0, 40.0), &[target], 6.0);
        assert_eq!(s, vec2(-3.0, 0.0));
        assert!(g.iter().any(|g| matches!(g, Guide::Line(a, b) if a.x == 0.0 && b.x == 0.0)));
        // Centres: moving centre x = 52 vs 50.
        let (s, _) = snap_move(r(22.0, 200.0, 60.0, 40.0), &[target], 6.0);
        assert_eq!(s.x, -2.0);
        // Too far: nothing.
        let (s, g) = snap_move(r(13.0, 300.0, 30.0, 30.0), &[target], 6.0);
        assert_eq!(s, Vec2::ZERO);
        assert!(g.is_empty());
    }

    #[test]
    fn snaps_to_equal_gaps() {
        // Two boxes in a row 40 apart; a third dropped near 40 after the second snaps to exactly 40.
        let a = r(0.0, 0.0, 50.0, 50.0);
        let b = r(90.0, 0.0, 50.0, 50.0);
        let (s, g) = snap_move(r(183.0, 10.0, 50.0, 30.0), &[a, b], 6.0);
        assert_eq!(s.x, -3.0);
        assert!(g.iter().filter(|g| matches!(g, Guide::Gap(..))).count() >= 2);
    }

    #[test]
    fn resize_snaps_only_the_moving_edge() {
        let target = r(0.0, 0.0, 100.0, 50.0);
        let (out, g) = snap_resize(r(200.0, 0.0, 50.0, 47.0), [false, false, false, true], &[target], 6.0);
        assert_eq!(out.bottom(), 50.0);
        assert_eq!(out.left(), 200.0);
        assert!(!g.is_empty());
    }
}
