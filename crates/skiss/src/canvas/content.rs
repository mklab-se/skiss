//! Laying out the text of a card: title, paragraphs, lists, tasks, quotes, code and images.
//!
//! The same layout is used to measure cards (at world scale), to draw them (at the zoom level)
//! and to place the caret while editing, so what you click is exactly what you see.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Color32, FontFamily, FontId, Galley, Stroke, Vec2, epaint::text::FontsView};
use skiss_core::board::{Line, LineKind, Span};

use super::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
pub enum Marker {
    None,
    Bullet,
    Number(String),
    Checkbox { checked: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    Table,
    Title,
    Text,
    Quote,
    Code,
    Image(String),
}

pub struct Row {
    pub galley: Arc<Galley>,
    /// Top-left of the text, relative to the content origin (scaled units).
    pub offset: Vec2,
    pub kind: RowKind,
    /// Index into the card lines (`None` for the title).
    pub line: Option<usize>,
    pub marker: Marker,
    /// Where the marker is drawn (left of `offset`).
    pub marker_x: f32,
    /// For images: the box size.
    pub size: Vec2,
    /// For tables: cell texts with offsets (relative to `offset`), and the y of each rule.
    pub cells: Vec<(Arc<Galley>, Vec2)>,
    pub rules: Vec<f32>,
}

pub struct CardText {
    pub rows: Vec<Row>,
    pub size: Vec2,
}

/// Measures images: returns the aspect ratio (height / width) if known.
pub type ImageAspect<'a> = &'a mut dyn FnMut(&str) -> Option<f32>;

pub struct TextStyle<'t> {
    pub theme: &'t Theme,
    pub scale: f32,
    pub color: Color32,
    /// Presenter size multiplier.
    pub text_scale: f32,
}

impl TextStyle<'_> {
    fn body(&self) -> f32 {
        self.theme.body_size * self.scale * self.text_scale
    }

    fn title(&self) -> f32 {
        self.theme.title_size * self.scale * self.text_scale
    }

    pub fn format(&self, size: f32, family: &FontFamily, color: Color32) -> TextFormat {
        TextFormat {
            font_id: FontId::new(size, family.clone()),
            color,
            ..TextFormat::default()
        }
    }
}

/// Builds the layout job for one run of spans.
pub fn job(
    spans: &[Span],
    st: &TextStyle<'_>,
    size: f32,
    base: &FontFamily,
    bold_base: bool,
    wrap: f32,
) -> LayoutJob {
    let theme = st.theme;
    let mut job = LayoutJob {
        wrap: TextWrapping {
            max_width: wrap.max(1.0),
            ..TextWrapping::default()
        },
        ..LayoutJob::default()
    };
    for span in spans {
        let s = span.style;
        let (family, italics) = if s.code {
            (FontFamily::Monospace, false)
        } else if s.bold || bold_base {
            (theme.bold.clone(), s.italic)
        } else if s.italic {
            match &theme.italic {
                Some(f) => (f.clone(), false),
                None => (base.clone(), true),
            }
        } else {
            (base.clone(), false)
        };
        let font_size = if s.code { size * 0.8 } else { size };
        let mut fmt = TextFormat {
            font_id: FontId::new(font_size, family),
            color: st.color,
            italics,
            ..TextFormat::default()
        };
        if s.code {
            fmt.background = if theme.dark {
                Color32::from_rgba_unmultiplied(255, 255, 255, 18)
            } else {
                Color32::from_rgba_unmultiplied(60, 40, 20, 22)
            };
        }
        if span.link.is_some() {
            fmt.color = theme.link;
            fmt.underline = Stroke::new((size / 16.0).max(0.6), theme.link.gamma_multiply(0.6));
        }
        if s.strike {
            fmt.strikethrough = Stroke::new((size / 14.0).max(0.7), st.color);
            fmt.color = fmt.color.gamma_multiply(0.55);
        }
        job.append(&span.text, 0.0, fmt);
    }
    if spans.is_empty() {
        job.append("", 0.0, st.format(size, base, st.color));
    }
    job
}

/// Lays out a card's title and lines within `width` (scaled units).
pub fn layout(
    fonts: &mut FontsView<'_>,
    st: &TextStyle<'_>,
    title: Option<&[Span]>,
    lines: &[Line],
    width: f32,
    aspect: ImageAspect<'_>,
) -> CardText {
    let theme = st.theme;
    let body = st.body();
    let mut rows = Vec::new();
    let mut y = 0.0f32;
    let mut prev_list = false;
    if let Some(title) = title {
        let g = fonts.layout_job(job(title, st, st.title(), &theme.title, false, width));
        let h = g.size().y;
        rows.push(Row {
            galley: g,
            offset: Vec2::new(0.0, 0.0),
            kind: RowKind::Title,
            line: None,
            marker: Marker::None,
            marker_x: 0.0,
            size: Vec2::ZERO,
            cells: Vec::new(),
            rules: Vec::new(),
        });
        y += h;
        if !lines.is_empty() {
            y += body * 0.3;
        }
    }
    let indent_step = body * 0.9;
    let mut number_width = 0.0f32;
    for (i, line) in lines.iter().enumerate() {
        if let LineKind::Html { text, image } = &line.kind
            && text.is_empty()
            && image.as_deref().and_then(&mut *aspect).is_none()
        {
            continue;
        }
        let is_list = line.is_list();
        if i > 0 {
            y += if is_list && prev_list {
                body * 0.12
            } else {
                body * 0.45
            };
        }
        prev_list = is_list;
        let depth_x = line.depth as f32 * indent_step;
        let (marker, marker_w) = match &line.kind {
            LineKind::Bullet => (Marker::Bullet, body * 0.85),
            LineKind::Numbered(n) => {
                let w = body * (0.6 + 0.5 * n.to_string().len() as f32);
                number_width = number_width.max(w);
                (Marker::Number(format!("{n}.")), w)
            }
            LineKind::Task(done) => (Marker::Checkbox { checked: *done }, body * 1.15),
            _ => (Marker::None, 0.0),
        };
        let x = depth_x + marker_w;
        let avail = (width - x).max(body * 2.0);
        match &line.kind {
            LineKind::Image { url, .. } => {
                let ratio = aspect(url).unwrap_or(0.6).clamp(0.1, 4.0);
                let w = width;
                let h = w * ratio;
                let g = fonts.layout_job(job(&[], st, body, &theme.body, false, w));
                rows.push(Row {
                    galley: g,
                    offset: Vec2::new(0.0, y),
                    kind: RowKind::Image(url.clone()),
                    line: Some(i),
                    marker: Marker::None,
                    marker_x: 0.0,
                    size: Vec2::new(w, h),
                    cells: Vec::new(),
                    rules: Vec::new(),
                });
                y += h;
            }
            LineKind::Html { text, image } => {
                if let Some(url) = image
                    && let Some(ratio) = aspect(url)
                {
                    let w = width
                        .min(width * 0.9 / ratio.max(0.2))
                        .max(body * 4.0)
                        .min(width);
                    let h = w * ratio.clamp(0.1, 4.0);
                    let g = fonts.layout_job(job(&[], st, body, &theme.body, false, w));
                    rows.push(Row {
                        galley: g,
                        offset: Vec2::new((width - w) / 2.0, y),
                        kind: RowKind::Image(url.clone()),
                        line: Some(i),
                        marker: Marker::None,
                        marker_x: 0.0,
                        size: Vec2::new(w, h),
                        cells: Vec::new(),
                        rules: Vec::new(),
                    });
                    y += h;
                    if !text.is_empty() {
                        y += body * 0.4;
                    }
                }
                if !text.is_empty() {
                    let g = fonts.layout_job(job(
                        &[Span::plain(text.clone())],
                        st,
                        body,
                        &theme.body,
                        false,
                        avail,
                    ));
                    let h = g.size().y;
                    rows.push(Row {
                        galley: g,
                        offset: Vec2::new(x, y),
                        kind: RowKind::Text,
                        line: Some(i),
                        marker: Marker::None,
                        marker_x: 0.0,
                        size: Vec2::new(avail, h),
                        cells: Vec::new(),
                        rules: Vec::new(),
                    });
                    y += h;
                }
            }
            LineKind::Table => {
                let row = table(fonts, st, &line.text(), avail, body);
                let h = row.size.y;
                rows.push(Row {
                    offset: Vec2::new(x, y),
                    line: Some(i),
                    ..row
                });
                y += h;
            }
            LineKind::Code(_) | LineKind::Raw => {
                let text = line.text();
                let fmt = st.format(body * 0.78, &FontFamily::Monospace, st.color);
                let mut j = LayoutJob::single_section(text, fmt);
                j.wrap.max_width = (avail - body * 0.8).max(1.0);
                let g = fonts.layout_job(j);
                let h = g.size().y;
                rows.push(Row {
                    galley: g,
                    offset: Vec2::new(x + body * 0.4, y + body * 0.3),
                    kind: RowKind::Code,
                    line: Some(i),
                    marker: Marker::None,
                    marker_x: 0.0,
                    size: Vec2::new(avail, h + body * 0.6),
                    cells: Vec::new(),
                    rules: Vec::new(),
                });
                y += h + body * 0.6;
            }
            LineKind::Quote => {
                let bar = body * 0.6;
                let mut q = TextStyle {
                    theme,
                    scale: st.scale,
                    color: theme.muted,
                    text_scale: st.text_scale,
                };
                q.color = theme.muted;
                let g =
                    fonts.layout_job(job(&line.spans, &q, body, &theme.body, false, avail - bar));
                let h = g.size().y;
                rows.push(Row {
                    galley: g,
                    offset: Vec2::new(x + bar, y),
                    kind: RowKind::Quote,
                    line: Some(i),
                    marker: Marker::None,
                    marker_x: x,
                    size: Vec2::new(avail, h),
                    cells: Vec::new(),
                    rules: Vec::new(),
                });
                y += h;
            }
            _ => {
                let done = matches!(line.kind, LineKind::Task(true));
                let mut sub = TextStyle {
                    theme,
                    scale: st.scale,
                    color: st.color,
                    text_scale: st.text_scale,
                };
                if done {
                    sub.color = theme.muted;
                }
                let spans: Vec<Span>;
                let use_spans = if done {
                    spans = line
                        .spans
                        .iter()
                        .map(|s| {
                            let mut s = s.clone();
                            s.style.strike = true;
                            s
                        })
                        .collect();
                    &spans[..]
                } else {
                    &line.spans[..]
                };
                let g = fonts.layout_job(job(use_spans, &sub, body, &theme.body, false, avail));
                let h = g.size().y;
                rows.push(Row {
                    galley: g,
                    offset: Vec2::new(x, y),
                    kind: RowKind::Text,
                    line: Some(i),
                    marker,
                    marker_x: depth_x,
                    size: Vec2::new(avail, h),
                    cells: Vec::new(),
                    rules: Vec::new(),
                });
                y += h;
            }
        }
    }
    let _ = number_width;
    CardText {
        rows,
        size: Vec2::new(width, y),
    }
}

fn is_rule_row(l: &str) -> bool {
    let t = l.trim();
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
}

fn cells_of(l: &str) -> Vec<String> {
    let t = l.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut esc = false;
    for c in t.chars() {
        if esc {
            cur.push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
            cur.push(c);
        } else if c == '|' {
            out.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// A Markdown table laid out as a grid within `avail`.
fn table(fonts: &mut FontsView<'_>, st: &TextStyle<'_>, src: &str, avail: f32, body: f32) -> Row {
    let theme = st.theme;
    let size = body * 0.82;
    let rows: Vec<Vec<String>> = src
        .lines()
        .filter(|l| !l.trim().is_empty() && !is_rule_row(l))
        .map(cells_of)
        .collect();
    let ncols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let pad = body * 0.3;
    let spans = |text: &str, header: bool| -> Vec<Span> {
        let mut s = skiss_core::board::inline::parse_inline(text);
        if header {
            for x in &mut s {
                x.style.bold = true;
            }
        }
        s
    };
    // Natural widths, then scale the columns to fit.
    let mut natural = vec![body * 1.5; ncols];
    for (r, row) in rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let g = fonts.layout_job(job(
                &spans(cell, r == 0),
                st,
                size,
                &theme.body,
                false,
                f32::INFINITY,
            ));
            natural[c] = natural[c].max(g.size().x + pad * 2.0);
        }
    }
    let total: f32 = natural.iter().sum();
    let scale = if total > avail { avail / total } else { 1.0 };
    let widths: Vec<f32> = natural
        .iter()
        .map(|w| (w * scale).max(body * 1.5))
        .collect();
    let mut cells = Vec::new();
    let mut rules = Vec::new();
    let mut y = 0.0;
    for (r, row) in rows.iter().enumerate() {
        let mut x = 0.0;
        let mut h = 0.0f32;
        for (c, w) in widths.iter().enumerate() {
            let text = row.get(c).map(String::as_str).unwrap_or("");
            let g = fonts.layout_job(job(
                &spans(text, r == 0),
                st,
                size,
                &theme.body,
                false,
                (w - pad * 2.0).max(4.0),
            ));
            h = h.max(g.size().y);
            cells.push((g, Vec2::new(x + pad, y + pad)));
            x += w;
        }
        y += h + pad * 2.0;
        rules.push(y);
    }
    let g = fonts.layout_job(job(&[], st, size, &theme.body, false, 1.0));
    Row {
        galley: g,
        offset: Vec2::ZERO,
        kind: RowKind::Table,
        line: None,
        marker: Marker::None,
        marker_x: 0.0,
        size: Vec2::new(widths.iter().sum::<f32>().min(avail), y),
        cells,
        rules,
    }
}
