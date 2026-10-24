//! Plots: equations drawn as graphs, in the Plot panel and as live items on the board.
//!
//! An equation is one of: `y = …` (or just an expression in x), `r = …` (polar, in θ), `(x(t), y(t))`
//! or `x = …, y = …` (parametric, in t), `x = 3` (a vertical line), or `a = 2`, which sets a constant
//! (every letter that is not x, y, t, r or θ is a constant and gets a slider).

use crate::board::Ink;
use crate::editor::Editor;
use crate::theme::{self, Pal, UiExt, pal};
use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde::{Deserialize, Serialize};

// ── what is saved ──

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PlotEq {
    pub text: String,
    pub ink: Ink,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PlotParam {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

/// A plot: its equations, the constants' values, and the part of the plane shown.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PlotSpec {
    pub eqs: Vec<PlotEq>,
    pub params: Vec<PlotParam>,
    /// Centre x, centre y, and how many units wide the view is (the scale is the same both ways).
    pub view: [f32; 3],
    /// The range of t for parametric curves and of θ for polar ones.
    pub t_range: [f32; 2],
    pub grid: bool,
}

impl Default for PlotSpec {
    fn default() -> Self {
        PlotSpec {
            eqs: vec![PlotEq { text: "y = sin(x)".into(), ink: curve_ink(0), hidden: false }],
            params: Vec::new(),
            view: [0.0, 0.0, 20.0],
            t_range: [0.0, std::f32::consts::TAU],
            grid: true,
        }
    }
}

/// The colour for the `k`th curve.
fn curve_ink(k: usize) -> Ink {
    const C: [[u8; 3]; 6] = [[47, 128, 237], [235, 87, 87], [39, 174, 96], [242, 153, 74], [155, 81, 224], [45, 183, 199]];
    Ink::Rgb(C[k % C.len()])
}

// ── expressions ──

#[derive(Clone, Debug)]
enum Ex {
    Num(f64),
    Var(String),
    Neg(Box<Ex>),
    Bin(char, Box<Ex>, Box<Ex>),
    Call(String, Vec<Ex>),
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Id(String),
    Op(char),
}

const FUNCTIONS: [&str; 22] = ["sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "sqrt", "abs", "ln", "log", "exp", "floor", "ceil", "round", "sign", "min", "max", "sec", "csc"];
/// Names that stand for themselves rather than a product of letters.
const NAMES: [&str; 4] = ["pi", "theta", "tau", "cot"];

fn tokens(s: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                i += 1;
            }
            // 1e-3
            if i < cs.len() && (cs[i] == 'e' || cs[i] == 'E') && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit() || ((*d == '-' || *d == '+') && cs.get(i + 2).is_some_and(|d| d.is_ascii_digit()))) {
                i += 2;
                while i < cs.len() && cs[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let t: String = cs[start..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("{t} is not a number"))?));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            let word: String = cs[start..i].iter().collect();
            split_word(&word, &mut out);
        } else if "+-*/^(),=|".contains(c) {
            out.push(Tok::Op(c));
            i += 1;
        } else if c == '·' || c == '×' {
            out.push(Tok::Op('*'));
            i += 1;
        } else if c == '−' {
            out.push(Tok::Op('-'));
            i += 1;
        } else if c == '÷' {
            out.push(Tok::Op('/'));
            i += 1;
        } else {
            return Err(format!("“{c}” is not understood"));
        }
    }
    Ok(out)
}

/// A run of letters: a function or a known name if it is one, otherwise each letter is a variable
/// multiplied by the next (as in 2ab). A function name inside the run is kept whole (xsin → x·sin).
fn split_word(word: &str, out: &mut Vec<Tok>) {
    let lower = word.to_lowercase();
    if FUNCTIONS.contains(&lower.as_str()) || NAMES.contains(&lower.as_str()) || word.chars().count() == 1 {
        out.push(Tok::Id(lower));
        return;
    }
    let cs: Vec<char> = word.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let rest: String = cs[i..].iter().collect::<String>().to_lowercase();
        if let Some(f) = FUNCTIONS.iter().chain(NAMES.iter()).filter(|f| rest.starts_with(**f)).max_by_key(|f| f.len()) {
            out.push(Tok::Id((*f).to_owned()));
            i += f.chars().count();
        } else {
            out.push(Tok::Id(cs[i].to_string()));
            i += 1;
        }
    }
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(&Tok::Op(c)) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Result<Ex, String> {
        let mut a = self.term()?;
        loop {
            if self.eat('+') {
                a = Ex::Bin('+', Box::new(a), Box::new(self.term()?));
            } else if self.eat('-') {
                a = Ex::Bin('-', Box::new(a), Box::new(self.term()?));
            } else {
                return Ok(a);
            }
        }
    }
    fn term(&mut self) -> Result<Ex, String> {
        let mut a = self.unary()?;
        loop {
            if self.eat('*') {
                a = Ex::Bin('*', Box::new(a), Box::new(self.unary()?));
            } else if self.eat('/') {
                a = Ex::Bin('/', Box::new(a), Box::new(self.unary()?));
            } else if matches!(self.peek(), Some(Tok::Num(_)) | Some(Tok::Id(_)) | Some(Tok::Op('('))) {
                // Written side by side: 2x, x sin(x), (x+1)(x-1).
                a = Ex::Bin('*', Box::new(a), Box::new(self.power()?));
            } else {
                return Ok(a);
            }
        }
    }
    fn unary(&mut self) -> Result<Ex, String> {
        if self.eat('-') {
            return Ok(Ex::Neg(Box::new(self.unary()?)));
        }
        if self.eat('+') {
            return self.unary();
        }
        self.power()
    }
    fn power(&mut self) -> Result<Ex, String> {
        let base = self.primary()?;
        if self.eat('^') {
            return Ok(Ex::Bin('^', Box::new(base), Box::new(self.unary()?)));
        }
        Ok(base)
    }
    fn primary(&mut self) -> Result<Ex, String> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(Ex::Num(n))
            }
            Some(Tok::Id(name)) => {
                self.i += 1;
                if FUNCTIONS.contains(&name.as_str()) || name == "cot" {
                    // sin(x), sin^2(x), or sin x
                    let pow = if self.eat('^') { Some(self.primary()?) } else { None };
                    let args = if self.eat('(') {
                        let mut args = vec![self.expr()?];
                        while self.eat(',') {
                            args.push(self.expr()?);
                        }
                        if !self.eat(')') {
                            return Err("a bracket is not closed".into());
                        }
                        args
                    } else {
                        vec![self.power()?]
                    };
                    let call = Ex::Call(name, args);
                    return Ok(match pow {
                        Some(p) => Ex::Bin('^', Box::new(call), Box::new(p)),
                        None => call,
                    });
                }
                Ok(Ex::Var(name))
            }
            Some(Tok::Op('(')) => {
                self.i += 1;
                let e = self.expr()?;
                if !self.eat(')') {
                    return Err("a bracket is not closed".into());
                }
                Ok(e)
            }
            Some(Tok::Op('|')) => {
                self.i += 1;
                let e = self.expr()?;
                if !self.eat('|') {
                    return Err("an | is not closed".into());
                }
                Ok(Ex::Call("abs".into(), vec![e]))
            }
            Some(Tok::Op(c)) => Err(format!("“{c}” is out of place")),
            None => Err("something is missing at the end".into()),
        }
    }
}

fn parse(s: &str) -> Result<Ex, String> {
    let t = tokens(s)?;
    if t.is_empty() {
        return Err("empty".into());
    }
    let mut p = Parser { t, i: 0 };
    let e = p.expr()?;
    if p.i < p.t.len() {
        return Err(match &p.t[p.i] {
            Tok::Op(c) => format!("“{c}” is out of place"),
            _ => "something extra at the end".into(),
        });
    }
    Ok(e)
}

impl Ex {
    fn eval(&self, v: &dyn Fn(&str) -> f64) -> f64 {
        match self {
            Ex::Num(n) => *n,
            Ex::Var(name) => v(name),
            Ex::Neg(a) => -a.eval(v),
            Ex::Bin(op, a, b) => {
                let (a, b) = (a.eval(v), b.eval(v));
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => a / b,
                    _ => a.powf(b),
                }
            }
            Ex::Call(f, args) => {
                let a = args.first().map(|e| e.eval(v)).unwrap_or(f64::NAN);
                let b = args.get(1).map(|e| e.eval(v)).unwrap_or(f64::NAN);
                match f.as_str() {
                    "sin" => a.sin(),
                    "cos" => a.cos(),
                    "tan" => a.tan(),
                    "sec" => 1.0 / a.cos(),
                    "csc" => 1.0 / a.sin(),
                    "cot" => 1.0 / a.tan(),
                    "asin" => a.asin(),
                    "acos" => a.acos(),
                    "atan" => if args.len() > 1 { a.atan2(b) } else { a.atan() },
                    "sinh" => a.sinh(),
                    "cosh" => a.cosh(),
                    "tanh" => a.tanh(),
                    "sqrt" => a.sqrt(),
                    "abs" => a.abs(),
                    "ln" => a.ln(),
                    "log" => if args.len() > 1 { b.ln() / a.ln() } else { a.log10() },
                    "exp" => a.exp(),
                    "floor" => a.floor(),
                    "ceil" => a.ceil(),
                    "round" => a.round(),
                    "sign" => a.signum(),
                    "min" => a.min(b),
                    "max" => a.max(b),
                    _ => f64::NAN,
                }
            }
        }
    }

    fn vars(&self, out: &mut Vec<String>) {
        match self {
            Ex::Num(_) => {}
            Ex::Var(n) => {
                if !out.contains(n) {
                    out.push(n.clone());
                }
            }
            Ex::Neg(a) => a.vars(out),
            Ex::Bin(_, a, b) => {
                a.vars(out);
                b.vars(out);
            }
            Ex::Call(_, args) => args.iter().for_each(|a| a.vars(out)),
        }
    }
}

/// Letters that are not constants.
fn is_variable(n: &str) -> bool {
    matches!(n, "x" | "y" | "t" | "r" | "θ" | "theta" | "pi" | "π" | "tau" | "e")
}

// ── equations ──

enum Curve {
    /// y as a function of x.
    Func(Ex),
    /// A vertical line.
    Vertical(Ex),
    Param(Ex, Ex),
    Polar(Ex),
    /// A constant set to a value.
    Set(String, f64),
}

/// The part of `s` before the first `sep` that is outside brackets, and the part after it.
fn split_top(s: &str, sep: char) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => return Some((&s[..i], &s[i + c.len_utf8()..])),
            _ => {}
        }
    }
    None
}

fn curve(text: &str) -> Result<Option<Curve>, String> {
    let s = text.trim();
    if s.is_empty() {
        return Ok(None);
    }
    // (x(t), y(t))
    if s.starts_with('(') && s.ends_with(')') {
        if let Some((a, b)) = split_top(&s[1..s.len() - 1], ',') {
            return Ok(Some(Curve::Param(parse(a)?, parse(b)?)));
        }
    }
    // x = …, y = …
    if let Some((a, b)) = split_top(s, ',') {
        let side = |p: &str| p.split_once('=').map(|(l, r)| (l.trim().to_owned(), r.to_owned()));
        if let (Some((la, ra)), Some((lb, rb))) = (side(a), side(b)) {
            return match (la.as_str(), lb.as_str()) {
                ("x", "y") => Ok(Some(Curve::Param(parse(&ra)?, parse(&rb)?))),
                ("y", "x") => Ok(Some(Curve::Param(parse(&rb)?, parse(&ra)?))),
                _ => Err("write a curve as x = …, y = … (in t)".into()),
            };
        }
        return Err("write a curve as (x(t), y(t))".into());
    }
    let Some((lhs, rhs)) = s.split_once('=') else {
        return Ok(Some(Curve::Func(parse(s)?)));
    };
    let lhs = lhs.trim().replace(' ', "");
    let e = parse(rhs)?;
    let mut vars = Vec::new();
    e.vars(&mut vars);
    match lhs.as_str() {
        "y" | "f(x)" | "y(x)" | "g(x)" | "h(x)" => Ok(Some(Curve::Func(e))),
        "r" | "r(θ)" | "r(theta)" => Ok(Some(Curve::Polar(e))),
        "x" if vars.iter().all(|v| !matches!(v.as_str(), "x" | "y" | "t")) => Ok(Some(Curve::Vertical(e))),
        name if name.chars().count() == 1 && name.chars().all(char::is_alphabetic) && !is_variable(name) && vars.is_empty() => {
            Ok(Some(Curve::Set(name.to_owned(), e.eval(&|_| f64::NAN))))
        }
        _ => Err("write y = …, r = …, (x(t), y(t)), x = 3, or a = 2".into()),
    }
}

/// Each equation's curve (or the reason it cannot be read), and the constants they use.
fn curves(spec: &PlotSpec) -> (Vec<Result<Option<Curve>, String>>, Vec<String>) {
    let cs: Vec<Result<Option<Curve>, String>> = spec.eqs.iter().map(|e| curve(&e.text)).collect();
    let mut names = Vec::new();
    for c in cs.iter().flatten().flatten() {
        let mut v = Vec::new();
        match c {
            Curve::Func(e) | Curve::Vertical(e) | Curve::Polar(e) => e.vars(&mut v),
            Curve::Param(a, b) => {
                a.vars(&mut v);
                b.vars(&mut v);
            }
            Curve::Set(n, _) => v.push(n.clone()),
        }
        for n in v.into_iter().filter(|n| !is_variable(n)) {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    (cs, names)
}

/// Make sure every constant used has a slider, and that `a = 2` lines set theirs.
fn sync_params(spec: &mut PlotSpec) {
    let (cs, names) = curves(spec);
    for n in &names {
        if !spec.params.iter().any(|p| &p.name == n) {
            spec.params.push(PlotParam { name: n.clone(), value: 1.0, min: -10.0, max: 10.0 });
        }
    }
    for c in cs.iter().flatten().flatten() {
        if let Curve::Set(n, v) = c {
            if let Some(p) = spec.params.iter_mut().find(|p| &p.name == n) {
                if v.is_finite() {
                    p.value = *v as f32;
                    p.min = p.min.min(p.value);
                    p.max = p.max.max(p.value);
                }
            }
        }
    }
    spec.params.retain(|p| names.contains(&p.name));
}

// ── drawing ──

/// The plane shown in `rect`: (x min, x max, y min, y max).
fn bounds(spec: &PlotSpec, rect: Rect) -> [f64; 4] {
    let [cx, cy, w] = spec.view.map(|v| v as f64);
    let h = w * rect.height() as f64 / rect.width().max(1.0) as f64;
    [cx - w / 2.0, cx + w / 2.0, cy - h / 2.0, cy + h / 2.0]
}

/// A round step for grid lines about `target` units apart: 1, 2 or 5 times a power of ten.
fn nice_step(target: f64) -> f64 {
    let p = 10f64.powf(target.log10().floor());
    let m = target / p;
    p * if m < 1.5 { 1.0 } else if m < 3.5 { 2.0 } else if m < 7.5 { 5.0 } else { 10.0 }
}

fn label(v: f64, step: f64) -> String {
    let digits = (-step.log10().floor()).max(0.0) as usize;
    let s = format!("{v:.digits$}");
    if s == "-0" { "0".into() } else { s }
}

/// Draw the plot into `rect`: grid, axes with numbers, and each curve. `text` is the size of the
/// numbers on the axes.
pub fn paint(painter: &Painter, rect: Rect, spec: &PlotSpec, p: &Pal, text: f32) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let [x0, x1, y0, y1] = bounds(spec, rect);
    let sx = |x: f64| rect.left() + ((x - x0) / (x1 - x0)) as f32 * rect.width();
    let sy = |y: f64| rect.bottom() - ((y - y0) / (y1 - y0)) as f32 * rect.height();
    let step = nice_step((x1 - x0) / (rect.width() as f64 / 90.0).max(1.0));
    let font = FontId::proportional(text);
    // The grid: fine lines every step, the axes stronger.
    if spec.grid {
        let minor = p.hairline.gamma_multiply(0.6);
        let mut x = (x0 / step).floor() * step;
        while x <= x1 {
            painter.line_segment([pos2(sx(x), rect.top()), pos2(sx(x), rect.bottom())], Stroke::new(1.0, minor));
            x += step;
        }
        let mut y = (y0 / step).floor() * step;
        while y <= y1 {
            painter.line_segment([pos2(rect.left(), sy(y)), pos2(rect.right(), sy(y))], Stroke::new(1.0, minor));
            y += step;
        }
    }
    let axis = p.text_secondary.gamma_multiply(0.8);
    let ax_y = sy(0.0).clamp(rect.top(), rect.bottom());
    let ax_x = sx(0.0).clamp(rect.left(), rect.right());
    painter.line_segment([pos2(rect.left(), ax_y), pos2(rect.right(), ax_y)], Stroke::new(1.2, axis));
    painter.line_segment([pos2(ax_x, rect.top()), pos2(ax_x, rect.bottom())], Stroke::new(1.2, axis));
    if text >= 5.0 {
        let num = p.text_secondary;
        let mut x = (x0 / step).ceil() * step;
        while x <= x1 {
            if x.abs() > step * 0.01 {
                let below = ax_y + 3.0 + text * 1.2 < rect.bottom();
                painter.text(pos2(sx(x), if below { ax_y + 3.0 } else { ax_y - 3.0 }), if below { Align2::CENTER_TOP } else { Align2::CENTER_BOTTOM }, label(x, step), font.clone(), num);
            }
            x += step;
        }
        let mut y = (y0 / step).ceil() * step;
        while y <= y1 {
            if y.abs() > step * 0.01 {
                let left = ax_x - 4.0 > rect.left() + text * 2.5;
                painter.text(pos2(if left { ax_x - 4.0 } else { ax_x + 4.0 }, sy(y)), if left { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER }, label(y, step), font.clone(), num);
            }
            y += step;
        }
        painter.text(pos2(ax_x - 4.0, ax_y + 3.0), Align2::RIGHT_TOP, "0", font.clone(), num);
    }

    // The curves.
    let (cs, _) = curves(spec);
    let value = |n: &str, x: f64, t: f64| -> f64 {
        match n {
            "x" => x,
            "t" | "θ" | "theta" => t,
            "pi" | "π" => std::f64::consts::PI,
            "tau" => std::f64::consts::TAU,
            "e" => std::f64::consts::E,
            _ => spec.params.iter().find(|p| p.name == n).map(|p| p.value as f64).unwrap_or(f64::NAN),
        }
    };
    let width = (text / 6.0).clamp(1.2, 3.0);
    let [t0, t1] = spec.t_range.map(|v| v as f64);
    for (eq, c) in spec.eqs.iter().zip(&cs) {
        let Ok(Some(c)) = c else { continue };
        if eq.hidden {
            continue;
        }
        let color = eq.ink.color(p);
        let stroke = Stroke::new(width, color);
        // Pieces of line, broken where the curve is undefined or jumps off the view.
        let mut line: Vec<Pos2> = Vec::new();
        let flush = |line: &mut Vec<Pos2>| {
            if line.len() > 1 {
                painter.add(egui::Shape::line(std::mem::take(line), stroke));
            }
            line.clear();
        };
        let limit = rect.height() * 4.0;
        let push = |q: Option<(f64, f64)>, line: &mut Vec<Pos2>| match q {
            Some((x, y)) if x.is_finite() && y.is_finite() => {
                let s = pos2(sx(x), sy(y));
                let off = (s.y - rect.center().y).abs() > limit || (s.x - rect.center().x).abs() > rect.width() * 4.0;
                if off {
                    if let Some(last) = line.last().copied() {
                        line.push(pos2(s.x, last.y + (s.y - last.y).clamp(-limit, limit)));
                    }
                    flush(line);
                } else {
                    if let Some(last) = line.last() {
                        // A jump across most of the view is an asymptote, not a steep line.
                        if (s.y - last.y).abs() > rect.height() * 1.5 {
                            flush(line);
                        }
                    }
                    line.push(s);
                }
            }
            _ => flush(line),
        };
        match c {
            Curve::Func(e) => {
                let n = (rect.width() * 1.5) as usize;
                for i in 0..=n {
                    let x = x0 + (x1 - x0) * i as f64 / n as f64;
                    push(Some((x, e.eval(&|v| value(v, x, 0.0)))), &mut line);
                }
            }
            Curve::Vertical(e) => {
                let x = e.eval(&|v| value(v, 0.0, 0.0));
                if x.is_finite() {
                    painter.line_segment([pos2(sx(x), rect.top()), pos2(sx(x), rect.bottom())], stroke);
                }
            }
            Curve::Param(a, b) => {
                let n = 900;
                for i in 0..=n {
                    let t = t0 + (t1 - t0) * i as f64 / n as f64;
                    push(Some((a.eval(&|v| value(v, 0.0, t)), b.eval(&|v| value(v, 0.0, t)))), &mut line);
                }
            }
            Curve::Polar(e) => {
                let n = 900;
                for i in 0..=n {
                    let t = t0 + (t1 - t0) * i as f64 / n as f64;
                    let r = e.eval(&|v| value(v, 0.0, t));
                    push(Some((r * t.cos(), r * t.sin())), &mut line);
                }
            }
            Curve::Set(..) => {}
        }
        flush(&mut line);
    }
}

// ── the panel ──

/// The Plot panel: equations on the left, the graph on the right (below, when narrow). It edits the
/// selected plot on the board, or else the board's own plot that the panel keeps.
pub fn plot_ui(ui: &mut Ui, ed: &mut Editor) {
    let p = pal(ui.ctx());
    let node = ed.selection.first().copied().filter(|id| ed.selection.len() == 1 && matches!(ed.board.get(*id).map(|o| &o.kind), Some(crate::board::Kind::Plot { .. })));
    let mut spec = match node.and_then(|id| ed.board.get(id)) {
        Some(crate::board::Object { kind: crate::board::Kind::Plot { plot }, .. }) => plot.clone(),
        _ => ed.board.plot.clone(),
    };
    let before = spec.clone();
    sync_params(&mut spec);

    let full = ui.available_rect_before_wrap();
    let wide = full.width() >= 560.0;
    let list_w = if wide { (full.width() * 0.36).clamp(240.0, 340.0) } else { full.width() };
    let (list_rect, graph_rect) = if wide {
        (Rect::from_min_size(full.min, vec2(list_w, full.height())), Rect::from_min_max(pos2(full.left() + list_w + 10.0, full.top()), full.max))
    } else {
        let h = (full.height() * 0.45).max(160.0);
        (Rect::from_min_size(full.min, vec2(full.width(), h)), Rect::from_min_max(pos2(full.left(), full.top() + h + 8.0), full.max))
    };
    ui.allocate_rect(full, Sense::hover());

    // The equations, their constants, and what to do with the plot.
    let mut list = ui.new_child(egui::UiBuilder::new().max_rect(list_rect).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt("plot-list").auto_shrink([false, false]).show(&mut list, |ui| {
        if node.is_some() {
            ui.label(theme::secondary(ui, "Editing the plot selected on the board."));
        }
        theme::heading(ui, "EQUATIONS");
        let (cs, _) = curves(&spec);
        let mut remove = None;
        for (k, eq) in spec.eqs.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // The colour, which also shows or hides the curve.
                let (r, resp) = ui.allocate_exact_size(vec2(20.0, 24.0), Sense::click());
                let c = eq.ink.color(p);
                if eq.hidden {
                    ui.painter().circle_stroke(r.center(), 6.0, Stroke::new(1.5, c));
                } else {
                    ui.painter().circle_filled(r.center(), 6.0, c);
                }
                if resp.on_hover_text(if eq.hidden { "Show this curve" } else { "Hide this curve" }).clicked() {
                    eq.hidden = !eq.hidden;
                }
                let w = (ui.available_width() - 26.0).max(60.0);
                ui.add(egui::TextEdit::singleline(&mut eq.text).font(FontId::proportional(14.0)).desired_width(w).hint_text("y = x^2"));
                if ui.add(egui::Button::new(RichText::new("×").color(p.text_secondary)).frame(false).min_size(vec2(18.0, 24.0))).on_hover_text("Remove").clicked() {
                    remove = Some(k);
                }
            });
            if let Some(Err(e)) = cs.get(k) {
                if !eq.text.trim().is_empty() {
                    ui.label(RichText::new(format!("    {e}")).size(11.5).color(p.danger));
                }
            }
        }
        if let Some(k) = remove {
            spec.eqs.remove(k);
        }
        if ui.button("Add equation").clicked() {
            let ink = curve_ink(spec.eqs.len());
            spec.eqs.push(PlotEq { text: String::new(), ink, hidden: false });
        }
        ui.label(theme::secondary(ui, "y = …, r = … (polar, in θ), (cos t, sin t) or x = …, y = … (in t), x = 3, a = 2. Other letters become sliders."));

        if !spec.params.is_empty() {
            theme::heading(ui, "CONSTANTS");
            for prm in spec.params.iter_mut() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&prm.name).size(14.0).italics());
                    ui.spacing_mut().slider_width = (ui.available_width() - 150.0).max(60.0);
                    let (lo, hi) = (prm.min, prm.max);
                    ui.add(egui::Slider::new(&mut prm.value, lo..=hi).show_value(false));
                    ui.add(egui::DragValue::new(&mut prm.value).speed(0.05).max_decimals(3));
                });
                ui.horizontal(|ui| {
                    ui.label(theme::secondary(ui, "from"));
                    ui.add(egui::DragValue::new(&mut prm.min).speed(0.5));
                    ui.label(theme::secondary(ui, "to"));
                    ui.add(egui::DragValue::new(&mut prm.max).speed(0.5));
                });
                if prm.max <= prm.min {
                    prm.max = prm.min + 1.0;
                }
            }
        }
        if cs.iter().flatten().flatten().any(|c| matches!(c, Curve::Param(..) | Curve::Polar(..))) {
            theme::heading(ui, "t AND θ");
            ui.horizontal(|ui| {
                ui.label(theme::secondary(ui, "from"));
                ui.add(egui::DragValue::new(&mut spec.t_range[0]).speed(0.05));
                ui.label(theme::secondary(ui, "to"));
                ui.add(egui::DragValue::new(&mut spec.t_range[1]).speed(0.05));
                if ui.button("0 to 2π").clicked() {
                    spec.t_range = [0.0, std::f32::consts::TAU];
                }
            });
        }
        theme::heading(ui, "VIEW");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Reset view").on_hover_text("Centre on 0, 20 units across").clicked() {
                spec.view = [0.0, 0.0, 20.0];
            }
            ui.switch(&mut spec.grid, "Grid");
        });
        ui.label(theme::secondary(ui, "Drag the graph to move it; the mouse wheel zooms."));
        ui.add_space(6.0);
        if node.is_none() {
            if ui.button("Place on board").on_hover_text("A live plot item: select it to change its equations here").clicked() {
                let at = ed.paste_point();
                ed.mark_changed();
                let id = ed.board.add(Rect::from_center_size(at, vec2(480.0, 320.0)), crate::board::Kind::Plot { plot: spec.clone() }, crate::board::Style::default());
                ed.selection = vec![id];
                ed.clear_wires();
                ed.say("Placed the plot; select it to change it here");
            }
        } else if ui.button("Copy to the panel's plot").on_hover_text("Keep these equations as the panel's own").clicked() {
            ed.board.plot = spec.clone();
        }
    });

    // The graph: drag to pan, wheel to zoom about the pointer, the point under the pointer shown.
    let resp = ui.interact(graph_rect, ui.id().with("plot-graph"), Sense::click_and_drag());
    let painter = ui.painter();
    painter.rect(graph_rect, CornerRadius::same(8), p.canvas, Stroke::new(1.0, p.hairline), StrokeKind::Inside);
    let inner = graph_rect.shrink(1.0);
    if resp.dragged() {
        let d = resp.drag_delta();
        let per_px = spec.view[2] / inner.width().max(1.0);
        spec.view[0] -= d.x * per_px;
        spec.view[1] += d.y * per_px;
    }
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = (zoom * (scroll * 0.0018).exp()).clamp(0.5, 2.0);
        if (factor - 1.0).abs() > 1e-4 {
            if let Some(m) = resp.hover_pos() {
                let [x0, x1, y0, y1] = bounds(&spec, inner);
                let wx = x0 + (x1 - x0) * ((m.x - inner.left()) / inner.width()) as f64;
                let wy = y1 - (y1 - y0) * ((m.y - inner.top()) / inner.height()) as f64;
                let k = 1.0 / factor;
                spec.view[0] = (wx + (spec.view[0] as f64 - wx) * k as f64) as f32;
                spec.view[1] = (wy + (spec.view[1] as f64 - wy) * k as f64) as f32;
                spec.view[2] = (spec.view[2] * k).clamp(1e-4, 1e7);
            }
        }
    }
    paint(painter, inner, &spec, p, 11.0);
    if let Some(m) = resp.hover_pos() {
        let [x0, x1, y0, y1] = bounds(&spec, inner);
        let wx = x0 + (x1 - x0) * ((m.x - inner.left()) / inner.width()) as f64;
        let wy = y1 - (y1 - y0) * ((m.y - inner.top()) / inner.height()) as f64;
        let step = nice_step((x1 - x0) / 200.0);
        let t = format!("({}, {})", label(wx, step), label(wy, step));
        let g = painter.layout_no_wrap(t, FontId::proportional(11.5), p.text);
        let at = inner.right_top() + vec2(-g.size().x - 14.0, 8.0);
        painter.rect_filled(Rect::from_min_size(at - vec2(6.0, 3.0), g.size() + vec2(12.0, 6.0)), CornerRadius::same(6), p.window_fill);
        painter.galley(at, g, p.text);
        resp.on_hover_cursor(egui::CursorIcon::Grab);
    }

    if spec != before {
        match node {
            Some(id) => {
                // Moving the view of an item on the board is not a change worth undoing on its own.
                if spec.eqs != before.eqs || spec.params != before.params || spec.t_range != before.t_range || spec.grid != before.grid {
                    ed.mark_changed();
                }
                if let Some(crate::board::Kind::Plot { plot }) = ed.board.get_mut(id).map(|o| &mut o.kind) {
                    *plot = spec;
                }
            }
            None => ed.board.plot = spec,
        }
    }
}

/// A plot on the board: a card with the graph inside.
pub fn paint_node(painter: &Painter, r: Rect, spec: &PlotSpec, p: &Pal, z: f32, selected: bool) {
    let radius = CornerRadius::same((8.0 * z).clamp(1.0, 14.0) as u8);
    painter.rect(r, radius, p.window_fill, Stroke::new(1.0, if selected { p.accent } else { p.window_stroke }), StrokeKind::Inside);
    let inner = r.shrink(6.0 * z);
    if inner.width() > 8.0 && inner.height() > 8.0 {
        paint(painter, inner, spec, p, (11.0 * z).clamp(0.0, 20.0));
    }
    // The equations, small, at the top left.
    if z > 0.35 {
        let mut y = inner.top() + 4.0 * z;
        for eq in spec.eqs.iter().filter(|e| !e.hidden && !e.text.trim().is_empty()).take(6) {
            let g = painter.layout_no_wrap(eq.text.clone(), FontId::proportional(12.0 * z), eq.ink.color(p));
            let bg = Rect::from_min_size(pos2(inner.left() + 4.0 * z, y), g.size() + vec2(10.0, 4.0) * z);
            painter.rect_filled(bg, CornerRadius::same((4.0 * z) as u8), p.window_fill.gamma_multiply(0.85));
            painter.galley(bg.min + vec2(5.0, 2.0) * z, g, Color32::WHITE);
            y = bg.bottom() + 3.0 * z;
        }
    }
}
