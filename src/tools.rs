//! Tools and their icons.
//!
//! The design system has no icon set. Tool icons are drawn here as small vector strokes in the style of
//! the Grapht mark (round caps and joins, one weight), coloured like text so they follow the theme.

use crate::board::Brush;
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Tool {
    #[default]
    Select,
    Hand,
    Pen,
    Pencil,
    Marker,
    Highlighter,
    Eraser,
    Line,
    Arrow,
    Polyline,
    Rectangle,
    Ellipse,
    Node,
    Text,
    Connect,
}

impl Tool {
    pub const ALL: [Tool; 15] = [
        Tool::Select,
        Tool::Hand,
        Tool::Pen,
        Tool::Pencil,
        Tool::Marker,
        Tool::Highlighter,
        Tool::Eraser,
        Tool::Line,
        Tool::Arrow,
        Tool::Polyline,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Node,
        Tool::Text,
        Tool::Connect,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Hand => "Pan",
            Tool::Pen => "Pen",
            Tool::Pencil => "Pencil",
            Tool::Marker => "Marker",
            Tool::Highlighter => "Highlighter",
            Tool::Eraser => "Eraser",
            Tool::Line => "Line",
            Tool::Arrow => "Arrow",
            Tool::Polyline => "Polyline",
            Tool::Rectangle => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Node => "Flowchart box",
            Tool::Text => "Text",
            Tool::Connect => "Connect",
        }
    }

    pub fn key(self) -> egui::Key {
        use egui::Key::*;
        match self {
            Tool::Select => V,
            Tool::Hand => H,
            Tool::Pen => P,
            Tool::Pencil => B,
            Tool::Marker => K,
            Tool::Highlighter => Y,
            Tool::Eraser => E,
            Tool::Line => L,
            Tool::Arrow => A,
            Tool::Polyline => U,
            Tool::Rectangle => R,
            Tool::Ellipse => O,
            Tool::Node => N,
            Tool::Text => T,
            Tool::Connect => C,
        }
    }

    pub fn key_name(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::Hand => "H",
            Tool::Pen => "P",
            Tool::Pencil => "B",
            Tool::Marker => "K",
            Tool::Highlighter => "Y",
            Tool::Eraser => "E",
            Tool::Line => "L",
            Tool::Arrow => "A",
            Tool::Polyline => "U",
            Tool::Rectangle => "R",
            Tool::Ellipse => "O",
            Tool::Node => "N",
            Tool::Text => "T",
            Tool::Connect => "C",
        }
    }

    /// The brush a freehand drawing tool paints with.
    pub fn brush(self) -> Option<Brush> {
        match self {
            Tool::Pen => Some(Brush::Pen),
            Tool::Pencil => Some(Brush::Pencil),
            Tool::Marker => Some(Brush::Marker),
            Tool::Highlighter => Some(Brush::Highlighter),
            _ => None,
        }
    }
}

/// Draw an icon into `rect`. `c` is the stroke colour, `dot` the accent for the one coloured detail.
pub fn paint_icon(p: &Painter, rect: Rect, tool: Tool, c: Color32, dot: Color32) {
    let s = rect.width().min(rect.height()) / 20.0;
    let o = rect.center() - vec2(10.0, 10.0) * s;
    let at = |x: f32, y: f32| -> Pos2 { o + vec2(x, y) * s };
    let st = Stroke::new(1.6 * s.max(0.8), c);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), st));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(Shape::closed_line(pts.iter().map(|&(x, y)| at(x, y)).collect(), st));
    };
    match tool {
        Tool::Select => closed(&[(5.0, 3.0), (5.0, 16.0), (8.6, 12.6), (11.2, 17.6), (13.2, 16.6), (10.8, 11.8), (15.6, 11.4)]),
        Tool::Hand => {
            line(&[(10.0, 2.5), (10.0, 17.5)]);
            line(&[(2.5, 10.0), (17.5, 10.0)]);
            line(&[(7.5, 5.0), (10.0, 2.5), (12.5, 5.0)]);
            line(&[(7.5, 15.0), (10.0, 17.5), (12.5, 15.0)]);
            line(&[(5.0, 7.5), (2.5, 10.0), (5.0, 12.5)]);
            line(&[(15.0, 7.5), (17.5, 10.0), (15.0, 12.5)]);
        }
        Tool::Pen => {
            // Fountain pen nib.
            closed(&[(10.0, 2.5), (15.0, 9.0), (12.0, 17.5), (8.0, 17.5), (5.0, 9.0)]);
            line(&[(10.0, 10.0), (10.0, 17.5)]);
            p.circle_filled(at(10.0, 9.0), 1.5 * s, dot);
        }
        Tool::Pencil => {
            closed(&[(13.5, 2.5), (17.5, 6.5), (7.0, 17.0), (3.0, 17.0), (3.0, 13.0)]);
            line(&[(11.0, 5.0), (15.0, 9.0)]);
            p.circle_filled(at(4.2, 15.8), 1.4 * s, dot);
        }
        Tool::Marker => {
            closed(&[(12.0, 2.5), (17.5, 8.0), (10.0, 15.5), (4.5, 10.0)]);
            closed(&[(4.5, 10.0), (10.0, 15.5), (7.0, 17.5), (2.5, 13.0)]);
            line(&[(2.5, 17.5), (6.0, 17.5)]);
            p.circle_filled(at(4.0, 17.5), 1.4 * s, dot);
        }
        Tool::Highlighter => {
            closed(&[(11.0, 2.5), (16.5, 8.0), (10.5, 14.0), (5.0, 8.5)]);
            line(&[(5.0, 8.5), (3.5, 13.0), (6.0, 15.5), (10.5, 14.0)]);
            let bar = Rect::from_min_max(at(2.5, 16.5), at(17.5, 18.5));
            p.rect_filled(bar, 1.0, dot.gamma_multiply(0.7));
        }
        Tool::Eraser => {
            closed(&[(3.5, 12.5), (10.5, 5.5), (16.0, 11.0), (11.0, 16.0), (7.0, 16.0)]);
            line(&[(7.0, 9.0), (12.5, 14.5)]);
            line(&[(11.0, 16.0), (17.0, 16.0)]);
        }
        Tool::Line => line(&[(4.0, 16.0), (16.0, 4.0)]),
        Tool::Arrow => {
            line(&[(4.0, 16.0), (16.0, 4.0)]);
            line(&[(9.5, 4.0), (16.0, 4.0), (16.0, 10.5)]);
        }
        Tool::Polyline => {
            line(&[(3.5, 15.5), (7.5, 6.0), (12.5, 13.0), (16.5, 4.5)]);
            for (x, y) in [(3.5, 15.5), (7.5, 6.0), (12.5, 13.0)] {
                p.circle_filled(at(x, y), 1.5 * s, c);
            }
            p.circle_filled(at(16.5, 4.5), 1.7 * s, dot);
        }
        Tool::Rectangle => {
            p.rect_stroke(Rect::from_min_max(at(3.5, 5.0), at(16.5, 15.0)), 1.5 * s, st, egui::StrokeKind::Middle);
        }
        Tool::Ellipse => {
            let pts: Vec<Pos2> = (0..32)
                .map(|i| {
                    let a = i as f32 / 32.0 * std::f32::consts::TAU;
                    at(10.0 + 6.8 * a.cos(), 10.0 + 5.2 * a.sin())
                })
                .collect();
            p.add(Shape::closed_line(pts, st));
        }
        Tool::Node => {
            p.rect_stroke(Rect::from_min_max(at(2.5, 3.5), at(9.5, 8.5)), 1.5 * s, st, egui::StrokeKind::Middle);
            p.rect_stroke(Rect::from_min_max(at(10.5, 11.5), at(17.5, 16.5)), 1.5 * s, st, egui::StrokeKind::Middle);
            line(&[(6.0, 8.5), (6.0, 14.0), (10.5, 14.0)]);
        }
        Tool::Text => {
            line(&[(4.5, 4.5), (15.5, 4.5)]);
            line(&[(10.0, 4.5), (10.0, 16.0)]);
            line(&[(7.5, 16.0), (12.5, 16.0)]);
        }
        Tool::Connect => {
            p.circle_stroke(at(4.5, 14.0), 2.2 * s, st);
            let curve = crate::board::sample_curve(&[at(6.7, 14.0), at(12.0, 14.0), at(8.0, 6.0), at(13.3, 6.0)], 16);
            p.add(Shape::line(curve, st));
            p.circle_filled(at(15.5, 6.0), 2.2 * s, dot);
        }
    }
}

/// Icon for the "Insert image" action.
pub fn paint_image_icon(p: &Painter, rect: Rect, c: Color32, dot: Color32) {
    let s = rect.width().min(rect.height()) / 20.0;
    let o = rect.center() - vec2(10.0, 10.0) * s;
    let at = |x: f32, y: f32| o + vec2(x, y) * s;
    let st = Stroke::new(1.6 * s.max(0.8), c);
    p.rect_stroke(Rect::from_min_max(at(3.0, 4.0), at(17.0, 16.0)), 2.0 * s, st, egui::StrokeKind::Middle);
    p.add(Shape::line(vec![at(3.5, 14.5), at(8.0, 10.0), at(11.0, 13.0), at(13.0, 11.0), at(16.5, 14.5)], st));
    p.circle_filled(at(13.0, 7.5), 1.6 * s, dot);
}

/// Icon for "Zoom to fit": four corners.
pub fn paint_fit_icon(p: &Painter, rect: Rect, c: Color32, dot: Color32) {
    let s = rect.width().min(rect.height()) / 20.0;
    let o = rect.center() - vec2(10.0, 10.0) * s;
    let at = |x: f32, y: f32| o + vec2(x, y) * s;
    let st = Stroke::new(1.6 * s.max(0.8), c);
    for pts in [[(3.0, 7.5), (3.0, 3.0), (7.5, 3.0)], [(12.5, 3.0), (17.0, 3.0), (17.0, 7.5)], [(17.0, 12.5), (17.0, 17.0), (12.5, 17.0)], [(7.5, 17.0), (3.0, 17.0), (3.0, 12.5)]] {
        p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), st));
    }
    p.circle_filled(at(10.0, 10.0), 1.6 * s, dot);
}

/// Icon for the Overview map: a frame with the view inside it.
pub fn paint_overview_icon(p: &Painter, rect: Rect, c: Color32, dot: Color32) {
    let s = rect.width().min(rect.height()) / 20.0;
    let o = rect.center() - vec2(10.0, 10.0) * s;
    let at = |x: f32, y: f32| o + vec2(x, y) * s;
    let st = Stroke::new(1.6 * s.max(0.8), c);
    p.rect_stroke(Rect::from_min_max(at(2.5, 4.0), at(17.5, 16.0)), 2.0 * s, st, egui::StrokeKind::Middle);
    p.rect_filled(Rect::from_min_max(at(9.5, 8.5), at(15.0, 13.5)), 1.0 * s, dot);
}

/// Icon for snapping: a magnet.
pub fn paint_snap_icon(p: &Painter, rect: Rect, c: Color32, dot: Color32) {
    let s = rect.width().min(rect.height()) / 20.0;
    let o = rect.center() - vec2(10.0, 10.0) * s;
    let at = |x: f32, y: f32| o + vec2(x, y) * s;
    let st = Stroke::new(2.4 * s.max(0.8), c);
    let mut pts = vec![at(5.0, 3.5), at(5.0, 10.0)];
    pts.extend((1..16).map(|i| {
        let a = std::f32::consts::PI * (1.0 - i as f32 / 16.0);
        at(10.0 + 5.0 * a.cos(), 10.0 + 5.0 * a.sin())
    }));
    pts.extend([at(15.0, 10.0), at(15.0, 3.5)]);
    p.add(Shape::line(pts, st));
    p.line_segment([at(3.5, 5.5), at(6.5, 5.5)], Stroke::new(1.4 * s.max(0.8), dot));
    p.line_segment([at(13.5, 5.5), at(16.5, 5.5)], Stroke::new(1.4 * s.max(0.8), dot));
}
