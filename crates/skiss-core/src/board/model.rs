//! The board: what a Markdown file looks like on the canvas.
//!
//! The mapping (see the design doc):
//!
//! | Markdown | Board |
//! | --- | --- |
//! | the only `# H1`, first in the file | board title |
//! | a heading with sub-headings | area |
//! | a heading without sub-headings | card (title + body) |
//! | loose paragraphs, quotes, code, tables | one card each |
//! | a loose bullet list | one card per item, nested items inside |
//! | a loose task list | one checklist card |
//! | `- Manages [Erik](#erik-lund)` in a card | a connection (arrow) |
//!
//! Every element remembers its byte range so edits can splice the file instead of rewriting it.

use std::collections::HashMap;
use std::ops::Range;

use super::appendix::{self, Appendix};
use super::blocks::{self, Block, BlockKind};
use super::inline::{self, Span};
use super::slug::Slugger;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    /// Plain text of the heading.
    pub text: String,
    pub spans: Vec<Span>,
    pub level: u8,
    /// The heading line(s), without the trailing newline.
    pub line: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineKind {
    Paragraph,
    Bullet,
    Numbered(u64),
    Task(bool),
    Quote,
    Code(String),
    Image {
        alt: String,
        url: String,
    },
    /// A table; the spans hold its source, which is never rewritten.
    Table,
    /// An HTML block shown as readable text (and its first local image); the spans hold the
    /// source, which is never rewritten.
    Html {
        text: String,
        image: Option<String>,
    },
    /// Anything else shown as-is.
    Raw,
}

impl LineKind {
    /// Lines shown but not edited as text (their source is kept byte for byte).
    pub fn is_atom(&self) -> bool {
        matches!(
            self,
            LineKind::Code(_)
                | LineKind::Raw
                | LineKind::Table
                | LineKind::Html { .. }
                | LineKind::Image { .. }
        )
    }
}

/// One line of a card body as it is displayed: a paragraph, a list item, a code block...
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    pub depth: u8,
    pub spans: Vec<Span>,
    /// The exact source text this line came from (marker and indentation included), reused
    /// verbatim when the line is written back unchanged.
    pub orig: Range<usize>,
    /// For tasks: byte offset of the `[` of the `[ ]` box.
    pub task_at: Option<usize>,
}

impl Line {
    pub fn is_list(&self) -> bool {
        matches!(
            self.kind,
            LineKind::Bullet | LineKind::Numbered(_) | LineKind::Task(_)
        )
    }

    pub fn text(&self) -> String {
        inline::plain_text(&self.spans)
    }
}

/// A connection written as a sentence: "Manages [Erik](#erik-lund) and [Sara](#sara-ek)".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
    /// The words before the first link ("Manages").
    pub label: String,
    /// Anchors of the targets (without `#`).
    pub targets: Vec<String>,
    /// Text after the last link ("2026-10-14").
    pub note: String,
    /// The whole list line, including its newline.
    pub line: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    /// A heading without sub-headings.
    Section,
    /// A loose paragraph, quote, code block, table or image.
    Block,
    /// A loose list item (children are nested items).
    Item,
    /// A loose task list.
    Tasks,
}

#[derive(Clone, Debug)]
pub struct Card {
    pub id: String,
    /// Title used as the appendix key.
    pub key: String,
    pub kind: CardKind,
    pub heading: Option<Heading>,
    /// Everything the card owns in the file (heading through the end of its section).
    pub range: Range<usize>,
    /// The body content (trimmed). Empty and placed right after the heading if there is none.
    pub body: Range<usize>,
    pub lines: Vec<Line>,
    pub children: Vec<Card>,
    pub connections: Vec<Connection>,
}

impl Card {
    pub fn title(&self) -> Option<&str> {
        self.heading.as_ref().map(|h| h.text.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct Area {
    pub id: String,
    pub key: String,
    /// `None` for the root of the board.
    pub heading: Option<Heading>,
    pub section: Range<usize>,
    /// Loose content between the heading and the first sub-heading.
    pub intro: Range<usize>,
    /// The heading level used for new children.
    pub child_level: u8,
    pub children: Vec<Node>,
    pub connections: Vec<Connection>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Area(Area),
    Card(Card),
}

impl Node {
    pub fn id(&self) -> &str {
        match self {
            Node::Area(a) => &a.id,
            Node::Card(c) => &c.id,
        }
    }

    pub fn key(&self) -> &str {
        match self {
            Node::Area(a) => &a.key,
            Node::Card(c) => &c.key,
        }
    }

    /// The source range this node owns.
    pub fn range(&self) -> Range<usize> {
        match self {
            Node::Area(a) => a.section.clone(),
            Node::Card(c) => c.range.clone(),
        }
    }

    pub fn heading(&self) -> Option<&Heading> {
        match self {
            Node::Area(a) => a.heading.as_ref(),
            Node::Card(c) => c.heading.as_ref(),
        }
    }

    pub fn connections(&self) -> &[Connection] {
        match self {
            Node::Area(a) => &a.connections,
            Node::Card(c) => &c.connections,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Board {
    pub title: Option<Heading>,
    pub root: Area,
    pub appendix: Appendix,
    /// The appendix comment, if the file has one.
    pub appendix_range: Option<Range<usize>>,
    /// Anchor -> node id, for resolving connection targets.
    pub anchors: HashMap<String, String>,
    pub len: usize,
}

/// A reference to something on the board, with the ids of its ancestors.
#[derive(Clone, Copy, Debug)]
pub enum NodeRef<'a> {
    Area(&'a Area),
    Card(&'a Card),
}

impl<'a> NodeRef<'a> {
    pub fn id(&self) -> &'a str {
        match self {
            NodeRef::Area(a) => &a.id,
            NodeRef::Card(c) => &c.id,
        }
    }

    pub fn key(&self) -> &'a str {
        match self {
            NodeRef::Area(a) => &a.key,
            NodeRef::Card(c) => &c.key,
        }
    }

    pub fn heading(&self) -> Option<&'a Heading> {
        match self {
            NodeRef::Area(a) => a.heading.as_ref(),
            NodeRef::Card(c) => c.heading.as_ref(),
        }
    }

    pub fn range(&self) -> Range<usize> {
        match self {
            NodeRef::Area(a) => a.section.clone(),
            NodeRef::Card(c) => c.range.clone(),
        }
    }

    pub fn connections(&self) -> &'a [Connection] {
        match self {
            NodeRef::Area(a) => &a.connections,
            NodeRef::Card(c) => &c.connections,
        }
    }
}

impl Board {
    pub fn parse(src: &str) -> Board {
        Parser::new(src).run()
    }

    /// Finds a node and the chain of area ids above it (root first).
    pub fn find(&self, id: &str) -> Option<(NodeRef<'_>, Vec<&Area>)> {
        fn walk<'a>(area: &'a Area, id: &str, path: &mut Vec<&'a Area>) -> Option<NodeRef<'a>> {
            if area.id == id {
                return Some(NodeRef::Area(area));
            }
            path.push(area);
            for child in &area.children {
                match child {
                    Node::Area(a) => {
                        if let Some(found) = walk(a, id, path) {
                            return Some(found);
                        }
                    }
                    Node::Card(c) => {
                        if let Some(found) = find_card(c, id) {
                            return Some(NodeRef::Card(found));
                        }
                    }
                }
            }
            path.pop();
            None
        }
        fn find_card<'a>(card: &'a Card, id: &str) -> Option<&'a Card> {
            if card.id == id {
                return Some(card);
            }
            card.children.iter().find_map(|c| find_card(c, id))
        }
        let mut path = Vec::new();
        let found = walk(&self.root, id, &mut path)?;
        Some((found, path))
    }

    pub fn node(&self, id: &str) -> Option<NodeRef<'_>> {
        self.find(id).map(|(n, _)| n)
    }

    pub fn area(&self, id: &str) -> Option<&Area> {
        match self.node(id)? {
            NodeRef::Area(a) => Some(a),
            NodeRef::Card(_) => None,
        }
    }

    pub fn card(&self, id: &str) -> Option<&Card> {
        match self.node(id)? {
            NodeRef::Card(c) => Some(c),
            NodeRef::Area(_) => None,
        }
    }

    /// The area that directly contains `id`.
    pub fn parent_area(&self, id: &str) -> Option<&Area> {
        let (_, path) = self.find(id)?;
        path.last().copied()
    }

    /// Every node in document order (areas before their children).
    pub fn walk(&self) -> Vec<NodeRef<'_>> {
        fn go<'a>(area: &'a Area, out: &mut Vec<NodeRef<'a>>) {
            out.push(NodeRef::Area(area));
            for child in &area.children {
                match child {
                    Node::Area(a) => go(a, out),
                    Node::Card(c) => card(c, out),
                }
            }
        }
        fn card<'a>(c: &'a Card, out: &mut Vec<NodeRef<'a>>) {
            out.push(NodeRef::Card(c));
            for child in &c.children {
                card(child, out);
            }
        }
        let mut out = Vec::new();
        go(&self.root, &mut out);
        out
    }

    /// All appendix keys in document order.
    pub fn keys(&self) -> Vec<String> {
        self.walk()
            .into_iter()
            .filter(|n| !n.id().is_empty())
            .map(|n| n.key().to_string())
            .collect()
    }

    /// All connections as (source id, target id, connection).
    pub fn connections(&self) -> Vec<(String, String, &Connection)> {
        let mut out = Vec::new();
        for node in self.walk() {
            for conn in node.connections() {
                for t in &conn.targets {
                    if let Some(target) = self.anchors.get(t) {
                        out.push((node.id().to_string(), target.clone(), conn));
                    }
                }
            }
        }
        out
    }

    /// The anchor of a node (for links pointing at it).
    pub fn anchor_of(&self, id: &str) -> Option<&str> {
        self.anchors
            .iter()
            .find(|(_, v)| v.as_str() == id)
            .map(|(k, _)| k.as_str())
    }

    /// Where the content (excluding the appendix) ends.
    pub fn content_end(&self) -> usize {
        self.appendix_range.as_ref().map_or(self.len, |r| r.start)
    }
}

// ---------------------------------------------------------------------------------------------

enum Top {
    Heading(Heading),
    Block(Block),
}

struct Sec {
    heading: Option<Heading>,
    level: u8,
    blocks: Vec<Block>,
    children: Vec<Sec>,
    start: usize,
    end: usize,
}

struct Parser<'s> {
    src: &'s str,
    slugger: Slugger,
    anchors: HashMap<String, String>,
    keys: HashMap<String, usize>,
}

impl<'s> Parser<'s> {
    fn new(src: &'s str) -> Self {
        Parser {
            src,
            slugger: Slugger::default(),
            anchors: HashMap::new(),
            keys: HashMap::new(),
        }
    }

    fn run(mut self) -> Board {
        let src = self.src;
        let mut tops: Vec<Top> = Vec::new();
        let mut appendix = Appendix::default();
        let mut appendix_range = None;
        let blocks = blocks::parse_blocks(src);
        let last_html = blocks
            .iter()
            .rposition(|b| b.kind == BlockKind::Html && is_appendix(&src[b.range.clone()]));
        for (i, b) in blocks.into_iter().enumerate() {
            if Some(i) == last_html {
                let r = blocks::trim(src, b.range.clone());
                appendix = Appendix::parse(&src[r.clone()]);
                appendix_range = Some(r);
                continue;
            }
            match b.kind {
                BlockKind::Heading(level) => {
                    let line = heading_line(src, &b);
                    let text = blocks::heading_text(src, b.range.clone());
                    let spans = inline::parse_inline(&text);
                    tops.push(Top::Heading(Heading {
                        text: inline::plain_text(&spans).trim().to_string(),
                        spans,
                        level,
                        line,
                    }));
                }
                _ => tops.push(Top::Block(b)),
            }
        }
        let content_end = appendix_range.as_ref().map_or(src.len(), |r| r.start);

        // Board title: the only H1, and the first thing in the file.
        let h1_count = tops
            .iter()
            .filter(|t| matches!(t, Top::Heading(h) if h.level == 1))
            .count();
        let mut title = None;
        if h1_count == 1
            && let Some(Top::Heading(h)) = tops.first()
            && h.level == 1
        {
            let Top::Heading(h) = tops.remove(0) else {
                unreachable!()
            };
            self.slugger.next(&h.text);
            title = Some(h);
        }

        // Sections by heading level.
        let root_start = title.as_ref().map_or(0, |h| h.line.end);
        let mut stack: Vec<Sec> = vec![Sec {
            heading: None,
            level: 0,
            blocks: Vec::new(),
            children: Vec::new(),
            start: root_start,
            end: content_end,
        }];
        for top in tops {
            match top {
                Top::Heading(h) => {
                    while stack.len() > 1 && stack.last().expect("non-empty").level >= h.level {
                        let mut done = stack.pop().expect("len > 1");
                        done.end = h.line.start;
                        stack.last_mut().expect("root").children.push(done);
                    }
                    stack.push(Sec {
                        level: h.level,
                        start: h.line.start,
                        end: content_end,
                        heading: Some(h),
                        blocks: Vec::new(),
                        children: Vec::new(),
                    });
                }
                Top::Block(b) => stack.last_mut().expect("root").blocks.push(b),
            }
        }
        while stack.len() > 1 {
            let mut done = stack.pop().expect("len > 1");
            done.end = content_end;
            stack.last_mut().expect("root").children.push(done);
        }
        let root_sec = stack.pop().expect("root");
        let root_child_level = root_sec
            .children
            .iter()
            .map(|s| s.level)
            .min()
            .unwrap_or(if title.is_some() { 2 } else { 1 });

        let mut root = self.area(root_sec, String::new(), root_child_level);
        root.key = String::new();
        Board {
            title,
            root,
            appendix,
            appendix_range,
            anchors: self.anchors,
            len: src.len(),
        }
    }

    fn unique_key(&mut self, key: &str) -> String {
        let base = if key.trim().is_empty() {
            "Card".to_string()
        } else {
            key.trim().to_string()
        };
        let n = self.keys.entry(base.clone()).or_insert(0);
        *n += 1;
        if *n == 1 {
            base
        } else {
            format!("{base} ({n})")
        }
    }

    fn section_id(&mut self, heading: &Heading) -> String {
        let text = if heading.text.trim().is_empty() {
            "untitled"
        } else {
            heading.text.as_str()
        };
        let anchor = self.slugger.next(text);
        // Ids are the anchors, which are unique and stable while the title stays the same.
        self.anchors.insert(anchor.clone(), anchor.clone());
        anchor
    }

    fn area(&mut self, sec: Sec, id: String, child_level: u8) -> Area {
        let src = self.src;
        let key = sec
            .heading
            .as_ref()
            .map(|h| self.unique_key(&h.text))
            .unwrap_or_default();
        let intro_start = sec.heading.as_ref().map_or(sec.start, |h| {
            blocks::line_end_incl(src, h.line.end.saturating_sub(1).max(h.line.start))
        });
        let intro_end = sec.children.first().map_or(sec.end, |c| c.start);
        let intro = intro_start.min(intro_end)..intro_end;

        let mut children = Vec::new();
        let mut connections = Vec::new();
        let mut n = 0usize;
        for b in &sec.blocks {
            self.loose_block(b, &id, &mut n, &mut children, &mut connections);
        }
        for child in sec.children {
            if child.children.is_empty() {
                children.push(Node::Card(self.section_card(child)));
            } else {
                let h = child
                    .heading
                    .as_ref()
                    .expect("child sections have headings");
                let cid = self.section_id(h);
                let level = child.level + 1;
                let child_level = child
                    .children
                    .iter()
                    .map(|s| s.level)
                    .min()
                    .unwrap_or(level);
                children.push(Node::Area(self.area(child, cid, child_level)));
            }
        }
        Area {
            id,
            key,
            heading: sec.heading,
            section: sec.start..sec.end,
            intro,
            child_level,
            children,
            connections,
        }
    }

    fn section_card(&mut self, sec: Sec) -> Card {
        let src = self.src;
        let heading = sec.heading.expect("sections have headings");
        let id = self.section_id(&heading);
        let key = self.unique_key(&heading.text);
        let mut lines = Vec::new();
        let mut connections = Vec::new();
        for b in &sec.blocks {
            self.body_block(b, 0, true, &mut lines, &mut connections);
        }
        let body = match (sec.blocks.first(), sec.blocks.last()) {
            (Some(f), Some(l)) => blocks::trim(src, f.range.start..l.range.end),
            _ => {
                let at = blocks::line_end_incl(
                    src,
                    heading.line.end.saturating_sub(1).max(heading.line.start),
                );
                at..at
            }
        };
        Card {
            id,
            key,
            kind: CardKind::Section,
            heading: Some(heading),
            range: sec.start..sec.end,
            body,
            lines,
            children: Vec::new(),
            connections,
        }
    }

    /// Loose content directly in an area becomes cards (and area connections).
    fn loose_block(
        &mut self,
        b: &Block,
        area_id: &str,
        n: &mut usize,
        out: &mut Vec<Node>,
        connections: &mut Vec<Connection>,
    ) {
        let src = self.src;
        match &b.kind {
            BlockKind::Rule => {}
            BlockKind::Html
                if html_text(&src[b.range.clone()]).is_empty()
                    && html_local_image(&src[b.range.clone()]).is_none() => {}
            BlockKind::List(_) if !b.is_task_list() => {
                for item in &b.children {
                    let text = blocks::item_text_range(src, item);
                    if let Some(conn) = connection(src, item, &text) {
                        connections.push(conn);
                        continue;
                    }
                    *n += 1;
                    let id = format!("{area_id}/{n}");
                    out.push(Node::Card(self.item_card(item, id)));
                }
            }
            _ => {
                *n += 1;
                let id = format!("{area_id}/{n}");
                let mut lines = Vec::new();
                let mut conns = Vec::new();
                self.body_block(b, 0, false, &mut lines, &mut conns);
                let kind = if matches!(b.kind, BlockKind::List(_)) {
                    CardKind::Tasks
                } else {
                    CardKind::Block
                };
                let first = lines.first().map(Line::text).unwrap_or_default();
                let key = self.unique_key(&short(&first));
                let range = blocks::trim(src, b.range.clone());
                out.push(Node::Card(Card {
                    id,
                    key,
                    kind,
                    heading: None,
                    range: range.clone(),
                    body: range,
                    lines,
                    children: Vec::new(),
                    connections: conns,
                }));
            }
        }
    }

    fn item_card(&mut self, item: &Block, id: String) -> Card {
        let src = self.src;
        let text = blocks::item_text_range(src, item);
        let spans = inline::parse_inline(&src[text.clone()]);
        let key = self.unique_key(&short(&inline::plain_text(&spans)));
        let lines = vec![Line {
            kind: LineKind::Paragraph,
            depth: 0,
            spans,
            orig: text.clone(),
            task_at: None,
        }];
        let mut children = Vec::new();
        let mut i = 0;
        for child in &item.children {
            if matches!(child.kind, BlockKind::List(_)) {
                for sub in &child.children {
                    i += 1;
                    children.push(self.item_card(sub, format!("{id}.{i}")));
                }
            }
        }
        let range = blocks::trim(src, item.range.clone());
        Card {
            id,
            key,
            kind: CardKind::Item,
            heading: None,
            range,
            body: text,
            lines,
            children,
            connections: Vec::new(),
        }
    }

    fn body_block(
        &mut self,
        b: &Block,
        depth: u8,
        allow_connections: bool,
        lines: &mut Vec<Line>,
        connections: &mut Vec<Connection>,
    ) {
        let src = self.src;
        let range = blocks::trim(src, b.range.clone());
        match &b.kind {
            BlockKind::Paragraph => {
                let text = &src[range.clone()];
                let kind = image_only(text)
                    .map(|(alt, url)| LineKind::Image { alt, url })
                    .unwrap_or(LineKind::Paragraph);
                lines.push(Line {
                    kind,
                    depth,
                    spans: inline::parse_inline(text),
                    orig: range,
                    task_at: None,
                });
            }
            BlockKind::List(start) => {
                for (i, item) in b.children.iter().enumerate() {
                    let text = blocks::item_text_range(src, item);
                    if allow_connections
                        && depth == 0
                        && let Some(conn) = connection(src, item, &text)
                    {
                        connections.push(conn);
                        continue;
                    }
                    let kind = match (item.task, start) {
                        (Some((checked, _)), _) => LineKind::Task(checked),
                        (None, Some(n)) => LineKind::Numbered(n + i as u64),
                        (None, None) => LineKind::Bullet,
                    };
                    let line_from = blocks::line_start(src, item.range.start);
                    lines.push(Line {
                        kind,
                        depth,
                        spans: inline::parse_inline(&src[text.clone()]),
                        orig: line_from..text.end.max(line_from),
                        task_at: item.task.map(|t| t.1),
                    });
                    for child in &item.children {
                        if matches!(child.kind, BlockKind::List(_)) {
                            self.body_block(child, depth + 1, false, lines, connections);
                        }
                    }
                }
            }
            BlockKind::Quote => {
                let inner: String = src[range.clone()]
                    .lines()
                    .map(|l| l.trim_start().trim_start_matches('>').trim())
                    .collect::<Vec<_>>()
                    .join(" ");
                lines.push(Line {
                    kind: LineKind::Quote,
                    depth,
                    spans: inline::parse_inline(&inner),
                    orig: range,
                    task_at: None,
                });
            }
            BlockKind::Code(lang) => {
                lines.push(Line {
                    kind: LineKind::Code(lang.clone()),
                    depth,
                    spans: vec![Span::plain(code_text(&src[range.clone()]))],
                    orig: range,
                    task_at: None,
                });
            }
            BlockKind::Rule | BlockKind::Heading(_) => {}
            BlockKind::Item => {}
            BlockKind::Html => {
                let raw = &src[range.clone()];
                lines.push(Line {
                    kind: LineKind::Html {
                        text: html_text(raw),
                        image: html_local_image(raw),
                    },
                    depth,
                    spans: vec![Span::plain(raw.to_string())],
                    orig: range,
                    task_at: None,
                });
            }
            BlockKind::Table => {
                lines.push(Line {
                    kind: LineKind::Table,
                    depth,
                    spans: vec![Span::plain(src[range.clone()].to_string())],
                    orig: range,
                    task_at: None,
                });
            }
        }
    }
}

/// Parses Markdown as the body of a card (for pasting several lines into a card).
/// Headings become plain paragraphs. Source ranges are cleared: the text is not in the file.
pub fn body_lines(src: &str) -> Vec<Line> {
    let mut parser = Parser::new(src);
    let mut lines = Vec::new();
    let mut conns = Vec::new();
    for b in blocks::parse_blocks(src) {
        if let BlockKind::Heading(_) = b.kind {
            let text = blocks::heading_text(src, b.range.clone());
            lines.push(Line {
                kind: LineKind::Paragraph,
                depth: 0,
                spans: inline::parse_inline(&text),
                orig: 0..0,
                task_at: None,
            });
            continue;
        }
        parser.body_block(&b, 0, false, &mut lines, &mut conns);
    }
    for l in &mut lines {
        l.orig = 0..0;
        l.task_at = None;
    }
    lines
}

/// The readable text of an HTML block: tags removed, entities decoded, alt texts of remote
/// images kept (so a row of badges reads "CI · crates.io").
pub fn html_text(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    let mut alts: Vec<String> = Vec::new();
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('>') else {
            rest = "";
            break;
        };
        let tag = &rest[i + 1..i + j];
        let lower = tag.to_lowercase();
        if lower.starts_with("!--") {
            // Comments are invisible, as in any viewer.
            let end = rest[i..].find("-->").map_or(rest.len(), |e| i + e + 3);
            rest = &rest[end..];
            continue;
        }
        if lower.starts_with("br")
            || lower.starts_with("/p")
            || lower.starts_with("/h")
            || lower.starts_with("/div")
        {
            out.push(' ');
        }
        if lower.starts_with("img")
            && let Some(alt) = attr(tag, "alt")
            && attr(tag, "src").is_some_and(|s| s.starts_with("http"))
            && !alt.is_empty()
        {
            alts.push(alt);
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    let text = decode_entities(&out);
    let mut text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() && !alts.is_empty() {
        text = alts.join(" · ");
    }
    text
}

/// The first `<img>` with a local `src` in an HTML block.
pub fn html_local_image(html: &str) -> Option<String> {
    let mut rest = html;
    while let Some(i) = rest.to_lowercase().find("<img") {
        let end = rest[i..].find('>').map_or(rest.len(), |e| i + e);
        let tag = &rest[i + 1..end];
        if let Some(src) = attr(tag, "src")
            && !src.starts_with("http")
            && !src.starts_with("data:")
        {
            return Some(src);
        }
        rest = &rest[end..];
    }
    None
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let after = lower[at + name.len()..].trim_start();
        if before_ok && after.starts_with('=') {
            let v = tag[tag.len() - after.len() + 1..].trim_start();
            let quote = v.chars().next()?;
            if quote == '"' || quote == '\'' {
                let inner = &v[1..];
                return inner.find(quote).map(|e| decode_entities(&inner[..e]));
            }
            return Some(
                v.split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('/')
                    .to_string(),
            );
        }
        from = at + name.len();
    }
    None
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&middot;", "·")
        .replace("&mdash;", "\u{2014}")
        .replace("&ndash;", "–")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn is_appendix(text: &str) -> bool {
    text.trim_start().starts_with(appendix::OPEN)
}

fn heading_line(src: &str, b: &Block) -> std::ops::Range<usize> {
    let r = blocks::trim(src, b.range.clone());
    r.start..r.end
}

fn short(text: &str) -> String {
    let t = text.trim();
    let mut out: String = t.chars().take(40).collect();
    if t.chars().count() > 40 {
        out = out.trim_end().to_string();
        out.push('…');
    }
    out
}

fn code_text(block: &str) -> String {
    let lines: Vec<&str> = block.lines().collect();
    if let Some(first) = lines.first() {
        let f = first.trim_start();
        if f.starts_with("```") || f.starts_with("~~~") {
            let end = if lines.len() > 1
                && lines
                    .last()
                    .is_some_and(|l| l.trim_start().starts_with(&f[..3]))
            {
                lines.len() - 1
            } else {
                lines.len()
            };
            return lines[1.min(end)..end].join("\n");
        }
    }
    lines
        .iter()
        .map(|l| l.strip_prefix("    ").unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `![alt](url)` and nothing else.
fn image_only(text: &str) -> Option<(String, String)> {
    let t = text.trim();
    let rest = t.strip_prefix("![")?;
    let close = rest.find("](")?;
    let alt = &rest[..close];
    let after = &rest[close + 2..];
    let url = after.strip_suffix(')')?;
    if url.contains(')') || url.contains(' ') && !url.contains('"') {
        return None;
    }
    let url = url.split_whitespace().next().unwrap_or(url);
    Some((alt.to_string(), url.to_string()))
}

/// Recognizes "Label [Target](#anchor) and [Other](#other), note" as a connection.
fn connection(src: &str, item: &Block, text: &Range<usize>) -> Option<Connection> {
    if item.task.is_some() {
        return None;
    }
    let spans = inline::parse_inline(&src[text.clone()]);
    let mut label = String::new();
    let mut targets: Vec<String> = Vec::new();
    let mut between = String::new();
    let mut note = String::new();
    let mut last_link: Option<&str> = None;
    for s in &spans {
        match s.link.as_deref() {
            Some(l) if l.starts_with('#') => {
                if last_link != Some(l) {
                    if !targets.is_empty() && !is_separator(&between) {
                        return None;
                    }
                    between.clear();
                    targets.push(l[1..].to_string());
                    last_link = Some(l);
                }
            }
            Some(_) => {
                if targets.is_empty() {
                    return None;
                }
                between.push_str(&s.text);
                last_link = None;
            }
            None => {
                last_link = None;
                if targets.is_empty() {
                    label.push_str(&s.text);
                } else {
                    between.push_str(&s.text);
                }
            }
        }
    }
    if targets.is_empty() || label.chars().count() > 60 {
        return None;
    }
    if !between.is_empty() {
        note = between
            .trim()
            .trim_start_matches([',', ';', '-', '–', ':'])
            .trim()
            .to_string();
    }
    let label = label
        .trim()
        .trim_end_matches([':', '-', '–'])
        .trim()
        .to_string();
    let from = blocks::line_start(src, item.range.start);
    let to = blocks::line_end_incl(src, text.end.max(from));
    Some(Connection {
        label,
        targets,
        note,
        line: from..to,
    })
}

fn is_separator(text: &str) -> bool {
    let t = text.trim().trim_matches(',').trim();
    t.is_empty() || ["and", "&", "or", "och", "+"].contains(&t.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM: &str = "# Apollo program

## People

### Anna Berg
Program sponsor.
- Manages [Erik Lund](#erik-lund) and [Sara Ek](#sara-ek)
- Told me to contact [Johan Holm](#johan-holm), 2026-10-14

### Erik Lund
Tech lead, backend.

### Sara Ek
Owns the budget.

### Johan Holm
Integration lead.

## Meetings

### 2026-10-20 Meeting with Johan
Agreed on an API freeze by 1 November.

## Next steps

- [x] Book workshop with Erik
- [ ] Send API freeze summary to Anna
";

    #[test]
    fn maps_headings_to_areas_and_cards() {
        let b = Board::parse(PROGRAM);
        assert_eq!(b.title.as_ref().unwrap().text, "Apollo program");
        assert_eq!(b.root.children.len(), 3);
        let Node::Area(people) = &b.root.children[0] else {
            panic!("People is an area")
        };
        assert_eq!(people.id, "people");
        assert_eq!(people.children.len(), 4);
        let Node::Card(anna) = &people.children[0] else {
            panic!("Anna is a card")
        };
        assert_eq!(anna.title(), Some("Anna Berg"));
        assert_eq!(anna.lines.len(), 1);
        assert_eq!(anna.lines[0].text(), "Program sponsor.");
    }

    #[test]
    fn connections_are_sentences_with_links() {
        let b = Board::parse(PROGRAM);
        let anna = b.card("anna-berg").unwrap();
        assert_eq!(anna.connections.len(), 2);
        assert_eq!(anna.connections[0].label, "Manages");
        assert_eq!(anna.connections[0].targets, vec!["erik-lund", "sara-ek"]);
        assert_eq!(anna.connections[1].label, "Told me to contact");
        assert_eq!(anna.connections[1].note, "2026-10-14");
        let all = b.connections();
        assert_eq!(all.len(), 3);
        assert!(
            all.iter()
                .any(|(f, t, _)| f == "anna-berg" && t == "johan-holm")
        );
    }

    #[test]
    fn task_list_becomes_one_checklist_card() {
        let b = Board::parse(PROGRAM);
        let next = b.area("next-steps").map(|a| a.children.len());
        // "Next steps" has no sub-headings, so it is a card holding the list.
        assert!(next.is_none());
        let card = b.card("next-steps").unwrap();
        assert_eq!(card.lines.len(), 2);
        assert_eq!(card.lines[0].kind, LineKind::Task(true));
        let at = card.lines[1].task_at.unwrap();
        assert_eq!(&PROGRAM[at..at + 3], "[ ]");
    }

    #[test]
    fn any_markdown_opens() {
        for src in [
            "",
            "just text",
            "- a\n  - b\n- c\n",
            "```rust\nfn main() {}\n```\n",
            "## Only h2\n\n### a\n\n#### deep\n\ntext\n",
            "| a |\n|---|\n| 1 |\n",
            "# One\n# Two\n",
            "> quote\n\n---\n\n<div>html</div>\n",
        ] {
            let b = Board::parse(src);
            assert!(!b.walk().is_empty(), "{src:?}");
        }
    }

    #[test]
    fn loose_lists_become_nested_item_cards() {
        let b = Board::parse("- idea\n  - sub one\n  - sub two\n- second\n");
        assert_eq!(b.root.children.len(), 2);
        let Node::Card(first) = &b.root.children[0] else {
            panic!()
        };
        assert_eq!(first.kind, CardKind::Item);
        assert_eq!(first.children.len(), 2);
        assert_eq!(first.children[1].lines[0].text(), "sub two");
    }

    #[test]
    fn appendix_is_read_and_not_content() {
        let src = format!("{PROGRAM}\n<!-- skiss\nAnna Berg: at 10, 20; red\n-->\n");
        let b = Board::parse(&src);
        assert_eq!(b.appendix.get("Anna Berg").unwrap().at, Some((10, 20)));
        assert!(b.appendix_range.is_some());
        let card = b.card("next-steps").unwrap();
        assert!(card.range.end <= b.appendix_range.unwrap().start);
    }

    #[test]
    fn duplicate_titles_get_unique_ids_and_keys() {
        let b = Board::parse("## Notes\n\na\n\n## Notes\n\nb\n");
        assert!(b.card("notes").is_some());
        assert!(b.card("notes-1").is_some());
        assert_eq!(b.card("notes-1").unwrap().key, "Notes (2)");
    }

    #[test]
    fn html_blocks_read_as_text() {
        let html = "<p align=\"center\">\n  <strong>mdeck 2</strong> is here &middot; <a href=\"x\">news</a>\n</p>";
        assert_eq!(html_text(html), "mdeck 2 is here · news");
        let badges = "<p><a href=\"x\"><img src=\"https://img/ci.svg\" alt=\"CI\"></a> <img src=\"https://b\" alt=\"crates.io\"></p>";
        assert_eq!(html_text(badges), "CI · crates.io");
        let logo =
            "<p align=\"center\"><img src=\"media/logo.png\" alt=\"mdeck\" width=\"560\"></p>";
        assert_eq!(html_local_image(logo).as_deref(), Some("media/logo.png"));
        assert_eq!(html_text("<!-- just a comment -->"), "");
    }

    #[test]
    fn loose_area_connections() {
        let b = Board::parse("## Team A\n\n- Depends on [Team B](#team-b)\n\n### x\n\n## Team B\n");
        let a = b.area("team-a").unwrap();
        assert_eq!(a.connections.len(), 1);
        assert_eq!(b.connections()[0].1, "team-b");
    }
}

#[cfg(test)]
mod demo {
    use super::*;

    /// The demo boards in `examples/` stay valid: every arrow points at a real card, every
    /// decorated key exists, and opening a board changes nothing.
    fn check_demo(src: &str, title: &str, min_connections: usize) {
        let b = Board::parse(src);
        assert_eq!(b.title.as_ref().map(|t| t.text.as_str()), Some(title));
        for node in b.walk() {
            for c in node.connections() {
                for t in &c.targets {
                    assert!(
                        b.anchors.contains_key(t),
                        "{} links to missing #{t}",
                        node.key()
                    );
                }
            }
        }
        assert!(
            b.connections().len() >= min_connections,
            "{}",
            b.connections().len()
        );
        for key in b.appendix.entries.iter().map(|e| e.key.clone()) {
            assert!(
                b.keys().contains(&key),
                "appendix key {key} matches no card"
            );
        }
        for node in b.walk() {
            if let NodeRef::Card(c) = node {
                let title = c.heading.as_ref().map(|h| h.text.as_str());
                assert!(
                    crate::board::edit::set_card_text(&b, src, &c.id, title, &c.lines).is_none(),
                    "{} would be rewritten",
                    c.key
                );
            }
        }
    }

    #[test]
    fn apollo_demo_is_consistent() {
        check_demo(
            include_str!("../../../../examples/apollo.md"),
            "Apollo: men on the Moon before 1970",
            15,
        );
    }

    #[test]
    fn studio_demo_is_consistent() {
        check_demo(
            include_str!("../../../../examples/studio.md"),
            "Studio notebook",
            6,
        );
    }

    #[test]
    fn orgchart_demo_is_consistent() {
        check_demo(
            include_str!("../../../../examples/orgchart.md"),
            "Kestrel Labs",
            6,
        );
    }

    #[test]
    fn stockholm_demo_is_consistent() {
        check_demo(
            include_str!("../../../../examples/stockholm.md"),
            "Stockholm: a history in six chapters",
            0,
        );
    }
}
