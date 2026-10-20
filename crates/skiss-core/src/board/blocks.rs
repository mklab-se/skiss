//! The block structure of a Markdown file, with the byte range of every block.
//!
//! pulldown-cmark does the CommonMark heavy lifting; this module keeps only what skiss needs to
//! build a board and to edit the file surgically: block kinds, their exact source ranges, list
//! nesting and task markers.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Heading(u8),
    Paragraph,
    /// `Some(start number)` for ordered lists.
    List(Option<u64>),
    Item,
    Quote,
    Code(String),
    Html,
    Table,
    Rule,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub kind: BlockKind,
    pub range: Range<usize>,
    pub children: Vec<Block>,
    /// For list items: checked state and byte offset of the `[` of a task marker.
    pub task: Option<(bool, usize)>,
}

impl Block {
    fn new(kind: BlockKind, range: Range<usize>) -> Self {
        Block {
            kind,
            range,
            children: Vec::new(),
            task: None,
        }
    }

    /// Whether any direct item of this list is a task (`- [ ]`).
    pub fn is_task_list(&self) -> bool {
        self.children.iter().any(|i| i.task.is_some())
    }
}

fn level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

pub fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// Parses `src` into its top-level blocks.
pub fn parse_blocks(src: &str) -> Vec<Block> {
    let mut stack: Vec<Block> = vec![Block::new(BlockKind::Rule, 0..src.len())];
    let mut table_depth = 0u32;
    for (event, range) in Parser::new_ext(src, options()).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if table_depth > 0 {
                    if matches!(tag, Tag::Table(_)) {
                        table_depth += 1;
                    }
                    continue;
                }
                let kind = match tag {
                    Tag::Paragraph => BlockKind::Paragraph,
                    Tag::Heading { level: l, .. } => BlockKind::Heading(level(l)),
                    Tag::BlockQuote(_) => BlockKind::Quote,
                    Tag::CodeBlock(CodeBlockKind::Fenced(lang)) => {
                        BlockKind::Code(lang.to_string())
                    }
                    Tag::CodeBlock(CodeBlockKind::Indented) => BlockKind::Code(String::new()),
                    Tag::HtmlBlock => BlockKind::Html,
                    Tag::List(start) => BlockKind::List(start),
                    Tag::Item => BlockKind::Item,
                    Tag::Table(_) => {
                        table_depth = 1;
                        BlockKind::Table
                    }
                    _ => continue,
                };
                stack.push(Block::new(kind, range));
            }
            Event::End(end) => {
                if table_depth > 0 {
                    if matches!(end, TagEnd::Table) {
                        table_depth -= 1;
                        if table_depth == 0 {
                            close(&mut stack);
                        }
                    }
                    continue;
                }
                let block_end = matches!(
                    end,
                    TagEnd::Paragraph
                        | TagEnd::Heading(_)
                        | TagEnd::BlockQuote(_)
                        | TagEnd::CodeBlock
                        | TagEnd::HtmlBlock
                        | TagEnd::List(_)
                        | TagEnd::Item
                );
                if block_end {
                    close(&mut stack);
                }
            }
            Event::Rule => {
                if let Some(top) = stack.last_mut() {
                    top.children.push(Block::new(BlockKind::Rule, range));
                }
            }
            Event::TaskListMarker(checked) => {
                if let Some(item) = stack.iter_mut().rev().find(|b| b.kind == BlockKind::Item) {
                    item.task = Some((checked, range.start));
                }
            }
            _ => {}
        }
    }
    while stack.len() > 1 {
        close(&mut stack);
    }
    stack.pop().map(|root| root.children).unwrap_or_default()
}

fn close(stack: &mut Vec<Block>) {
    if stack.len() > 1 {
        let done = stack.pop().expect("len > 1");
        stack.last_mut().expect("root").children.push(done);
    }
}

/// `range` with surrounding whitespace removed.
pub fn trim(src: &str, range: Range<usize>) -> Range<usize> {
    let s = &src[range.clone()];
    let start = range.start + (s.len() - s.trim_start().len());
    let end = range.start + s.trim_end().len();
    if end < start {
        start..start
    } else {
        start..end
    }
}

/// Byte offset of the start of the line containing `pos`.
pub fn line_start(src: &str, pos: usize) -> usize {
    src[..pos].rfind('\n').map_or(0, |i| i + 1)
}

/// Byte offset just past the end of the line containing `pos` (after its `\n`, or the file end).
pub fn line_end_incl(src: &str, pos: usize) -> usize {
    src[pos..].find('\n').map_or(src.len(), |i| pos + i + 1)
}

/// Byte offset of the end of the line containing `pos` (before its `\n`).
pub fn line_end(src: &str, pos: usize) -> usize {
    src[pos..].find('\n').map_or(src.len(), |i| pos + i)
}

/// The range of a list item's own text: after the marker (and task box), before nested lists.
pub fn item_text_range(src: &str, item: &Block) -> Range<usize> {
    let mut start = item.range.start;
    let bytes = src.as_bytes();
    // Skip the marker: `-`, `*`, `+` or `12.` / `12)`.
    while start < item.range.end && bytes[start].is_ascii_digit() {
        start += 1;
    }
    if start < item.range.end && matches!(bytes[start], b'-' | b'*' | b'+' | b'.' | b')') {
        start += 1;
    }
    if let Some((_, at)) = item.task {
        start = (at + 3).min(item.range.end);
    }
    while start < item.range.end && matches!(bytes[start], b' ' | b'\t') {
        start += 1;
    }
    let end = item
        .children
        .iter()
        .find(|c| matches!(c.kind, BlockKind::List(_)))
        .map_or(item.range.end, |l| line_start(src, l.range.start));
    let r = trim(src, start.min(end)..end);
    if r.is_empty() {
        // An empty item ("- "): its text goes right after the marker, on the marker's line.
        let at = start.min(line_end(src, item.range.start));
        return at..at;
    }
    r
}

/// Heading text without the `#` marks (or the setext underline).
pub fn heading_text(src: &str, range: Range<usize>) -> String {
    let s = &src[range];
    let first = s.lines().next().unwrap_or("");
    let t = first.trim();
    if t.starts_with('#') {
        let t = t.trim_start_matches('#').trim();
        // Optional closing sequence: "## Title ##".
        let stripped = t.trim_end_matches('#');
        if stripped.len() != t.len() && (stripped.is_empty() || stripped.ends_with(' ')) {
            return stripped.trim().to_string();
        }
        t.to_string()
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_top_level_blocks_with_ranges() {
        let src = "# Title\n\nHello\nworld\n\n- a\n- b\n";
        let blocks = parse_blocks(src);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].kind, BlockKind::Heading(1));
        assert_eq!(&src[trim(src, blocks[1].range.clone())], "Hello\nworld");
        assert_eq!(blocks[2].kind, BlockKind::List(None));
        assert_eq!(blocks[2].children.len(), 2);
    }

    #[test]
    fn item_text_skips_marker_task_and_children() {
        let src = "- [ ] Do it\n- [x] Done\n  - child\n1. First\n";
        let blocks = parse_blocks(src);
        let items = &blocks[0].children;
        assert_eq!(&src[item_text_range(src, &items[0])], "Do it");
        assert_eq!(items[0].task.map(|t| t.0), Some(false));
        assert_eq!(
            &src[items[0].task.unwrap().1..items[0].task.unwrap().1 + 3],
            "[ ]"
        );
        assert_eq!(&src[item_text_range(src, &items[1])], "Done");
        let ordered = &blocks[1].children[0];
        assert_eq!(&src[item_text_range(src, ordered)], "First");
    }

    #[test]
    fn empty_item_text_sits_on_its_own_line() {
        let src = "- a\n- \n- b\n";
        let blocks = parse_blocks(src);
        let empty = &blocks[0].children[1];
        let r = item_text_range(src, empty);
        assert!(r.is_empty());
        assert_eq!(&src[..r.start], "- a\n- ");
    }

    #[test]
    fn heading_text_strips_marks() {
        let src = "## Hello world ##\n";
        assert_eq!(heading_text(src, 0..src.len()), "Hello world");
        let src = "Setext\n======\n";
        assert_eq!(heading_text(src, 0..src.len()), "Setext");
    }

    #[test]
    fn tables_are_one_block() {
        let src = "| a | b |\n|---|---|\n| 1 | 2 |\n\nAfter\n";
        let blocks = parse_blocks(src);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Table);
    }
}
