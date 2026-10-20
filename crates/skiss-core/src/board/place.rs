//! Placing a new note where it belongs: the map of a board's areas (what an AI chooses from),
//! turning a quick note into a card, and inserting it in the right spot.
//!
//! Deciding *which* area is left to the caller (an AI judge, or a fixed inbox); everything here
//! is plain text work, so a placement is one ordinary, minimal splice of the file.

use super::blocks::{self, BlockKind};
use super::edit::{Edited, insert_block, shift_headings};
use super::model::{Area, Board, Card, LineKind, Node};

/// The name of the area notes land in when nothing better is known.
pub const INBOX: &str = "Inbox";

/// An area a note could go to, described for a model that picks one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spot {
    /// The area id (empty for the top level of the board).
    pub id: String,
    /// "People > Engineers", or "Top level" for the board itself.
    pub path: String,
    /// Titles of the cards directly in it (a sample, in document order).
    pub cards: Vec<String>,
}

impl Spot {
    /// A one-line description: where it is and what is already there.
    pub fn describe(&self) -> String {
        if self.cards.is_empty() {
            self.path.clone()
        } else {
            format!("{} (holds: {})", self.path, self.cards.join("; "))
        }
    }
}

/// Every area on the board, outermost first, plus the top level when it has cards of its own.
pub fn spots(board: &Board) -> Vec<Spot> {
    fn titles(area: &Area) -> Vec<String> {
        area.children
            .iter()
            .filter_map(|n| match n {
                Node::Card(c) => Some(card_label(c)),
                Node::Area(_) => None,
            })
            .filter(|t| !t.is_empty())
            .take(12)
            .collect()
    }
    fn go(area: &Area, path: &str, out: &mut Vec<Spot>) {
        for child in &area.children {
            if let Node::Area(a) = child {
                let name = a.heading.as_ref().map(|h| h.text.trim()).unwrap_or("");
                let p = if path.is_empty() {
                    name.to_string()
                } else {
                    format!("{path} > {name}")
                };
                out.push(Spot {
                    id: a.id.clone(),
                    path: p.clone(),
                    cards: titles(a),
                });
                go(a, &p, out);
            }
        }
    }
    let mut out = Vec::new();
    let top = titles(&board.root);
    if !top.is_empty() {
        out.push(Spot {
            id: board.root.id.clone(),
            path: "Top level".into(),
            cards: top,
        });
    }
    go(&board.root, "", &mut out);
    out
}

fn card_label(c: &Card) -> String {
    match &c.heading {
        Some(h) => h.text.trim().to_string(),
        None => c
            .lines
            .first()
            .map(|l| short(&super::inline::plain_text(&l.spans), 60))
            .unwrap_or_default(),
    }
}

/// The first `max` characters of `s`, cut at a word.
fn short(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(a, _)| a);
    format!("{}…", cut.trim_end_matches([',', ';', ':', '.']))
}

/// Cards directly in an area that hold a task list: a to-do may join one of them.
pub fn checklists<'a>(board: &'a Board, area_id: &str) -> Vec<&'a Card> {
    let Some(area) = board.area(area_id) else {
        return Vec::new();
    };
    area.children
        .iter()
        .filter_map(|n| match n {
            Node::Card(c) if c.lines.iter().any(|l| matches!(l.kind, LineKind::Task(_))) => Some(c),
            _ => None,
        })
        .collect()
}

/// The Markdown of the titled cards directly in an area (the last `max`), to show a model what
/// notes there look like.
pub fn examples(board: &Board, src: &str, area_id: &str, max: usize) -> Vec<String> {
    let Some(area) = board.area(area_id) else {
        return Vec::new();
    };
    let cards: Vec<&Card> = area
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Card(c) if c.heading.is_some() => Some(c),
            _ => None,
        })
        .collect();
    let skip = cards.len().saturating_sub(max);
    cards[skip..]
        .iter()
        .map(|c| src[c.range.clone()].trim().to_string())
        .collect()
}

/// Turns a quick note into a titled card in Markdown (`# Title` and its body).
///
/// Text that already starts with a heading is kept as it is. Otherwise a short first line is
/// the title; a long note gets the start of its first sentence as title and keeps all of its
/// text. Each line becomes its own paragraph, and list lines stay together.
pub fn note_card(text: &str) -> String {
    let text = text.trim();
    if text.starts_with('#') {
        return text.to_string();
    }
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let first = lines.first().copied().unwrap_or("");
    let (title, rest): (String, &[&str]) = if first.chars().count() <= 70 && !is_block(first) {
        (first.trim_end_matches(['.', ':']).to_string(), &lines[1..])
    } else {
        let sentence = first
            .split_inclusive(['.', '!', '?'])
            .next()
            .unwrap_or(first);
        (
            short(sentence, 50).trim_end_matches('.').to_string(),
            &lines[..],
        )
    };
    let mut out = format!("# {title}");
    let mut prev_tight = false;
    for line in rest.iter().filter(|l| !l.is_empty()) {
        // Lists and table rows stay together; other lines are paragraphs of their own.
        let tight = is_block(line);
        out.push_str(if tight && prev_tight { "\n" } else { "\n\n" });
        out.push_str(line);
        prev_tight = tight;
    }
    out
}

fn is_block(line: &str) -> bool {
    let l = line.trim_start();
    l.starts_with("- ")
        || l.starts_with("* ")
        || l.starts_with('|')
        || l.starts_with("![")
        || l.split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// A date at the start of a title (`2026-10-20 Kickoff`), as a sortable string.
fn leading_date(title: &str) -> Option<&str> {
    let d = title.get(..10)?;
    let ok = d.bytes().enumerate().all(|(i, c)| match i {
        4 | 7 => c == b'-',
        _ => c.is_ascii_digit(),
    });
    ok.then_some(d)
}

fn heading_level(md: &str) -> Option<u8> {
    blocks::parse_blocks(md)
        .iter()
        .filter_map(|b| match b.kind {
            BlockKind::Heading(l) => Some(l),
            _ => None,
        })
        .min()
}

/// Inserts a card (Markdown starting with a heading of any level) into an area, re-levelled to
/// fit. When the area's cards are dated (`2026-10-20 …`) and so is the new one, it goes in date
/// order; otherwise it goes last.
pub fn place_card(board: &Board, src: &str, area_id: &str, md: &str) -> Option<Edited> {
    let area = board.area(area_id)?;
    let md = md.trim();
    let level = area.child_level.clamp(1, 6);
    let mut block = shift_headings(md, level as i32 - heading_level(md)? as i32);
    if tight_cards(board, src) {
        block = tighten(&block);
    }
    let title = block
        .lines()
        .next()
        .unwrap_or("")
        .trim_start_matches('#')
        .trim();

    let cards: Vec<&Card> = area
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Card(c) if c.heading.is_some() => Some(c),
            _ => None,
        })
        .collect();
    let date_of = |c: &Card| {
        c.heading
            .as_ref()
            .and_then(|h| leading_date(h.text.trim()).map(str::to_string))
    };
    let dated = cards.iter().filter(|c| date_of(c).is_some()).count();
    let before = match leading_date(title) {
        Some(day) if dated * 2 > cards.len() => cards
            .iter()
            .find(|c| date_of(c).is_some_and(|d| d.as_str() > day)),
        _ => None,
    };
    let at = match before {
        Some(c) => c.range.start,
        None if area.id.is_empty() => board.content_end(),
        None => area.section.end.min(board.content_end()),
    };
    let (source, at) = insert_block(src, at, &block);
    Some(Edited { source, at })
}

/// Whether this file writes card text right under the title, with no blank line between (the
/// usual way among its cards that have text).
fn tight_cards(board: &Board, src: &str) -> bool {
    let (mut tight, mut loose) = (0, 0);
    for node in board.walk() {
        if let super::model::NodeRef::Card(c) = node
            && let Some(h) = &c.heading
            && !c.body.is_empty()
            && h.line.start <= c.body.start
        {
            match src[h.line.start..c.body.start].matches('\n').count() {
                1 => tight += 1,
                _ => loose += 1,
            }
        }
    }
    tight > loose
}

/// Removes the blank line between a card's title and its first line of text.
fn tighten(block: &str) -> String {
    match block.split_once('\n') {
        Some((head, rest)) if head.starts_with('#') => {
            format!("{head}\n{}", rest.trim_start_matches('\n'))
        }
        _ => block.to_string(),
    }
}

/// Adds a new area at the end of the board holding one card (Markdown with a heading).
/// Returns the offset of the card's heading.
pub fn place_in_new_area(board: &Board, src: &str, title: &str, md: &str) -> Option<Edited> {
    let level = board.root.child_level.clamp(1, 5) as usize;
    let md = md.trim();
    let head = format!("{} {}", "#".repeat(level), title.trim());
    let mut card = shift_headings(md, (level + 1) as i32 - heading_level(md)? as i32);
    if tight_cards(board, src) {
        card = tighten(&card);
    }
    let (source, start) = insert_block(src, board.content_end(), &format!("{head}\n\n{card}"));
    Some(Edited {
        at: start + head.len() + 2,
        source,
    })
}

/// Adds a to-do to a checklist card, right after its last task.
pub fn add_task(board: &Board, src: &str, card_id: &str, text: &str) -> Option<Edited> {
    let card = board.card(card_id)?;
    let last = card
        .lines
        .iter()
        .rev()
        .find(|l| matches!(l.kind, LineKind::Task(_)))?;
    let line_start = blocks::line_start(src, last.orig.start);
    let indent: String = src[line_start..]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let end = blocks::line_end_incl(src, last.orig.end.saturating_sub(1).max(last.orig.start));
    let text = text
        .trim()
        .trim_start_matches('#')
        .trim()
        .trim_start_matches("- [ ]")
        .trim_start_matches("- ")
        .trim();
    let mut source = String::with_capacity(src.len() + text.len() + 16);
    source.push_str(&src[..end]);
    if !source.ends_with('\n') {
        source.push('\n');
    }
    let at = source.len();
    source.push_str(&format!("{indent}- [ ] {text}\n"));
    source.push_str(&src[end..]);
    Some(Edited { source, at })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "# Notes

## Meetings

### 2026-10-01 Kickoff
Agreed on scope.

### 2026-10-15 Budget review
Approved.

## To do

### This week
- [ ] Book the room
- [x] Send the agenda

## Ideas

### Dark mode
Later.
";

    #[test]
    fn spots_list_areas_with_their_cards() {
        let s = spots(&Board::parse(SRC));
        let paths: Vec<&str> = s.iter().map(|x| x.path.as_str()).collect();
        assert_eq!(paths, ["Meetings", "To do", "Ideas"]);
        assert_eq!(
            s[0].cards,
            ["2026-10-01 Kickoff", "2026-10-15 Budget review"]
        );
        assert!(s[0].describe().contains("Budget review"));
    }

    #[test]
    fn notes_become_cards() {
        assert_eq!(note_card("Call the framer"), "# Call the framer");
        assert_eq!(
            note_card(
                "Lunch with Maria\nShe likes the blue series.\n- Send prices\n- Book a visit"
            ),
            "# Lunch with Maria\n\nShe likes the blue series.\n\n- Send prices\n- Book a visit"
        );
        let long = note_card(
            "Today I finally understood why the glazes looked muddy: too much water in every layer.",
        );
        assert!(long.starts_with("# Today I finally understood why the glazes"));
        assert!(long.contains("too much water in every layer."));
        assert_eq!(note_card("## Kept\nas is"), "## Kept\nas is");
    }

    #[test]
    fn dated_notes_go_in_date_order() {
        let b = Board::parse(SRC);
        let e = place_card(&b, SRC, "meetings", "# 2026-10-08 Design sync\nMockups.").unwrap();
        let kickoff = e.source.find("### 2026-10-01").unwrap();
        let sync = e.source.find("### 2026-10-08 Design sync").unwrap();
        let budget = e.source.find("### 2026-10-15").unwrap();
        assert!(kickoff < sync && sync < budget, "{}", e.source);
        // Only the new block was added.
        assert_eq!(
            e.source
                .replace("### 2026-10-08 Design sync\nMockups.\n\n", ""),
            SRC
        );
    }

    #[test]
    fn undated_notes_go_last_in_their_area() {
        let b = Board::parse(SRC);
        let e = place_card(&b, SRC, "ideas", "# Keyboard shortcuts\n\nFor everything.").unwrap();
        assert!(
            e.source
                .ends_with("### Dark mode\nLater.\n\n### Keyboard shortcuts\nFor everything.\n"),
            "{}",
            e.source
        );
        assert!(Board::parse(&e.source).card("keyboard-shortcuts").is_some());
    }

    #[test]
    fn todos_join_a_checklist() {
        let b = Board::parse(SRC);
        assert_eq!(checklists(&b, "to-do").len(), 1);
        let e = add_task(&b, SRC, "this-week", "Call the framer").unwrap();
        assert!(
            e.source
                .contains("- [x] Send the agenda\n- [ ] Call the framer\n\n## Ideas"),
            "{}",
            e.source
        );
    }

    #[test]
    fn new_areas_hold_the_note() {
        let b = Board::parse(SRC);
        let e = place_in_new_area(&b, SRC, "Travel", "# Train to Oslo\nFriday at 07:12.").unwrap();
        assert!(
            e.source
                .ends_with("## Travel\n\n### Train to Oslo\nFriday at 07:12.\n"),
            "{}",
            e.source
        );
        let nb = Board::parse(&e.source);
        assert!(nb.area("travel").is_some());
        assert!(nb.card("train-to-oslo").is_some());
    }

    #[test]
    fn examples_show_the_latest_cards() {
        let b = Board::parse(SRC);
        assert_eq!(
            examples(&b, SRC, "meetings", 1),
            ["### 2026-10-15 Budget review\nApproved."]
        );
    }
}
