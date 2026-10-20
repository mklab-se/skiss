//! Inline rich text: runs of text with bold, italic, code, strike and links.
//!
//! skiss never shows Markdown syntax, so card text lives as [`Span`]s while it is displayed and
//! edited, and is turned back into Markdown only when it is written to the file.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Character formatting of a run of text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
}

/// A run of text with one style and an optional link target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    pub link: Option<String>,
}

impl Span {
    pub fn plain(text: impl Into<String>) -> Self {
        Span {
            text: text.into(),
            style: Style::default(),
            link: None,
        }
    }
}

/// The text of some spans without any formatting.
pub fn plain_text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

/// Parses inline Markdown (one paragraph's worth) into spans. Line breaks become spaces.
pub fn parse_inline(md: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut style = Style::default();
    let mut bold = 0u32;
    let mut italic = 0u32;
    let mut strike = 0u32;
    let mut links: Vec<String> = Vec::new();
    let mut image_depth = 0u32;
    let opts = Options::ENABLE_STRIKETHROUGH;
    for event in Parser::new_ext(md, opts) {
        match event {
            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold = bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::End(TagEnd::Emphasis) => italic = italic.saturating_sub(1),
            Event::Start(Tag::Strikethrough) => strike += 1,
            Event::End(TagEnd::Strikethrough) => strike = strike.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) => links.push(dest_url.to_string()),
            Event::End(TagEnd::Link) => {
                links.pop();
            }
            Event::Start(Tag::Image { .. }) => image_depth += 1,
            Event::End(TagEnd::Image) => image_depth = image_depth.saturating_sub(1),
            Event::Start(Tag::Paragraph) | Event::Start(Tag::Heading { .. }) => {
                if !spans.is_empty() {
                    push(&mut spans, " ", Style::default(), None);
                }
            }
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => {
                style.bold = bold > 0;
                style.italic = italic > 0;
                style.strike = strike > 0;
                style.code = false;
                push(&mut spans, &t, style, links.last().cloned());
            }
            Event::Code(t) => {
                style.bold = bold > 0;
                style.italic = italic > 0;
                style.strike = strike > 0;
                style.code = true;
                push(&mut spans, &t, style, links.last().cloned());
            }
            Event::SoftBreak | Event::HardBreak => {
                style.code = false;
                push(&mut spans, " ", style, links.last().cloned());
            }
            _ => {}
        }
    }
    let _ = image_depth;
    spans
}

fn push(spans: &mut Vec<Span>, text: &str, style: Style, link: Option<String>) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.style == style
        && last.link == link
    {
        last.text.push_str(text);
        return;
    }
    spans.push(Span {
        text: text.to_string(),
        style,
        link,
    });
}

/// Merges neighbouring spans that look the same and drops empty ones.
pub fn normalize(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    for s in spans {
        push(&mut out, &s.text, s.style, s.link);
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flag {
    Strike,
    Bold,
    Italic,
}

impl Flag {
    fn delim(self) -> &'static str {
        match self {
            Flag::Strike => "~~",
            Flag::Bold => "**",
            Flag::Italic => "*",
        }
    }
}

fn wanted(style: Style) -> Vec<Flag> {
    let mut v = Vec::new();
    if style.strike {
        v.push(Flag::Strike);
    }
    if style.bold {
        v.push(Flag::Bold);
    }
    if style.italic {
        v.push(Flag::Italic);
    }
    v
}

/// Writes spans back as inline Markdown, using the most common syntax (`**bold**`, `*italic*`).
pub fn to_markdown(spans: &[Span]) -> String {
    let spans = normalize(spans.to_vec());
    let mut out = String::new();
    let mut open: Vec<Flag> = Vec::new();
    let mut link: Option<String> = None;
    let mut pending_ws = String::new();

    let close_all = |out: &mut String, open: &mut Vec<Flag>| {
        while let Some(f) = open.pop() {
            out.push_str(f.delim());
        }
    };

    for span in &spans {
        let core = span.text.trim();
        if core.is_empty() {
            pending_ws.push_str(&span.text);
            continue;
        }
        let lead = &span.text[..span.text.len() - span.text.trim_start().len()];
        let trail = &span.text[span.text.trim_end().len()..];

        if span.link != link {
            close_all(&mut out, &mut open);
            if let Some(url) = link.take() {
                out.push_str("](");
                out.push_str(&url);
                out.push(')');
            }
        }
        let want = wanted(span.style);
        let keep = open
            .iter()
            .zip(want.iter())
            .take_while(|(a, b)| a == b)
            .count();
        while open.len() > keep {
            let f = open.pop().expect("non-empty");
            out.push_str(f.delim());
        }
        out.push_str(&pending_ws);
        pending_ws.clear();
        out.push_str(lead);
        if span.link.is_some() && link.is_none() {
            out.push('[');
            link = span.link.clone();
        }
        for f in &want[keep..] {
            out.push_str(f.delim());
            open.push(*f);
        }
        if span.style.code {
            let ticks = if core.contains('`') { "`` " } else { "`" };
            let close = if core.contains('`') { " ``" } else { "`" };
            out.push_str(ticks);
            out.push_str(core);
            out.push_str(close);
        } else {
            out.push_str(&escape(core));
        }
        pending_ws.push_str(trail);
    }
    close_all(&mut out, &mut open);
    if let Some(url) = link {
        out.push_str("](");
        out.push_str(&url);
        out.push(')');
    }
    out.push_str(&pending_ws);
    out
}

/// Escapes the characters that would otherwise turn plain text into formatting.
fn escape(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let prev = if i > 0 { Some(chars[i - 1]) } else { None };
        let next = chars.get(i + 1).copied();
        match c {
            '\\' | '*' | '`' => {
                out.push('\\');
                out.push(c);
            }
            '~' if next == Some('~') || prev == Some('~') => {
                out.push('\\');
                out.push(c);
            }
            // Underscores inside words never format, so only escape them at word edges.
            '_' if !(prev.is_some_and(char::is_alphanumeric)
                && next.is_some_and(char::is_alphanumeric)) =>
            {
                out.push('\\');
                out.push(c);
            }
            '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str, bold: bool, italic: bool) -> Span {
        Span {
            text: text.into(),
            style: Style {
                bold,
                italic,
                ..Style::default()
            },
            link: None,
        }
    }

    #[test]
    fn parses_basic_formatting() {
        let spans = parse_inline("Hello **bold** and *it* `code`");
        assert_eq!(plain_text(&spans), "Hello bold and it code");
        assert!(spans[1].style.bold);
        assert!(spans[3].style.italic);
        assert!(spans.last().unwrap().style.code);
    }

    #[test]
    fn parses_links() {
        let spans = parse_inline("Manages [Erik Lund](#erik-lund)");
        assert_eq!(spans[1].text, "Erik Lund");
        assert_eq!(spans[1].link.as_deref(), Some("#erik-lund"));
    }

    #[test]
    fn round_trips_common_markdown() {
        for md in [
            "Hello **bold** and *it* `code`",
            "Manages [Erik Lund](#erik-lund) and [Sara](#sara-ek)",
            "~~gone~~ but **bold *and italic***",
            "snake_case stays plain",
            "Price is 5\\*3",
            "A [**bold link**](https://x.se) here",
        ] {
            let spans = parse_inline(md);
            let back = to_markdown(&spans);
            assert_eq!(parse_inline(&back), spans, "{md} -> {back}");
        }
    }

    #[test]
    fn whitespace_stays_outside_delimiters() {
        let md = to_markdown(&[s("a ", true, false), s("b", false, false)]);
        assert_eq!(md, "**a** b");
        let md = to_markdown(&[s("a", false, false), s(" b", false, true)]);
        assert_eq!(md, "a *b*");
    }

    #[test]
    fn nested_styles_close_in_order() {
        let md = to_markdown(&[
            s("a ", true, false),
            s("b", true, true),
            s(" c", false, false),
        ]);
        assert_eq!(md, "**a *b*** c");
        assert_eq!(
            parse_inline(&md),
            vec![
                s("a ", true, false),
                s("b", true, true),
                s(" c", false, false)
            ]
        );
    }

    #[test]
    fn escapes_markup_characters() {
        assert_eq!(
            to_markdown(&[Span::plain("2*3 and [x]")]),
            "2\\*3 and \\[x\\]"
        );
        assert_eq!(
            to_markdown(&[Span::plain("_lead and snake_case")]),
            "\\_lead and snake_case"
        );
    }
}
