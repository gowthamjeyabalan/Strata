//! A small markdown reader and painter for notes: headings, paragraphs, bold, italic, strikethrough,
//! inline code and code blocks, quotes, rules, bullet, numbered and task lists, pipe tables, links and
//! `[[Name]]` links to other notes or items. Painted with the Grapht type, at any zoom.

use crate::theme::{self, Pal};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    Url(String),
    /// `[[Name]]`: a note or item with that name.
    Wiki(String),
}

#[derive(Clone, Debug, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<Link>,
    /// A `[[Name]]` link to a note that does not exist yet (following it makes the note).
    pub missing: bool,
}

#[derive(Clone, Debug)]
pub enum Marker {
    Bullet,
    Number(String),
    /// A task; `line` is its line in the source, so ticking it can change the text.
    Task { done: bool, line: usize },
}

#[derive(Clone, Debug)]
pub enum Block {
    Heading(u8, Vec<Span>),
    Para(Vec<Span>),
    Item { indent: usize, marker: Marker, spans: Vec<Span> },
    Quote(Vec<Span>),
    Code(String),
    Rule,
    Table(Vec<Vec<Vec<Span>>>),
}

/// Bold, italic, strikethrough, code and links inside a line.
pub fn inline(s: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let (mut bold, mut italic, mut strike) = (false, false, false);
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    let flush = |cur: &mut String, out: &mut Vec<Span>, bold: bool, italic: bool, strike: bool| {
        if !cur.is_empty() {
            out.push(Span { text: std::mem::take(cur), bold, italic, strike, ..Default::default() });
        }
    };
    let find = |from: usize, pat: &str| -> Option<usize> {
        let p: Vec<char> = pat.chars().collect();
        (from..chars.len()).find(|&k| chars[k..].starts_with(&p))
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let rest = &chars[i..];
        if c == '\\' && i + 1 < chars.len() {
            cur.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(end) = find(i + 1, "`") {
                flush(&mut cur, &mut out, bold, italic, strike);
                out.push(Span { text: chars[i + 1..end].iter().collect(), code: true, ..Default::default() });
                i = end + 1;
                continue;
            }
        }
        if rest.starts_with(&['[', '[']) {
            if let Some(end) = find(i + 2, "]]") {
                flush(&mut cur, &mut out, bold, italic, strike);
                let name: String = chars[i + 2..end].iter().collect();
                out.push(Span { text: name.clone(), bold, italic, link: Some(Link::Wiki(name.trim().to_owned())), ..Default::default() });
                i = end + 2;
                continue;
            }
        }
        if c == '[' {
            if let Some(close) = find(i + 1, "](") {
                if let Some(end) = find(close + 2, ")") {
                    flush(&mut cur, &mut out, bold, italic, strike);
                    let text: String = chars[i + 1..close].iter().collect();
                    let url: String = chars[close + 2..end].iter().collect();
                    out.push(Span { text, bold, italic, link: Some(Link::Url(url.trim().to_owned())), ..Default::default() });
                    i = end + 1;
                    continue;
                }
            }
        }
        if rest.starts_with(&['~', '~']) {
            flush(&mut cur, &mut out, bold, italic, strike);
            strike = !strike;
            i += 2;
            continue;
        }
        if rest.starts_with(&['*', '*']) || rest.starts_with(&['_', '_']) {
            flush(&mut cur, &mut out, bold, italic, strike);
            bold = !bold;
            i += 2;
            continue;
        }
        // A single * or _ marks italics, but not inside a word for _ (snake_case stays as it is).
        let word_inside = c == '_' && i > 0 && chars[i - 1].is_alphanumeric() && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric());
        if (c == '*' || c == '_') && !word_inside {
            flush(&mut cur, &mut out, bold, italic, strike);
            italic = !italic;
            i += 1;
            continue;
        }
        cur.push(c);
        i += 1;
    }
    flush(&mut cur, &mut out, bold, italic, strike);
    out
}

fn table_cells(line: &str) -> Vec<String> {
    let t = line.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().to_owned()).collect()
}

fn is_table_rule(line: &str) -> bool {
    let t = line.trim();
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Split a note into blocks.
pub fn parse(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let end_para = |para: &mut Vec<String>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Para(inline(&para.join(" "))));
            para.clear();
        }
    };
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        let line = raw.trim_end();
        let t = line.trim_start();
        let indent = (line.len() - t.len()) / 2;
        if t.is_empty() {
            end_para(&mut para, &mut out);
            i += 1;
            continue;
        }
        if t.starts_with("```") {
            end_para(&mut para, &mut out);
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            out.push(Block::Code(code.join("\n")));
            i += 1;
            continue;
        }
        if t.starts_with('#') {
            let level = t.chars().take_while(|c| *c == '#').count();
            if level <= 6 && t[level..].starts_with(' ') {
                end_para(&mut para, &mut out);
                out.push(Block::Heading(level as u8, inline(t[level..].trim())));
                i += 1;
                continue;
            }
        }
        if t.len() >= 3 && (t.chars().all(|c| c == '-' || c == ' ') || t.chars().all(|c| c == '*' || c == ' ') || t.chars().all(|c| c == '_' || c == ' ')) && t.chars().filter(|c| !c.is_whitespace()).count() >= 3 {
            end_para(&mut para, &mut out);
            out.push(Block::Rule);
            i += 1;
            continue;
        }
        if let Some(q) = t.strip_prefix('>') {
            end_para(&mut para, &mut out);
            out.push(Block::Quote(inline(q.trim())));
            i += 1;
            continue;
        }
        // A pipe table: a header row, a rule row, then body rows.
        if t.starts_with('|') && i + 1 < lines.len() && is_table_rule(lines[i + 1]) {
            end_para(&mut para, &mut out);
            let mut rows = vec![table_cells(t).iter().map(|c| inline(c)).collect::<Vec<_>>()];
            i += 2;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                rows.push(table_cells(lines[i]).iter().map(|c| inline(c)).collect());
                i += 1;
            }
            out.push(Block::Table(rows));
            continue;
        }
        let bullet = ["- ", "* ", "+ "].iter().find_map(|b| t.strip_prefix(b));
        if let Some(rest) = bullet {
            end_para(&mut para, &mut out);
            let marker_task = |r: &str| -> Option<(bool, String)> {
                if let Some(x) = r.strip_prefix("[ ] ") {
                    Some((false, x.to_owned()))
                } else {
                    r.strip_prefix("[x] ").or_else(|| r.strip_prefix("[X] ")).map(|x| (true, x.to_owned()))
                }
            };
            let (marker, body) = match marker_task(rest) {
                Some((done, body)) => (Marker::Task { done, line: i }, body),
                None => (Marker::Bullet, rest.to_owned()),
            };
            out.push(Block::Item { indent, marker, spans: inline(&body) });
            i += 1;
            continue;
        }
        let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && t[digits..].starts_with(". ") {
            end_para(&mut para, &mut out);
            out.push(Block::Item { indent, marker: Marker::Number(t[..digits].to_owned()), spans: inline(&t[digits + 2..]) });
            i += 1;
            continue;
        }
        para.push(t.to_owned());
        i += 1;
    }
    end_para(&mut para, &mut out);
    out
}

/// Tick or untick the task on `line` of `text`.
pub fn toggle_task(text: &str, line: usize) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if let Some(l) = lines.get_mut(line) {
        if let Some(k) = l.find("[ ]") {
            l.replace_range(k..k + 3, "[x]");
        } else if let Some(k) = l.find("[x]").or_else(|| l.find("[X]")) {
            l.replace_range(k..k + 3, "[ ]");
        }
    }
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Does pasted text look like markdown (rather than plain words)?
pub fn looks_like_markdown(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().map(str::trim_start).collect();
    let marked = lines
        .iter()
        .filter(|l| l.starts_with("# ") || l.starts_with("## ") || l.starts_with("### ") || l.starts_with("- ") || l.starts_with("* ") || l.starts_with("> ") || l.starts_with("```") || l.starts_with('|') || l.starts_with("- [") || l.chars().take_while(|c| c.is_ascii_digit()).count() > 0 && l.contains(". "))
        .count();
    marked >= 1 && (lines.len() >= 2 || text.contains("**") || text.contains("[["))
}

/// Something on a painted note that can be clicked.
#[derive(Clone, Debug)]
pub enum Target {
    Task(usize),
    Link(Link),
}

/// Paint `blocks` from `top_left`, `width` wide, at `scale` (1 in panels, the zoom on the board).
/// Returns the height used and the clickable places.
pub fn paint(painter: &Painter, blocks: &[Block], top_left: Pos2, width: f32, scale: f32, p: &Pal, ink: Color32) -> (f32, Vec<(Rect, Target)>) {
    let mut y = top_left.y;
    let x = top_left.x;
    let mut hits = Vec::new();
    let body = 13.0 * scale;
    let gap = 6.0 * scale;
    let visible = painter.clip_rect();
    for (k, b) in blocks.iter().enumerate() {
        if k > 0 {
            y += gap;
        }
        match b {
            Block::Heading(level, spans) => {
                let size = match level { 1 => 21.0, 2 => 17.5, 3 => 15.0, _ => 13.5 } * scale;
                if *level <= 2 && k > 0 {
                    y += gap * 0.5;
                }
                let h = text(painter, spans, pos2(x, y), width, size, true, ink, p, &mut hits, visible);
                y += h;
                if *level == 1 {
                    y += 3.0 * scale;
                    painter.line_segment([pos2(x, y), pos2(x + width, y)], Stroke::new(1.0, p.hairline));
                    y += 2.0 * scale;
                }
            }
            Block::Para(spans) => y += text(painter, spans, pos2(x, y), width, body, false, ink, p, &mut hits, visible),
            Block::Quote(spans) => {
                let h = text(painter, spans, pos2(x + 12.0 * scale, y), width - 12.0 * scale, body, false, p.text_secondary, p, &mut hits, visible);
                painter.rect_filled(Rect::from_min_size(pos2(x, y), vec2(3.0 * scale, h)), CornerRadius::same(2), p.accent.gamma_multiply(0.6));
                y += h;
            }
            Block::Item { indent, marker, spans } => {
                let left = x + (*indent as f32 * 16.0 + 18.0) * scale;
                let line_h = body * 1.35;
                let mid = y + line_h * 0.5;
                let mx = left - 10.0 * scale;
                match marker {
                    Marker::Bullet => {
                        painter.circle_filled(pos2(mx, mid), 2.2 * scale, ink);
                    }
                    Marker::Number(n) => {
                        painter.text(pos2(mx + 4.0 * scale, y), egui::Align2::RIGHT_TOP, format!("{n}."), FontId::proportional(body), p.text_secondary);
                    }
                    Marker::Task { done, line } => {
                        let r = Rect::from_center_size(pos2(mx - 1.0 * scale, mid), vec2(12.0, 12.0) * scale);
                        if *done {
                            painter.rect_filled(r, CornerRadius::same((3.0 * scale) as u8), p.accent);
                            let c = r.center();
                            let s = r.width() / 12.0;
                            painter.add(egui::Shape::line(vec![c + vec2(-3.0, 0.0) * s, c + vec2(-1.0, 2.5) * s, c + vec2(3.5, -2.5) * s], Stroke::new(1.6 * s, Color32::WHITE)));
                        } else {
                            painter.rect_stroke(r, CornerRadius::same((3.0 * scale) as u8), Stroke::new(1.2, p.text_secondary), StrokeKind::Inside);
                        }
                        hits.push((r.expand(3.0), Target::Task(*line)));
                    }
                }
                let done = matches!(marker, Marker::Task { done: true, .. });
                let spans: Vec<Span> = if done { spans.iter().cloned().map(|mut s| { s.strike = true; s }).collect() } else { spans.clone() };
                let color = if done { p.text_secondary } else { ink };
                y += text(painter, &spans, pos2(left, y), x + width - left, body, false, color, p, &mut hits, visible);
            }
            Block::Code(code) => {
                let font = FontId::monospace(12.0 * scale);
                let g = painter.layout(code.clone(), font, ink, (width - 16.0 * scale).max(10.0));
                let r = Rect::from_min_size(pos2(x, y), vec2(width, g.size().y + 12.0 * scale));
                painter.rect_filled(r, CornerRadius::same((6.0 * scale).min(255.0) as u8), p.well);
                painter.galley(pos2(x + 8.0 * scale, y + 6.0 * scale), g, ink);
                y += r.height();
            }
            Block::Rule => {
                y += 4.0 * scale;
                painter.line_segment([pos2(x, y), pos2(x + width, y)], Stroke::new(1.0, p.hairline));
                y += 4.0 * scale;
            }
            Block::Table(rows) => {
                let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
                let cw = width / cols as f32;
                let pad = 6.0 * scale;
                for (ri, row) in rows.iter().enumerate() {
                    // Lay out the row first to know its height.
                    let jobs: Vec<_> = (0..cols)
                        .map(|c| {
                            let spans = row.get(c).cloned().unwrap_or_default();
                            painter.layout_job(job(&spans, (cw - pad * 2.0).max(8.0), body, ri == 0, ink, p))
                        })
                        .collect();
                    let h = jobs.iter().map(|g| g.size().y).fold(body * 1.3, f32::max) + pad * 2.0;
                    let rr = Rect::from_min_size(pos2(x, y), vec2(width, h));
                    if ri == 0 {
                        painter.rect_filled(rr, CornerRadius::ZERO, p.well);
                    }
                    for (c, g) in jobs.into_iter().enumerate() {
                        painter.galley(pos2(x + c as f32 * cw + pad, y + pad), g, ink);
                        if c > 0 {
                            painter.line_segment([pos2(x + c as f32 * cw, y), pos2(x + c as f32 * cw, y + h)], Stroke::new(1.0, p.hairline));
                        }
                    }
                    painter.line_segment([pos2(x, y + h), pos2(x + width, y + h)], Stroke::new(1.0, p.hairline));
                    y += h;
                }
            }
        }
        if y > visible.bottom() + 4000.0 {
            break;
        }
    }
    (y - top_left.y, hits)
}

fn job(spans: &[Span], width: f32, size: f32, strong: bool, ink: Color32, p: &Pal) -> LayoutJob {
    let mut job = LayoutJob { halign: Align::LEFT, ..Default::default() };
    job.wrap.max_width = width;
    for s in spans {
        let family = if s.code {
            FontFamily::Monospace
        } else if s.bold || strong {
            theme::semibold_family()
        } else {
            FontFamily::Proportional
        };
        // Links are blue; a link to a note that does not exist yet is a paler blue.
        let color = match &s.link {
            Some(_) if s.missing => p.accent.gamma_multiply(0.55),
            Some(_) => p.accent,
            None => ink,
        };
        let mut f = TextFormat::simple(FontId::new(if s.code { size * 0.92 } else { size }, family), color);
        f.italics = s.italic;
        if s.strike {
            f.strikethrough = Stroke::new((size / 13.0).max(0.6), color);
        }
        if s.code {
            f.background = p.well;
        }
        if s.link.is_some() {
            f.underline = Stroke::new((size / 14.0).max(0.6), color.gamma_multiply(0.6));
        }
        job.append(&s.text, 0.0, f);
    }
    job
}

/// Lay out and paint one run of text; returns its height. Links become click targets.
#[allow(clippy::too_many_arguments)]
fn text(painter: &Painter, spans: &[Span], at: Pos2, width: f32, size: f32, strong: bool, ink: Color32, p: &Pal, hits: &mut Vec<(Rect, Target)>, visible: Rect) -> f32 {
    if spans.is_empty() {
        return size * 1.3;
    }
    let g = painter.layout_job(job(spans, width.max(8.0), size, strong, ink, p));
    let h = g.size().y;
    if Rect::from_min_size(at, g.size()).intersects(visible) && spans.iter().any(|s| s.link.is_some()) {
        // Which characters belong to which link, then where their glyphs landed.
        let mut owner: Vec<Option<usize>> = Vec::new();
        for (k, s) in spans.iter().enumerate() {
            owner.extend(std::iter::repeat_n(s.link.as_ref().map(|_| k), s.text.chars().count()));
        }
        let chars: Vec<char> = spans.iter().flat_map(|s| s.text.chars()).collect();
        let mut ci = 0;
        let mut rects: Vec<(usize, Rect)> = Vec::new();
        for row in &g.rows {
            for gl in &row.glyphs {
                while ci < chars.len() && chars[ci] != gl.chr {
                    ci += 1;
                }
                if ci >= chars.len() {
                    break;
                }
                if let Some(k) = owner[ci] {
                    let r = gl.logical_rect().translate(row.pos.to_vec2() + at.to_vec2());
                    match rects.last_mut() {
                        Some((last, lr)) if *last == k && (lr.center().y - r.center().y).abs() < 1.0 => *lr = lr.union(r),
                        _ => rects.push((k, r)),
                    }
                }
                ci += 1;
            }
        }
        for (k, r) in rects {
            if let Some(l) = &spans[k].link {
                hits.push((r, Target::Link(l.clone())));
            }
        }
    }
    painter.galley(at, g, ink);
    h
}

/// The `[[Name]]` links in a note's text, as written.
pub fn wiki_links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(a) = rest.find("[[") {
        let after = &rest[a + 2..];
        let Some(b) = after.find("]]") else { break };
        let name = after[..b].trim();
        if !name.is_empty() && !name.contains('\n') {
            out.push(name.to_owned());
        }
        rest = &after[b + 2..];
    }
    out
}

/// Mark `[[Name]]` links whose target `known` does not recognise, so they show as not made yet.
pub fn mark_missing(blocks: &mut [Block], known: &dyn Fn(&str) -> bool) {
    let fix = |spans: &mut Vec<Span>| {
        for s in spans.iter_mut() {
            if let Some(Link::Wiki(name)) = &s.link {
                s.missing = !known(name);
            }
        }
    };
    for b in blocks.iter_mut() {
        match b {
            Block::Heading(_, s) | Block::Para(s) | Block::Quote(s) | Block::Item { spans: s, .. } => fix(s),
            Block::Table(rows) => {
                for row in rows.iter_mut() {
                    for cell in row.iter_mut() {
                        fix(cell);
                    }
                }
            }
            _ => {}
        }
    }
}
