//! Rich-text editing inside a card.
//!
//! The editor works on characters with formatting (never on Markdown syntax). When it commits,
//! its lines become `skiss_core` lines again and the card's text in the file is spliced.

use std::ops::Range;
use std::sync::Arc;

use skiss_core::board::{Card, CardKind, Line, LineKind, Span, Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EChar {
    pub c: char,
    pub style: Style,
    pub link: Option<Arc<str>>,
}

#[derive(Clone, Debug)]
pub struct ELine {
    pub title: bool,
    pub kind: LineKind,
    pub depth: u8,
    pub chars: Vec<EChar>,
    /// Source range of the line this came from (empty for new lines).
    pub orig: Range<usize>,
}

impl ELine {
    fn new(kind: LineKind, depth: u8) -> Self {
        ELine {
            title: false,
            kind,
            depth,
            chars: Vec::new(),
            orig: 0..0,
        }
    }

    /// Code blocks, tables and images are shown but not edited as text.
    pub fn is_atom(&self) -> bool {
        self.kind.is_atom()
    }

    pub fn is_list(&self) -> bool {
        matches!(
            self.kind,
            LineKind::Bullet | LineKind::Numbered(_) | LineKind::Task(_)
        )
    }

    pub fn spans(&self) -> Vec<Span> {
        let mut out: Vec<Span> = Vec::new();
        for ch in &self.chars {
            let link = ch.link.as_ref().map(|l| l.to_string());
            match out.last_mut() {
                Some(last) if last.style == ch.style && last.link == link => last.text.push(ch.c),
                _ => out.push(Span {
                    text: ch.c.to_string(),
                    style: ch.style,
                    link,
                }),
            }
        }
        out
    }

    pub fn text(&self) -> String {
        self.chars.iter().map(|c| c.c).collect()
    }

    fn from_spans(spans: &[Span]) -> Vec<EChar> {
        let mut out = Vec::new();
        for s in spans {
            let link: Option<Arc<str>> = s.link.as_deref().map(Arc::from);
            for c in s.text.chars() {
                out.push(EChar {
                    c,
                    style: s.style,
                    link: link.clone(),
                });
            }
        }
        out
    }
}

/// Picks one formatting flag of a style (bold, italic...).
type StyleFlag = fn(&mut Style) -> &mut bool;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

/// What the app should do after a key.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Changed,
    /// Enter in a list-item card: finish and start a sibling.
    NewSibling,
}

pub struct Editor {
    pub card_id: String,
    /// Where the card starts in the source; it stays put while its text is edited.
    pub card_start: usize,
    pub kind: CardKind,
    pub lines: Vec<ELine>,
    pub cursor: Pos,
    pub anchor: Pos,
    /// Formatting for the next typed text when nothing is selected.
    pub pending: Option<Style>,
    /// Remembered x for up/down movement.
    pub preferred_x: Option<f32>,
    /// Edited since the last commit.
    pub dirty: bool,
}

impl Editor {
    pub fn new(card: &Card) -> Editor {
        let mut lines = Vec::new();
        if let Some(h) = &card.heading {
            let mut l = ELine::new(LineKind::Paragraph, 0);
            l.title = true;
            l.chars = ELine::from_spans(&h.spans);
            l.orig = h.line.clone();
            lines.push(l);
        }
        for line in &card.lines {
            lines.push(ELine {
                title: false,
                kind: line.kind.clone(),
                depth: line.depth,
                chars: ELine::from_spans(&line.spans),
                orig: line.orig.clone(),
            });
        }
        if lines.is_empty() {
            lines.push(ELine::new(LineKind::Paragraph, 0));
        }
        let end = Pos {
            line: 0,
            col: lines[0].chars.len(),
        };
        Editor {
            card_id: card.id.clone(),
            card_start: card.range.start,
            kind: card.kind,
            lines,
            cursor: end,
            anchor: end,
            pending: None,
            preferred_x: None,
            dirty: false,
        }
    }

    pub fn has_title(&self) -> bool {
        self.lines.first().is_some_and(|l| l.title)
    }

    pub fn title(&self) -> Option<String> {
        self.lines
            .first()
            .filter(|l| l.title)
            .map(|l| l.text().trim().to_string())
    }

    /// The body as core lines, for writing back.
    pub fn body_lines(&self) -> Vec<Line> {
        self.lines
            .iter()
            .filter(|l| !l.title)
            .map(|l| Line {
                kind: l.kind.clone(),
                depth: l.depth,
                spans: if l.is_atom() && !matches!(l.kind, LineKind::Image { .. }) {
                    vec![Span::plain(l.text())]
                } else {
                    l.spans()
                },
                orig: l.orig.clone(),
                task_at: None,
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.lines
            .iter()
            .all(|l| l.chars.iter().all(|c| c.c.is_whitespace()) && !l.is_atom())
    }

    /// After a commit: point unchanged lines at their new source ranges.
    pub fn refresh_origins(&mut self, card: &Card) {
        self.card_id = card.id.clone();
        self.card_start = card.range.start;
        let mut body = card.lines.iter();
        for l in self.lines.iter_mut() {
            if l.title {
                if let Some(h) = &card.heading {
                    l.orig = h.line.clone();
                }
                continue;
            }
            match body.next() {
                Some(src)
                    if src.kind == l.kind && src.depth == l.depth && src.spans == l.spans() =>
                {
                    l.orig = src.orig.clone()
                }
                _ => l.orig = 0..0,
            }
        }
    }

    // --- Selection --------------------------------------------------------------------------

    pub fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    pub fn selection(&self) -> (Pos, Pos) {
        if self.cursor <= self.anchor {
            (self.cursor, self.anchor)
        } else {
            (self.anchor, self.cursor)
        }
    }

    pub fn select_all(&mut self) {
        self.anchor = Pos::default();
        let last = self.lines.len() - 1;
        self.cursor = Pos {
            line: last,
            col: self.lines[last].chars.len(),
        };
    }

    pub fn set_cursor(&mut self, p: Pos, extend: bool) {
        self.cursor = self.clamp(p);
        if !extend {
            self.anchor = self.cursor;
        }
        self.pending = None;
    }

    fn clamp(&self, p: Pos) -> Pos {
        let line = p.line.min(self.lines.len() - 1);
        Pos {
            line,
            col: p.col.min(self.lines[line].chars.len()),
        }
    }

    pub fn selected_text(&self) -> String {
        let (a, b) = self.selection();
        let mut out = String::new();
        for li in a.line..=b.line {
            let l = &self.lines[li];
            let from = if li == a.line { a.col } else { 0 };
            let to = if li == b.line { b.col } else { l.chars.len() };
            if li > a.line {
                out.push('\n');
            }
            out.extend(l.chars[from.min(to)..to].iter().map(|c| c.c));
        }
        out
    }

    /// Selection ranges per line: (line, from col, to col).
    pub fn selection_ranges(&self) -> Vec<(usize, usize, usize)> {
        if !self.has_selection() {
            return Vec::new();
        }
        let (a, b) = self.selection();
        (a.line..=b.line)
            .map(|li| {
                let from = if li == a.line { a.col } else { 0 };
                let to = if li == b.line {
                    b.col
                } else {
                    self.lines[li].chars.len()
                };
                (li, from, to)
            })
            .collect()
    }

    fn delete_selection(&mut self) -> bool {
        if !self.has_selection() {
            return false;
        }
        let (a, b) = self.selection();
        if a.line == b.line {
            self.lines[a.line].chars.drain(a.col..b.col);
        } else {
            let tail: Vec<EChar> = self.lines[b.line].chars[b.col..].to_vec();
            self.lines[a.line].chars.truncate(a.col);
            self.lines[a.line].chars.extend(tail);
            self.lines.drain(a.line + 1..=b.line);
        }
        self.cursor = a;
        self.anchor = a;
        self.dirty = true;
        true
    }

    // --- Typing -------------------------------------------------------------------------------

    fn style_here(&self) -> (Style, Option<Arc<str>>) {
        if let Some(p) = self.pending {
            return (p, None);
        }
        let line = &self.lines[self.cursor.line];
        let before = self
            .cursor
            .col
            .checked_sub(1)
            .and_then(|i| line.chars.get(i));
        let after = line.chars.get(self.cursor.col);
        let style = before.map(|c| c.style).unwrap_or_default();
        // Links only continue when typing inside them.
        let link = match (before, after) {
            (Some(b), Some(a)) if b.link.is_some() && b.link == a.link => b.link.clone(),
            _ => None,
        };
        (style, link)
    }

    pub fn insert_text(&mut self, text: &str) -> Action {
        self.delete_selection();
        if self.lines[self.cursor.line].is_atom() {
            // Typing on an image or code block starts a new paragraph after it.
            let at = self.cursor.line + 1;
            self.lines.insert(at, ELine::new(LineKind::Paragraph, 0));
            self.cursor = Pos { line: at, col: 0 };
        }
        let mut action = Action::Changed;
        for c in text.chars() {
            if c == '\n' {
                action = self.newline();
                if action == Action::NewSibling {
                    return action;
                }
                continue;
            }
            if c == '\r' {
                continue;
            }
            let (style, link) = self.style_here();
            let line = &mut self.lines[self.cursor.line];
            line.chars.insert(self.cursor.col, EChar { c, style, link });
            self.cursor.col += 1;
            if c == ' ' {
                self.autoformat();
            }
            if matches!(c, '*' | '_' | '`' | '~') {
                self.inline_shortcut();
            }
        }
        self.anchor = self.cursor;
        self.dirty = true;
        self.preferred_x = None;
        action
    }

    /// "[ ] " starts a task, "- " a bullet, "1. " a numbered item, at the start of a line.
    fn autoformat(&mut self) {
        let li = self.cursor.line;
        let line = &self.lines[li];
        if line.title || self.kind == CardKind::Item {
            return;
        }
        let text: String = line
            .chars
            .iter()
            .take(self.cursor.col)
            .map(|c| c.c)
            .collect();
        let (kind, strip) = match text.as_str() {
            "[ ] " | "[] " => (Some(LineKind::Task(false)), text.chars().count()),
            "[x] " | "[X] " => (Some(LineKind::Task(true)), 4),
            "- " | "* " if line.kind == LineKind::Paragraph => (Some(LineKind::Bullet), 2),
            "1. " if line.kind == LineKind::Paragraph => (Some(LineKind::Numbered(1)), 3),
            _ => (None, 0),
        };
        if let Some(kind) = kind {
            let line = &mut self.lines[li];
            line.chars.drain(..strip);
            line.kind = kind;
            line.orig = 0..0;
            self.cursor.col -= strip;
        }
    }

    /// Markdown habits work while typing: `**bold**`, `*italic*`, `` `code` ``, `~~strike~~`.
    fn inline_shortcut(&mut self) {
        let li = self.cursor.line;
        if self.lines[li].title {
            return;
        }
        let col = self.cursor.col;
        let chars: Vec<char> = self.lines[li].chars[..col].iter().map(|c| c.c).collect();
        let rules: [(&str, StyleFlag); 5] = [
            ("**", |s| &mut s.bold),
            ("~~", |s| &mut s.strike),
            ("`", |s| &mut s.code),
            ("*", |s| &mut s.italic),
            ("_", |s| &mut s.italic),
        ];
        for (delim, flag) in rules {
            let d: Vec<char> = delim.chars().collect();
            let n = d.len();
            if chars.len() < n * 2 + 1 || chars[chars.len() - n..] != d[..] {
                continue;
            }
            let close = chars.len() - n;
            // "**" is not an italic "*" closing.
            if n == 1 && close > 0 && chars[close - 1] == d[0] {
                continue;
            }
            let Some(open) = (0..close.saturating_sub(n)).rev().find(|&i| {
                chars[i..i + n] == d[..]
                    && (n > 1 || ((i == 0 || chars[i - 1] != d[0]) && chars[i + 1] != d[0]))
                    && (i == 0 || !chars[i - 1].is_alphanumeric() || d[0] != '_')
            }) else {
                continue;
            };
            let inner = &chars[open + n..close];
            if inner.is_empty()
                || inner[0].is_whitespace()
                || inner[inner.len() - 1].is_whitespace()
            {
                continue;
            }
            let line = &mut self.lines[li];
            line.chars.drain(close..close + n);
            line.chars.drain(open..open + n);
            for ch in &mut line.chars[open..close - n] {
                *flag(&mut ch.style) = true;
            }
            self.cursor.col = col - 2 * n;
            self.anchor = self.cursor;
            // Keep typing in plain text after the closing mark.
            let mut st = line
                .chars
                .get(self.cursor.col.saturating_sub(1))
                .map(|c| c.style)
                .unwrap_or_default();
            *flag(&mut st) = false;
            self.pending = Some(st);
            line.orig = 0..0;
            return;
        }
    }

    pub fn newline(&mut self) -> Action {
        self.delete_selection();
        if self.kind == CardKind::Item {
            return Action::NewSibling;
        }
        let li = self.cursor.line;
        let line = &self.lines[li];
        if line.title {
            // Enter on the title moves into the body.
            let next_empty = self
                .lines
                .get(li + 1)
                .is_some_and(|l| !l.is_atom() && l.chars.is_empty());
            if !next_empty {
                self.lines
                    .insert(li + 1, ELine::new(LineKind::Paragraph, 0));
            }
            self.cursor = Pos {
                line: li + 1,
                col: 0,
            };
            self.anchor = self.cursor;
            self.dirty = true;
            return Action::Changed;
        }
        if line.is_list() && line.chars.is_empty() {
            // Enter on an empty item ends the list (or outdents).
            let l = &mut self.lines[li];
            if l.depth > 0 {
                l.depth -= 1;
            } else {
                l.kind = LineKind::Paragraph;
            }
            l.orig = 0..0;
            self.dirty = true;
            return Action::Changed;
        }
        let kind = match &line.kind {
            LineKind::Task(_) => LineKind::Task(false),
            LineKind::Numbered(n) => LineKind::Numbered(n + 1),
            LineKind::Bullet => LineKind::Bullet,
            _ => LineKind::Paragraph,
        };
        let depth = line.depth;
        let tail: Vec<EChar> = self.lines[li].chars.split_off(self.cursor.col);
        self.lines[li].orig = 0..0;
        let mut new = ELine::new(kind, depth);
        new.chars = tail;
        self.lines.insert(li + 1, new);
        self.cursor = Pos {
            line: li + 1,
            col: 0,
        };
        self.anchor = self.cursor;
        self.dirty = true;
        Action::Changed
    }

    pub fn backspace(&mut self, word: bool) -> Action {
        if self.delete_selection() {
            return Action::Changed;
        }
        let Pos { line: li, col } = self.cursor;
        if col > 0 {
            let from = if word {
                self.word_left(self.cursor).col
            } else {
                col - 1
            };
            self.lines[li].chars.drain(from..col);
            self.cursor.col = from;
        } else {
            let line = &self.lines[li];
            if line.is_list() {
                let l = &mut self.lines[li];
                if l.depth > 0 {
                    l.depth -= 1;
                } else {
                    l.kind = LineKind::Paragraph;
                }
                l.orig = 0..0;
            } else if li > 0 {
                if self.lines[li - 1].is_atom() {
                    self.lines.remove(li - 1);
                    self.cursor.line -= 1;
                } else if line.is_atom() {
                    self.lines.remove(li);
                    self.cursor = Pos {
                        line: li - 1,
                        col: self.lines[li - 1].chars.len(),
                    };
                } else {
                    let chars = self.lines.remove(li).chars;
                    let prev = &mut self.lines[li - 1];
                    let at = prev.chars.len();
                    prev.chars.extend(chars);
                    prev.orig = 0..0;
                    self.cursor = Pos {
                        line: li - 1,
                        col: at,
                    };
                }
            } else {
                return Action::None;
            }
        }
        self.anchor = self.cursor;
        self.dirty = true;
        Action::Changed
    }

    pub fn delete_forward(&mut self, word: bool) -> Action {
        if self.delete_selection() {
            return Action::Changed;
        }
        let Pos { line: li, col } = self.cursor;
        let len = self.lines[li].chars.len();
        if col < len {
            let to = if word {
                self.word_right(self.cursor).col
            } else {
                col + 1
            };
            self.lines[li].chars.drain(col..to);
        } else if li + 1 < self.lines.len() {
            if self.lines[li + 1].is_atom() {
                self.lines.remove(li + 1);
            } else {
                let next = self.lines.remove(li + 1);
                self.lines[li].chars.extend(next.chars);
            }
        } else {
            return Action::None;
        }
        self.dirty = true;
        Action::Changed
    }

    pub fn indent(&mut self, out: bool) -> Action {
        let li = self.cursor.line;
        if !self.lines[li].is_list() {
            return Action::None;
        }
        let max = if li > 0 && self.lines[li - 1].is_list() {
            self.lines[li - 1].depth + 1
        } else {
            0
        };
        let l = &mut self.lines[li];
        if out {
            if l.depth == 0 {
                return Action::None;
            }
            l.depth -= 1;
        } else {
            if l.depth >= max {
                return Action::None;
            }
            l.depth += 1;
        }
        l.orig = 0..0;
        self.dirty = true;
        Action::Changed
    }

    /// Turns the current line into a task, or a task back into text.
    pub fn toggle_task(&mut self) -> Action {
        let li = self.cursor.line;
        let l = &mut self.lines[li];
        if l.title || l.is_atom() || self.kind == CardKind::Item {
            return Action::None;
        }
        l.kind = match l.kind {
            LineKind::Task(_) => LineKind::Paragraph,
            _ => LineKind::Task(false),
        };
        l.orig = 0..0;
        self.dirty = true;
        Action::Changed
    }

    pub fn toggle_check(&mut self, line: usize) -> Action {
        if let Some(l) = self.lines.get_mut(line)
            && let LineKind::Task(done) = l.kind
        {
            l.kind = LineKind::Task(!done);
            self.dirty = true;
            return Action::Changed;
        }
        Action::None
    }

    pub fn toggle_style(&mut self, f: StyleFlag) -> Action {
        if !self.has_selection() {
            let (mut st, _) = self.style_here();
            let v = f(&mut st);
            *v = !*v;
            self.pending = Some(st);
            return Action::None;
        }
        let ranges = self.selection_ranges();
        let all = ranges.iter().all(|(li, a, b)| {
            self.lines[*li].chars[*a..*b].iter().all(|c| {
                let mut s = c.style;
                *f(&mut s)
            })
        });
        for (li, a, b) in ranges {
            for c in &mut self.lines[li].chars[a..b] {
                *f(&mut c.style) = !all;
            }
        }
        self.dirty = true;
        Action::Changed
    }

    /// Pastes text; several lines (or Markdown) become several lines.
    pub fn paste(&mut self, text: &str) -> Action {
        let text = text.replace("\r\n", "\n");
        if !text.trim_end().contains('\n')
            || self.lines[self.cursor.line].title
            || self.kind == CardKind::Item
        {
            return self.insert_text(text.trim_end_matches('\n'));
        }
        self.delete_selection();
        let parsed = skiss_core::board::model::body_lines(&text);
        if parsed.is_empty() {
            return Action::None;
        }
        let li = self.cursor.line;
        let tail: Vec<EChar> = self.lines[li].chars.split_off(self.cursor.col);
        let mut insert_at = li + 1;
        let mut first = true;
        for p in parsed {
            let chars = ELine::from_spans(&p.spans);
            if first && !self.lines[li].is_atom() && !p.kind.is_atom() {
                self.lines[li].chars.extend(chars);
                first = false;
                continue;
            }
            first = false;
            let mut l = ELine::new(p.kind.clone(), p.depth);
            l.chars = chars;
            self.lines.insert(insert_at, l);
            insert_at += 1;
        }
        let last = insert_at - 1;
        let col = self.lines[last].chars.len();
        self.lines[last].chars.extend(tail);
        self.cursor = Pos { line: last, col };
        self.anchor = self.cursor;
        self.dirty = true;
        Action::Changed
    }

    /// Inserts an image line after the cursor line.
    pub fn insert_image(&mut self, alt: &str, url: &str) -> Action {
        let at = (self.cursor.line + 1).min(self.lines.len());
        let mut l = ELine::new(
            LineKind::Image {
                alt: alt.to_string(),
                url: url.to_string(),
            },
            0,
        );
        l.chars = alt
            .chars()
            .map(|c| EChar {
                c,
                style: Style::default(),
                link: None,
            })
            .collect();
        self.lines.insert(at, l);
        let mut p = ELine::new(LineKind::Paragraph, 0);
        p.orig = 0..0;
        self.lines.insert(at + 1, p);
        self.cursor = Pos {
            line: at + 1,
            col: 0,
        };
        self.anchor = self.cursor;
        self.dirty = true;
        Action::Changed
    }

    // --- Moving ---------------------------------------------------------------------------------

    pub fn left(&self, p: Pos) -> Pos {
        if p.col > 0 {
            Pos {
                line: p.line,
                col: p.col - 1,
            }
        } else if p.line > 0 {
            Pos {
                line: p.line - 1,
                col: self.lines[p.line - 1].chars.len(),
            }
        } else {
            p
        }
    }

    pub fn right(&self, p: Pos) -> Pos {
        if p.col < self.lines[p.line].chars.len() {
            Pos {
                line: p.line,
                col: p.col + 1,
            }
        } else if p.line + 1 < self.lines.len() {
            Pos {
                line: p.line + 1,
                col: 0,
            }
        } else {
            p
        }
    }

    pub fn word_left(&self, p: Pos) -> Pos {
        if p.col == 0 {
            return self.left(p);
        }
        let chars = &self.lines[p.line].chars;
        let mut i = p.col;
        while i > 0 && !chars[i - 1].c.is_alphanumeric() {
            i -= 1;
        }
        while i > 0 && chars[i - 1].c.is_alphanumeric() {
            i -= 1;
        }
        Pos {
            line: p.line,
            col: i,
        }
    }

    pub fn word_right(&self, p: Pos) -> Pos {
        let chars = &self.lines[p.line].chars;
        if p.col >= chars.len() {
            return self.right(p);
        }
        let mut i = p.col;
        while i < chars.len() && !chars[i].c.is_alphanumeric() {
            i += 1;
        }
        while i < chars.len() && chars[i].c.is_alphanumeric() {
            i += 1;
        }
        Pos {
            line: p.line,
            col: i,
        }
    }

    pub fn end(&self) -> Pos {
        let line = self.lines.len() - 1;
        Pos {
            line,
            col: self.lines[line].chars.len(),
        }
    }

    /// Selects the word at `p` (double-click).
    pub fn select_word(&mut self, p: Pos) {
        let p = self.clamp(p);
        let chars = &self.lines[p.line].chars;
        let mut a = p.col;
        let mut b = p.col;
        while a > 0 && chars[a - 1].c.is_alphanumeric() {
            a -= 1;
        }
        while b < chars.len() && chars[b].c.is_alphanumeric() {
            b += 1;
        }
        self.anchor = Pos {
            line: p.line,
            col: a,
        };
        self.cursor = Pos {
            line: p.line,
            col: b,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skiss_core::board::Board;

    fn editor(src: &str, id: &str) -> Editor {
        let b = Board::parse(src);
        Editor::new(b.card(id).unwrap())
    }

    #[test]
    fn typing_and_formatting() {
        let mut e = editor("## Note\n\nHello\n", "note");
        e.set_cursor(Pos { line: 1, col: 5 }, false);
        e.insert_text(" world");
        e.set_cursor(Pos { line: 1, col: 6 }, false);
        e.set_cursor(Pos { line: 1, col: 11 }, true);
        e.toggle_style(|s| &mut s.bold);
        let md = skiss_core::board::inline::to_markdown(&e.body_lines()[0].spans);
        assert_eq!(md, "Hello **world**");
    }

    #[test]
    fn markdown_habits_format_while_typing() {
        let mut e = editor("## Note\n\nx\n", "note");
        e.set_cursor(Pos { line: 1, col: 1 }, false);
        e.insert_text(" a **bold** and *it* and `code` then");
        let md = skiss_core::board::inline::to_markdown(&e.body_lines()[0].spans);
        assert_eq!(md, "x a **bold** and *it* and `code` then");
        assert!(e.lines[1].chars.iter().any(|c| c.style.bold));
        assert!(!e.lines[1].chars.last().unwrap().style.bold);
        // snake_case and 2*3*4 stay as typed.
        let mut e = editor("## Note\n\nx\n", "note");
        e.set_cursor(Pos { line: 1, col: 1 }, false);
        e.insert_text(" snake_case_name");
        assert!(e.lines[1].chars.iter().all(|c| !c.style.italic));
    }

    #[test]
    fn enter_in_a_task_list_adds_a_task() {
        let mut e = editor("## Todo\n\n- [ ] one\n", "todo");
        e.set_cursor(Pos { line: 1, col: 3 }, false);
        e.newline();
        e.insert_text("two");
        let lines = e.body_lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].kind, LineKind::Task(false));
        assert_eq!(lines[1].text(), "two");
    }

    #[test]
    fn brackets_start_a_task() {
        let mut e = editor("## Todo\n\ntext\n", "todo");
        e.set_cursor(Pos { line: 1, col: 4 }, false);
        e.newline();
        e.insert_text("[ ] call Johan");
        let lines = e.body_lines();
        assert_eq!(lines[1].kind, LineKind::Task(false));
        assert_eq!(lines[1].text(), "call Johan");
    }

    #[test]
    fn enter_on_title_moves_to_body() {
        let mut e = editor("## Note\n", "note");
        e.newline();
        e.insert_text("body");
        assert_eq!(e.title().as_deref(), Some("Note"));
        assert_eq!(e.body_lines()[0].text(), "body");
    }

    #[test]
    fn backspace_merges_lines_and_outdents_lists() {
        let mut e = editor("## N\n\n- a\n  - b\n", "n");
        e.set_cursor(Pos { line: 2, col: 0 }, false);
        e.backspace(false);
        assert_eq!(e.lines[2].depth, 0);
        e.backspace(false);
        assert_eq!(e.lines[2].kind, LineKind::Paragraph);
        e.backspace(false);
        assert_eq!(e.lines[1].text(), "ab");
    }

    #[test]
    fn multi_line_paste_becomes_lines() {
        let mut e = editor("## N\n\nstart\n", "n");
        e.set_cursor(Pos { line: 1, col: 5 }, false);
        e.paste("\n- [ ] task one\n- [ ] task two\n");
        let lines = e.body_lines();
        assert!(
            lines
                .iter()
                .any(|l| l.kind == LineKind::Task(false) && l.text() == "task two")
        );
    }
}

#[cfg(test)]
mod repro {
    use super::*;
    use skiss_core::board::{Board, edit};

    #[test]
    fn bold_at_end_has_no_trailing_spaces() {
        let src = "## Erik\n\nTech lead.\n";
        let b = Board::parse(src);
        let mut e = Editor::new(b.card("erik").unwrap());
        e.set_cursor(Pos { line: 1, col: 10 }, false);
        e.insert_text(" Loves Rust.");
        for _ in 0..5 {
            let p = e.left(e.cursor);
            e.set_cursor(p, true);
        }
        e.toggle_style(|s| &mut s.bold);
        let out =
            edit::set_card_text(&b, src, "erik", e.title().as_deref(), &e.body_lines()).unwrap();
        assert_eq!(out.source, "## Erik\n\nTech lead. Loves **Rust.**\n");
    }

    /// The app saves after every keystroke; the file must end up the same as one big save.
    #[test]
    fn saving_per_keystroke_leaves_no_stray_spaces() {
        let mut src = "## Erik\n\nTech lead.\n".to_string();
        let b = Board::parse(&src);
        let mut e = Editor::new(b.card("erik").unwrap());
        e.set_cursor(Pos { line: 1, col: 10 }, false);
        for c in " Loves Rust.".chars() {
            e.insert_text(&c.to_string());
            let b = Board::parse(&src);
            if let Some(out) =
                edit::set_card_text(&b, &src, "erik", e.title().as_deref(), &e.body_lines())
            {
                src = out.source;
                let nb = Board::parse(&src);
                e.refresh_origins(nb.card("erik").unwrap());
            }
        }
        assert_eq!(src, "## Erik\n\nTech lead. Loves Rust.\n");
    }
}

#[cfg(test)]
mod repro_tasks {
    use super::*;
    use skiss_core::board::{Board, edit};

    fn type_saving(src: &mut String, e: &mut Editor, text: &str) {
        for c in text.chars() {
            if c == '\n' {
                e.newline();
            } else {
                e.insert_text(&c.to_string());
            }
            let b = Board::parse(src);
            if let Some(out) =
                edit::set_card_text(&b, src, "agenda", e.title().as_deref(), &e.body_lines())
            {
                *src = out.source;
                let nb = Board::parse(src);
                e.refresh_origins(nb.card("agenda").unwrap());
            }
        }
    }

    #[test]
    fn typing_a_task_list_per_keystroke() {
        let mut src = "# T\n\n## Agenda\n".to_string();
        let b = Board::parse(&src);
        let mut e = Editor::new(b.card("agenda").unwrap());
        e.set_cursor(Pos { line: 0, col: 6 }, false);
        type_saving(
            &mut src,
            &mut e,
            "\n[ ] Morning: where we are\nAfternoon: where we go",
        );
        assert_eq!(
            src,
            "# T\n\n## Agenda\n- [ ] Morning: where we are\n- [ ] Afternoon: where we go\n"
        );
    }
}
