//! Edits as splices: every operation replaces the smallest span of the file it can, and leaves
//! every other byte alone, so Git diffs stay small and hand-written files survive skiss.

use std::ops::Range;

use super::appendix::Appendix;
use super::blocks::{self, BlockKind};
use super::inline::{self, Span};
use super::model::{Board, Card, CardKind, Line, LineKind, NodeRef};

/// A source edit result: the new text and where the touched element now starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edited {
    pub source: String,
    pub at: usize,
}

fn splice(src: &str, range: Range<usize>, with: &str) -> String {
    let mut out = String::with_capacity(src.len() + with.len());
    out.push_str(&src[..range.start]);
    out.push_str(with);
    out.push_str(&src[range.end..]);
    out
}

/// Inserts a block of text at a line boundary with one blank line around it.
/// Returns the new source and the offset where `block` starts.
pub fn insert_block(src: &str, at: usize, block: &str) -> (String, usize) {
    let before = &src[..at];
    let after = &src[at..];
    let prefix = if before.is_empty() || before.ends_with("\n\n") {
        ""
    } else if before.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let suffix = if after.is_empty() || after.starts_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let start = at + prefix.len();
    (format!("{before}{prefix}{block}{suffix}{after}"), start)
}

/// Removes `range` and the blank lines that separated it from what follows.
fn remove_block(src: &str, range: Range<usize>) -> String {
    let mut end = range.end;
    let bytes = src.as_bytes();
    while end < src.len() && matches!(bytes[end], b'\n' | b' ' | b'\t' | b'\r') {
        end += 1;
    }
    let mut start = range.start;
    if end >= src.len() {
        // Removing the tail: also drop the blank lines before it, keep one final newline.
        while start > 0 && matches!(bytes[start - 1], b'\n' | b' ' | b'\t' | b'\r') {
            start -= 1;
        }
        let mut out = src[..start].to_string();
        if !out.is_empty() {
            out.push('\n');
        }
        return out;
    }
    splice(src, start..end, "")
}

// --- Text of a card ------------------------------------------------------------------------

fn marker_width(kind: &LineKind) -> usize {
    match kind {
        LineKind::Numbered(n) => n.to_string().len() + 2,
        _ => 2,
    }
}

/// Writes card body lines as Markdown. Lines that are unchanged since parsing (same content and
/// still pointing at their original source) are copied verbatim.
pub fn lines_to_markdown(src: &str, lines: &[Line], original: &[Line]) -> String {
    let mut out = String::new();
    let mut widths: Vec<usize> = Vec::new();
    let mut prev_list = false;
    let mut first = true;
    for line in lines {
        let empty = line.spans.iter().all(|s| s.text.trim().is_empty());
        // Empty paragraphs and list items (a fresh line still being typed) are not written:
        // "- [ ] " would leave a trailing space behind and is not even a task to Markdown.
        if empty && (line.is_list() || matches!(line.kind, LineKind::Paragraph | LineKind::Quote)) {
            continue;
        }
        let is_list = line.is_list();
        if !first {
            out.push_str(if is_list && prev_list { "\n" } else { "\n\n" });
        }
        first = false;
        prev_list = is_list;

        let d = line.depth as usize;
        if is_list {
            widths.truncate(d);
            while widths.len() < d {
                widths.push(2);
            }
        }
        let unchanged = !line.orig.is_empty()
            && original.iter().any(|o| {
                o.orig == line.orig
                    && o.kind == line.kind
                    && o.depth == line.depth
                    && o.spans == line.spans
            });
        if unchanged {
            out.push_str(&src[line.orig.clone()]);
        } else {
            let indent = " ".repeat(widths.iter().take(d).sum());
            // Never leave trailing spaces: they are invisible, and a line saved mid-typing
            // ("word ") must not leave a stray space behind when the next save splices in.
            let md = inline::to_markdown(&line.spans).trim_end().to_string();
            match &line.kind {
                LineKind::Paragraph => out.push_str(&md),
                LineKind::Bullet => out.push_str(&format!("{indent}- {md}")),
                LineKind::Numbered(n) => out.push_str(&format!("{indent}{n}. {md}")),
                LineKind::Task(done) => out.push_str(&format!(
                    "{indent}- [{}] {md}",
                    if *done { 'x' } else { ' ' }
                )),
                LineKind::Quote => out.push_str(&format!("> {md}")),
                LineKind::Code(lang) => out.push_str(&format!(
                    "```{lang}\n{}\n```",
                    inline::plain_text(&line.spans)
                )),
                LineKind::Image { alt, url } => out.push_str(&format!("![{alt}]({url})")),
                LineKind::Raw | LineKind::Table | LineKind::Html { .. } => {
                    out.push_str(&inline::plain_text(&line.spans))
                }
            }
        }
        if is_list {
            widths.truncate(d);
            widths.push(marker_width(&line.kind));
        }
    }
    out
}

/// Replaces a card's title and body. Returns `None` if nothing changed.
pub fn set_card_text(
    board: &Board,
    src: &str,
    card_id: &str,
    title: Option<&str>,
    lines: &[Line],
) -> Option<Edited> {
    let card = board.card(card_id)?;
    match card.kind {
        CardKind::Item => {
            let md =
                inline::to_markdown(&lines.first().map(|l| l.spans.clone()).unwrap_or_default());
            if md == src[card.body.clone()] {
                return None;
            }
            Some(Edited {
                source: splice(src, card.body.clone(), &md),
                at: card.range.start,
            })
        }
        CardKind::Block | CardKind::Tasks => {
            let md = lines_to_markdown(src, lines, &card.lines);
            if md == src[card.range.clone()] {
                return None;
            }
            if md.trim().is_empty() {
                return Some(Edited {
                    source: remove_block(src, card.range.clone()),
                    at: card.range.start,
                });
            }
            Some(Edited {
                source: splice(src, card.range.clone(), &md),
                at: card.range.start,
            })
        }
        CardKind::Section => set_section_text(board, src, card, title, lines),
    }
}

fn set_section_text(
    board: &Board,
    src: &str,
    card: &Card,
    title: Option<&str>,
    lines: &[Line],
) -> Option<Edited> {
    let heading = card.heading.as_ref()?;
    let same_lines = lines.len() == card.lines.len()
        && lines
            .iter()
            .zip(&card.lines)
            .all(|(a, b)| a.kind == b.kind && a.depth == b.depth && a.spans == b.spans);
    let old_body = &src[card.body.clone()];
    let mut body = if same_lines {
        old_body.to_string()
    } else {
        lines_to_markdown(src, lines, &card.lines)
    };
    if !same_lines && !card.connections.is_empty() {
        let conns: Vec<&str> = card
            .connections
            .iter()
            .map(|c| src[c.line.clone()].trim_end_matches(['\n', '\r']))
            .collect();
        let joined = conns.join("\n");
        // Keep the separator the file used between the text and its connection sentences.
        let first_conn = card.connections[0].line.start;
        let sep = card
            .lines
            .iter()
            .map(|l| l.orig.end)
            .filter(|e| *e <= first_conn)
            .max()
            .map(|e| src[e..first_conn].to_string())
            .filter(|s| s.trim().is_empty() && !s.is_empty())
            .unwrap_or_else(|| "\n\n".to_string());
        if body.is_empty() {
            body = joined;
        } else {
            body = format!("{body}{sep}{joined}");
        }
    }
    let new_title = title.map(str::trim);
    let title_changed = new_title.is_some_and(|t| t != heading.text);
    if !title_changed && body == old_body {
        return None;
    }

    // Body first (it comes after the heading, so the heading offsets stay valid).
    let mut out = src.to_string();
    if body != old_body {
        if card.body.is_empty() {
            // No body yet: add one right after the heading line.
            let at = blocks::line_end_incl(
                src,
                heading.line.end.saturating_sub(1).max(heading.line.start),
            );
            let at = at.max(heading.line.end);
            let insert = if src[..at].ends_with('\n') {
                format!("{body}\n")
            } else {
                format!("\n{body}\n")
            };
            out = splice(&out, at..at, &insert);
        } else if body.is_empty() {
            // Remove the body and the blank line in front of it.
            let mut start = card.body.start;
            while start > heading.line.end && out.as_bytes()[start - 1] == b'\n' {
                start -= 1;
            }
            let end = card.body.end;
            out = splice(&out, start..end, "");
        } else {
            out = splice(&out, card.body.clone(), &body);
        }
    }
    if let Some(t) = new_title
        && title_changed
    {
        let hashes = "#".repeat(heading.level as usize);
        let line = if t.is_empty() {
            hashes
        } else {
            format!("{hashes} {t}")
        };
        out = splice(&out, heading.line.clone(), &line);
        // Keep the decoration with the card.
        let new_board = Board::parse(&out);
        let old_key = card.key.clone();
        if board.appendix.get(&old_key).is_some()
            && let Some(newcard) = new_board
                .walk()
                .into_iter()
                .find(|n| n.range().start == card.range.start)
        {
            let new_key = newcard.key().to_string();
            out = with_appendix(&new_board, &out, |a| a.rename(&old_key, &new_key));
        }
    }
    Some(Edited {
        source: out,
        at: card.range.start,
    })
}

/// Checks or unchecks the task whose `[ ]` box starts at `at`.
pub fn toggle_task(src: &str, at: usize) -> Option<String> {
    let mark = src.get(at..at + 3)?;
    let new = match mark {
        "[ ]" => "[x]",
        "[x]" | "[X]" => "[ ]",
        _ => return None,
    };
    Some(splice(src, at..at + 3, new))
}

// --- Creating and removing ---------------------------------------------------------------

/// Adds a new, empty titled card to an area (after `after` if given, else last).
/// The new heading has no text yet; the caller starts editing it right away.
pub fn insert_card(
    board: &Board,
    src: &str,
    area_id: &str,
    after: Option<&str>,
    title: &str,
) -> Option<Edited> {
    let area = board.area(area_id)?;
    // A brain-dump list stays a list: a new card next to list items is a new item.
    if let Some(after_id) = after
        && let Some(card) = board.card(after_id)
        && card.kind == CardKind::Item
    {
        return insert_item_after(src, card, title);
    }
    if after.is_none()
        && !area.children.is_empty()
        && area
            .children
            .iter()
            .all(|n| matches!(n, super::model::Node::Card(c) if c.kind == CardKind::Item))
        && let Some(super::model::Node::Card(last)) = area.children.last()
    {
        return insert_item_after(src, last, title);
    }
    let level = area.child_level.clamp(1, 6) as usize;
    let at = match after.and_then(|id| board.node(id)) {
        Some(n) if !matches!(n, NodeRef::Area(a) if a.id.is_empty()) => n.range().end,
        _ => {
            if area.id.is_empty() {
                board.content_end()
            } else {
                area.section.end
            }
        }
    };
    let at = at.min(board.content_end());
    let hashes = "#".repeat(level);
    let block = if title.is_empty() {
        hashes
    } else {
        format!("{hashes} {title}")
    };
    let (source, at) = insert_block(src, at, &block);
    Some(Edited { source, at })
}

fn insert_item_after(src: &str, card: &Card, text: &str) -> Option<Edited> {
    let line_from = blocks::line_start(src, card.range.start);
    let indent = &src[line_from..card.range.start];
    let marker: String = src[card.range.start..]
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect();
    let marker = if marker.ends_with('.') || marker.ends_with(')') {
        let n: u64 = marker.trim_end_matches(['.', ')']).parse().unwrap_or(1);
        format!("{}.", n + 1)
    } else {
        marker
    };
    let end = blocks::line_end_incl(src, card.range.end.saturating_sub(1).max(card.range.start));
    let mut insert = String::new();
    if !src[..end].ends_with('\n') {
        insert.push('\n');
    }
    let at = end + insert.len();
    insert.push_str(&format!("{indent}{marker} {text}\n"));
    Some(Edited {
        source: splice(src, end..end, &insert),
        at: at + indent.len(),
    })
}

/// Deletes a card or an area with everything in it.
pub fn delete_node(board: &Board, src: &str, id: &str) -> Option<String> {
    let node = board.node(id)?;
    let mut keys = Vec::new();
    collect_keys(node, &mut keys);
    let out = match node {
        NodeRef::Card(c) if c.kind == CardKind::Item => {
            let from = blocks::line_start(src, c.range.start);
            let to = blocks::line_end_incl(src, c.range.end.saturating_sub(1).max(c.range.start));
            splice(src, from..to, "")
        }
        NodeRef::Card(c) if c.kind == CardKind::Section => splice(src, c.range.clone(), ""),
        NodeRef::Card(c) => remove_block(src, c.range.clone()),
        NodeRef::Area(a) if a.id.is_empty() => return None,
        NodeRef::Area(a) => splice(src, a.section.clone(), ""),
    };
    let nb = Board::parse(&out);
    if keys.iter().any(|k| nb.appendix.get(k).is_some()) {
        Some(with_appendix(&nb, &out, |a| {
            for k in &keys {
                a.remove(k);
            }
        }))
    } else {
        Some(out)
    }
}

fn collect_keys(node: NodeRef<'_>, out: &mut Vec<String>) {
    match node {
        NodeRef::Card(c) => {
            out.push(c.key.clone());
            for ch in &c.children {
                collect_keys(NodeRef::Card(ch), out);
            }
        }
        NodeRef::Area(a) => {
            out.push(a.key.clone());
            for ch in &a.children {
                match ch {
                    super::model::Node::Area(x) => collect_keys(NodeRef::Area(x), out),
                    super::model::Node::Card(x) => collect_keys(NodeRef::Card(x), out),
                }
            }
        }
    }
}

// --- Connections ---------------------------------------------------------------------------

fn capitalize(s: &str) -> String {
    let mut c = s.trim().chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Writes "- Label [Target](#anchor)" under `from`. Joins an existing sentence with the same
/// label ("Manages [Erik](#erik) and [Sara](#sara)") instead of adding a line when it can.
pub fn add_connection(
    board: &Board,
    src: &str,
    from: &str,
    to: &str,
    label: &str,
) -> Option<String> {
    let target = board.node(to)?;
    let anchor = board.anchor_of(to)?.to_string();
    let title = target.heading()?.text.clone();
    let label = if label.trim().is_empty() {
        "Related to".to_string()
    } else {
        capitalize(label)
    };
    let link = format!(
        "[{}]({})",
        escape_link_text(&title),
        format_args!("#{anchor}")
    );
    let source = board.node(from)?;
    if source.id() == to {
        return None;
    }
    let conns = source.connections();
    if conns.iter().any(|c| c.targets.contains(&anchor)) {
        return None;
    }
    // Same label, no note: extend the sentence.
    if let Some(c) = conns
        .iter()
        .find(|c| c.label.eq_ignore_ascii_case(&label) && c.note.is_empty())
    {
        let end = blocks::line_end(src, c.line.start);
        let line = src[c.line.start..end].trim_end();
        let at = c.line.start + line.len();
        let joiner = if c.targets.len() >= 2 {
            // "A, B and C": turn the last " and " into ", ".
            let text = &src[c.line.start..at];
            if let Some(i) = text.rfind(" and [") {
                let s = splice(src, c.line.start + i..c.line.start + i + 5, ",");
                let at = at - 4;
                return Some(splice(&s, at..at, &format!(" and {link}")));
            }
            " and "
        } else {
            " and "
        };
        return Some(splice(src, at..at, &format!("{joiner}{link}")));
    }
    let line = format!("- {label} {link}");
    if let Some(last) = conns.last() {
        let at = last.line.end;
        let prefix = if src[..at].ends_with('\n') { "" } else { "\n" };
        return Some(splice(src, at..at, &format!("{prefix}{line}\n")));
    }
    match source {
        NodeRef::Card(card) if card.kind == CardKind::Section => {
            if card.body.is_empty() {
                let heading = card.heading.as_ref()?;
                let at = blocks::line_end_incl(
                    src,
                    heading.line.end.saturating_sub(1).max(heading.line.start),
                );
                let prefix = if src[..at].ends_with('\n') { "" } else { "\n" };
                Some(splice(src, at..at, &format!("{prefix}{line}\n")))
            } else if card
                .lines
                .last()
                .is_some_and(|l| matches!(l.kind, LineKind::Bullet))
            {
                let at = card.body.end;
                Some(splice(src, at..at, &format!("\n{line}")))
            } else {
                let at = blocks::line_end_incl(src, card.body.end);
                Some(insert_block(src, at, &line).0)
            }
        }
        NodeRef::Area(area) if !area.id.is_empty() => {
            let at = area.intro.end.min(board.content_end());
            Some(insert_block(src, at, &line).0)
        }
        _ => None,
    }
}

fn escape_link_text(t: &str) -> String {
    t.replace('[', "\\[").replace(']', "\\]")
}

/// Removes one target from a connection sentence (the whole line if it was the only one).
pub fn remove_connection(board: &Board, src: &str, from: &str, to: &str) -> Option<String> {
    let anchor = board.anchor_of(to)?;
    let source = board.node(from)?;
    let conn = source
        .connections()
        .iter()
        .find(|c| c.targets.iter().any(|t| t == anchor))?;
    if conn.targets.len() == 1 {
        return Some(splice(src, conn.line.clone(), ""));
    }
    let line = &src[conn.line.clone()];
    let needle = format!("(#{anchor})");
    let link_end = line.find(&needle)? + needle.len();
    let link_start = line[..link_end].rfind('[')?;
    // Take the separator in front of the link (or after it, for the first link) with it.
    let before = &line[..link_start];
    let sep_start = before
        .rfind(" and ")
        .or_else(|| before.rfind(", "))
        .filter(|i| {
            before[*i..]
                .trim_start_matches([' ', ','])
                .trim_start_matches("and")
                .trim()
                .is_empty()
        });
    let (cut_from, cut_to) = match sep_start {
        Some(i) => (i, link_end),
        None => {
            let after = &line[link_end..];
            let skip = if after.starts_with(" and ") {
                5
            } else if after.starts_with(", ") {
                2
            } else {
                0
            };
            (link_start, link_end + skip)
        }
    };
    let new_line = format!("{}{}", &line[..cut_from], &line[cut_to..]);
    Some(splice(src, conn.line.clone(), &new_line))
}

// --- Moving and grouping -------------------------------------------------------------------

/// Shifts every ATX heading in `text` by `delta` levels (clamped to 1..=6).
pub fn shift_headings(text: &str, delta: i32) -> String {
    if delta == 0 {
        return text.to_string();
    }
    let mut out = text.to_string();
    let mut heads: Vec<(usize, u8)> = Vec::new();
    for b in blocks::parse_blocks(text) {
        if let BlockKind::Heading(l) = b.kind
            && text[b.range.start..].trim_start().starts_with('#')
        {
            let start = b.range.start
                + (text[b.range.start..].len() - text[b.range.start..].trim_start().len());
            heads.push((start, l));
        }
    }
    for (start, level) in heads.into_iter().rev() {
        let new = (level as i32 + delta).clamp(1, 6) as usize;
        out = splice(&out, start..start + level as usize, &"#".repeat(new));
    }
    out
}

fn node_level(node: NodeRef<'_>) -> Option<u8> {
    node.heading().map(|h| h.level)
}

/// The text a node would take with it when moved, re-levelled for a parent with `child_level`.
fn node_text(src: &str, node: NodeRef<'_>, child_level: u8) -> String {
    match node {
        NodeRef::Card(c) if c.kind == CardKind::Item => {
            // Re-dedent the item and its children to top level.
            let from = blocks::line_start(src, c.range.start);
            let indent = c.range.start - from;
            let to = blocks::line_end(src, c.range.end.saturating_sub(1).max(c.range.start));
            src[from..to]
                .lines()
                .map(|l| {
                    if l.len() >= indent {
                        &l[indent.min(l.len() - l.trim_start().len())..]
                    } else {
                        l
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => {
            let r = blocks::trim(src, node.range());
            let text = &src[r];
            match node_level(node) {
                Some(l) => shift_headings(text, child_level as i32 - l as i32),
                None => text.to_string(),
            }
        }
    }
}

fn remove_node_text(src: &str, node: NodeRef<'_>) -> String {
    match node {
        NodeRef::Card(c) if c.kind == CardKind::Item => {
            let from = blocks::line_start(src, c.range.start);
            let to = blocks::line_end_incl(src, c.range.end.saturating_sub(1).max(c.range.start));
            splice(src, from..to, "")
        }
        NodeRef::Card(c) if c.kind == CardKind::Section => splice(src, c.range.clone(), ""),
        NodeRef::Area(a) => splice(src, a.section.clone(), ""),
        NodeRef::Card(c) => remove_block(src, c.range.clone()),
    }
}

/// Moves a card or area into another area (to the end of it).
pub fn move_node(board: &Board, src: &str, id: &str, to_area: &str) -> Option<Edited> {
    let node = board.node(id)?;
    let target = board.area(to_area)?;
    if board.parent_area(id).is_some_and(|p| p.id == to_area) {
        return None;
    }
    // Never into itself.
    if board
        .find(to_area)
        .is_some_and(|(_, path)| path.iter().any(|a| a.id == id))
        || to_area == id
    {
        return None;
    }
    let text = node_text(src, node, target.child_level.clamp(1, 6));
    let range = node.range();
    let is_section = node.heading().is_some();
    let at = if is_section {
        if target.id.is_empty() {
            board.content_end()
        } else {
            target.section.end
        }
    } else {
        target.intro.end
    };
    // Cut and paste: adjust the insertion point if it comes after the cut.
    let removed = remove_node_text(src, node);
    let shrink = src.len() - removed.len();
    let at = if at >= range.end {
        at - shrink
    } else {
        at.min(removed.len())
    };
    let (source, at) = insert_block(&removed, at.min(removed.len()), &text);
    Some(Edited { source, at })
}

/// Puts several siblings into a new sub-area titled `title`, at the end of their parent.
pub fn group(board: &Board, src: &str, ids: &[String], title: &str) -> Option<Edited> {
    let first = ids.first()?;
    let parent = board.parent_area(first)?;
    if ids
        .iter()
        .any(|id| board.parent_area(id).map(|p| p.id.as_str()) != Some(parent.id.as_str()))
    {
        return None;
    }
    let level = parent.child_level.clamp(1, 5);
    let mut nodes: Vec<NodeRef<'_>> = ids.iter().filter_map(|id| board.node(id)).collect();
    nodes.sort_by_key(|n| n.range().start);
    let loose: Vec<String> = nodes
        .iter()
        .filter(|n| n.heading().is_none())
        .map(|n| node_text(src, *n, level + 1))
        .collect();
    let sections: Vec<String> = nodes
        .iter()
        .filter(|n| n.heading().is_some())
        .map(|n| node_text(src, *n, level + 1))
        .collect();
    let mut block = "#".repeat(level as usize);
    if !title.is_empty() {
        block.push(' ');
        block.push_str(title);
    }
    for part in loose.iter().chain(sections.iter()) {
        block.push_str("\n\n");
        block.push_str(part);
    }
    let mut out = src.to_string();
    for n in nodes.iter().rev() {
        out = remove_node_text(&out, *n);
    }
    let nb = Board::parse(&out);
    let at = if parent.id.is_empty() {
        nb.content_end()
    } else {
        nb.area(&parent.id)
            .map(|a| a.section.end)
            .or_else(|| nb.card(&parent.id).map(|c| c.range.end))?
    };
    let (source, at) = insert_block(&out, at, &block);
    Some(Edited { source, at })
}

/// Adds a loose block (an image, a paragraph) to the end of an area's loose content.
pub fn insert_loose(board: &Board, src: &str, area_id: &str, block: &str) -> Option<Edited> {
    let area = board.area(area_id)?;
    let at = area.intro.end.min(board.content_end());
    let (source, at) = insert_block(src, at, block);
    Some(Edited { source, at })
}

/// Appends a line to a titled card's body (an image or a paragraph).
pub fn append_to_card(board: &Board, src: &str, card_id: &str, block: &str) -> Option<Edited> {
    let card = board.card(card_id)?;
    let at = if card.body.is_empty() {
        let h = card.heading.as_ref()?;
        blocks::line_end_incl(src, h.line.end.saturating_sub(1).max(h.line.start))
    } else {
        blocks::line_end_incl(src, card.body.end)
    };
    let (source, at) = insert_block(src, at, block);
    Some(Edited { source, at })
}

// --- The appendix ----------------------------------------------------------------------------

/// Applies `f` to the appendix and writes it back at the end of the file (removing it when empty).
pub fn with_appendix(board: &Board, src: &str, f: impl FnOnce(&mut Appendix)) -> String {
    let mut app = board.appendix.clone();
    f(&mut app);
    let order = board.keys();
    let comment = app.to_comment(&order);
    let empty =
        app.is_empty() || order.iter().all(|k| app.get(k).is_none()) && app.color_names.is_empty();
    match &board.appendix_range {
        Some(r) => {
            if empty {
                remove_block(src, r.clone())
            } else if src[r.clone()] == comment {
                src.to_string()
            } else {
                splice(src, r.clone(), &comment)
            }
        }
        None => {
            if empty {
                return src.to_string();
            }
            let mut out = src.trim_end_matches(['\n', ' ', '\t', '\r']).to_string();
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&comment);
            out.push('\n');
            out
        }
    }
}

/// Spans for a plain string (helper for callers building lines).
pub fn plain_line(kind: LineKind, depth: u8, text: &str) -> Line {
    Line {
        kind,
        depth,
        spans: if text.is_empty() {
            Vec::new()
        } else {
            vec![Span::plain(text)]
        },
        orig: 0..0,
        task_at: None,
    }
}

/// Renames the heading of a card or an area (and keeps its decoration).
pub fn rename_heading(board: &Board, src: &str, id: &str, title: &str) -> Option<Edited> {
    let node = board.node(id)?;
    let heading = node.heading()?;
    let title = title.trim();
    if title == heading.text {
        return None;
    }
    let hashes = "#".repeat(heading.level as usize);
    let line = if title.is_empty() {
        hashes
    } else {
        format!("{hashes} {title}")
    };
    let start = node.range().start;
    let mut out = splice(src, heading.line.clone(), &line);
    let old_key = node.key().to_string();
    if board.appendix.get(&old_key).is_some() {
        let nb = Board::parse(&out);
        if let Some(n) = nb
            .walk()
            .into_iter()
            .find(|n| n.range().start == start && !n.id().is_empty())
        {
            let new_key = n.key().to_string();
            out = with_appendix(&nb, &out, |a| a.rename(&old_key, &new_key));
        }
    }
    Some(Edited {
        source: out,
        at: start,
    })
}

/// The node (not the root) whose source starts at `at`.
pub fn node_at(board: &Board, at: usize) -> Option<NodeRef<'_>> {
    board
        .walk()
        .into_iter()
        .filter(|n| !n.id().is_empty())
        .find(|n| n.range().start == at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::appendix::Color;

    const SRC: &str = "# Apollo

## People

### Anna Berg
Program sponsor.
- Manages [Erik Lund](#erik-lund)

### Erik Lund
Tech lead.

## Next steps

- [ ] Draft risks
";

    #[test]
    fn unchanged_text_is_not_rewritten() {
        let b = Board::parse(SRC);
        let anna = b.card("anna-berg").unwrap();
        assert!(set_card_text(&b, SRC, "anna-berg", Some("Anna Berg"), &anna.lines).is_none());
    }

    #[test]
    fn editing_a_body_touches_only_that_body() {
        let b = Board::parse(SRC);
        let erik = b.card("erik-lund").unwrap();
        let mut lines = erik.lines.clone();
        lines[0].spans = vec![Span::plain("Tech lead, backend.")];
        let out = set_card_text(&b, SRC, "erik-lund", None, &lines)
            .unwrap()
            .source;
        assert_eq!(out, SRC.replace("Tech lead.", "Tech lead, backend."));
    }

    #[test]
    fn editing_a_body_keeps_connection_sentences() {
        let b = Board::parse(SRC);
        let anna = b.card("anna-berg").unwrap();
        let mut lines = anna.lines.clone();
        lines[0].spans = vec![Span::plain("Sponsor, owns the program.")];
        let out = set_card_text(&b, SRC, "anna-berg", None, &lines)
            .unwrap()
            .source;
        assert!(
            out.contains("Sponsor, owns the program.\n- Manages [Erik Lund](#erik-lund)"),
            "{out}"
        );
    }

    #[test]
    fn renaming_a_title_rewrites_the_heading_line() {
        let b = Board::parse(SRC);
        let erik = b.card("erik-lund").unwrap();
        let out = set_card_text(&b, SRC, "erik-lund", Some("Erik L."), &erik.lines)
            .unwrap()
            .source;
        assert_eq!(out, SRC.replace("### Erik Lund", "### Erik L."));
    }

    #[test]
    fn toggling_a_task_flips_one_character() {
        let b = Board::parse(SRC);
        let at = b.card("next-steps").unwrap().lines[0].task_at.unwrap();
        let out = toggle_task(SRC, at).unwrap();
        assert_eq!(out, SRC.replace("- [ ] Draft", "- [x] Draft"));
    }

    #[test]
    fn new_card_goes_at_the_end_of_its_area() {
        let b = Board::parse(SRC);
        let e = insert_card(&b, SRC, "people", None, "Lisa Nord").unwrap();
        assert_eq!(
            e.source,
            SRC.replace(
                "Tech lead.\n\n## Next",
                "Tech lead.\n\n### Lisa Nord\n\n## Next"
            )
        );
        assert_eq!(&e.source[e.at..e.at + 12], "### Lisa Nor");
        let nb = Board::parse(&e.source);
        assert!(nb.card("lisa-nord").is_some());
    }

    #[test]
    fn new_item_in_a_brain_dump_list() {
        let src = "- idea\n- other\n";
        let b = Board::parse(src);
        let e = insert_card(&b, src, "", None, "new").unwrap();
        assert_eq!(e.source, "- idea\n- other\n- new\n");
    }

    #[test]
    fn connection_is_one_new_line() {
        let b = Board::parse(SRC);
        let out = add_connection(&b, SRC, "erik-lund", "anna-berg", "reports to").unwrap();
        assert_eq!(
            out,
            SRC.replace(
                "Tech lead.\n",
                "Tech lead.\n\n- Reports to [Anna Berg](#anna-berg)\n"
            )
        );
        let nb = Board::parse(&out);
        assert!(
            nb.connections()
                .iter()
                .any(|(f, t, _)| f == "erik-lund" && t == "anna-berg")
        );
    }

    #[test]
    fn same_label_extends_the_sentence() {
        let src = SRC.replace("### Erik Lund", "### Sara Ek\nBudget.\n\n### Erik Lund");
        let b = Board::parse(&src);
        let out = add_connection(&b, &src, "anna-berg", "sara-ek", "manages").unwrap();
        assert!(
            out.contains("- Manages [Erik Lund](#erik-lund) and [Sara Ek](#sara-ek)\n"),
            "{out}"
        );
        let b2 = Board::parse(&out);
        let back = remove_connection(&b2, &out, "anna-berg", "sara-ek").unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn delete_removes_the_section() {
        let b = Board::parse(SRC);
        let out = delete_node(&b, SRC, "erik-lund").unwrap();
        assert!(!out.contains("Erik Lund\nTech"));
        assert!(out.contains("## Next steps"));
    }

    #[test]
    fn grouping_shifts_headings_one_level() {
        let src = "## People\n\n### Anna\na\n\n### Erik\ne\n\n### Sara\ns\n";
        let b = Board::parse(src);
        let e = group(&b, src, &["erik".into(), "sara".into()], "Core team").unwrap();
        assert_eq!(
            e.source,
            "## People\n\n### Anna\na\n\n### Core team\n\n#### Erik\ne\n\n#### Sara\ns\n"
        );
        let nb = Board::parse(&e.source);
        assert!(nb.area("core-team").is_some());
    }

    #[test]
    fn moving_into_another_area_relevels() {
        let src = "## A\n\n### x\n\n### y\n\n## B\n\n### z\n\n#### deep\n";
        let b = Board::parse(src);
        let e = move_node(&b, src, "y", "z").unwrap();
        let nb = Board::parse(&e.source);
        assert!(
            nb.parent_area("y").is_some_and(|p| p.id == "z"),
            "{}",
            e.source
        );
        assert!(e.source.contains("#### y"));
    }

    #[test]
    fn appendix_is_added_updated_and_removed() {
        let b = Board::parse(SRC);
        let out = with_appendix(&b, SRC, |a| {
            a.entry_mut("Erik Lund").color = Some(Color::Red)
        });
        assert!(
            out.ends_with("- [ ] Draft risks\n\n<!-- skiss\nErik Lund: red\n-->\n"),
            "{out}"
        );
        let b2 = Board::parse(&out);
        let out2 = with_appendix(&b2, &out, |a| a.entry_mut("Erik Lund").at = Some((10, 20)));
        assert!(out2.contains("Erik Lund: at 10, 20; red\n"));
        let b3 = Board::parse(&out2);
        let out3 = with_appendix(&b3, &out2, |a| a.remove("Erik Lund"));
        assert_eq!(out3, SRC);
    }

    #[test]
    fn rename_keeps_decoration() {
        let src = format!("{SRC}\n<!-- skiss\nErik Lund: red\n-->\n");
        let b = Board::parse(&src);
        let erik = b.card("erik-lund").unwrap();
        let out = set_card_text(&b, &src, "erik-lund", Some("Erik L"), &erik.lines)
            .unwrap()
            .source;
        assert!(out.contains("Erik L: red"), "{out}");
    }
}
