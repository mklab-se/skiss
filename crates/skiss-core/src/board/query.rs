//! Finding things on a board: search, and everything that relates to one card.

use super::inline;
use super::model::{Board, NodeRef};

/// The searchable text of a node: its title and its body (connections included).
pub fn node_text(node: NodeRef<'_>) -> String {
    let mut out = String::new();
    if let Some(h) = node.heading() {
        out.push_str(&h.text);
        out.push('\n');
    }
    if let NodeRef::Card(c) = node {
        for l in &c.lines {
            out.push_str(&inline::plain_text(&l.spans));
            out.push('\n');
        }
    }
    for c in node.connections() {
        out.push_str(&c.label);
        out.push(' ');
        out.push_str(&c.note);
        out.push('\n');
    }
    out
}

/// Ids of the cards and areas whose text contains every word of `query` (case-insensitive),
/// best matches (title hits) first, then in document order.
pub fn search(board: &Board, query: &str) -> Vec<String> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(bool, usize, String)> = Vec::new();
    for (i, node) in board.walk().into_iter().enumerate() {
        if node.id().is_empty() {
            continue;
        }
        let mut text = node_text(node).to_lowercase();
        for c in node.connections() {
            for t in &c.targets {
                if let Some(h) = board
                    .anchors
                    .get(t)
                    .and_then(|id| board.node(id))
                    .and_then(|n| n.heading())
                {
                    text.push_str(&h.text.to_lowercase());
                    text.push('\n');
                }
            }
        }
        // Connection targets count too: searching "Erik" finds "Manages [Erik Lund]".
        if words.iter().all(|w| text.contains(w)) {
            let title = node
                .heading()
                .map(|h| h.text.to_lowercase())
                .unwrap_or_default();
            let in_title = words.iter().all(|w| title.contains(w));
            hits.push((!in_title, i, node.id().to_string()));
        }
    }
    hits.sort();
    hits.into_iter().map(|(_, _, id)| id).collect()
}

/// A connection seen from one card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The other card.
    pub other: String,
    pub label: String,
    pub note: String,
}

/// Everything about one card: its arrows both ways, and the cards that mention it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Related {
    pub outgoing: Vec<Link>,
    pub incoming: Vec<Link>,
    /// Cards that link to it in their text, or mention its title.
    pub mentions: Vec<String>,
}

impl Related {
    pub fn is_empty(&self) -> bool {
        self.outgoing.is_empty() && self.incoming.is_empty() && self.mentions.is_empty()
    }
}

pub fn related(board: &Board, id: &str) -> Related {
    let mut r = Related::default();
    let Some(node) = board.node(id) else {
        return r;
    };
    for (from, to, c) in board.connections() {
        if from == id {
            r.outgoing.push(Link {
                other: to,
                label: c.label.clone(),
                note: c.note.clone(),
            });
        } else if to == id {
            r.incoming.push(Link {
                other: from,
                label: c.label.clone(),
                note: c.note.clone(),
            });
        }
    }
    let anchor = board.anchor_of(id).map(|a| format!("#{a}"));
    let title = node
        .heading()
        .map(|h| h.text.trim().to_lowercase())
        .filter(|t| t.chars().count() >= 3);
    let connected: Vec<&String> = r
        .outgoing
        .iter()
        .chain(r.incoming.iter())
        .map(|l| &l.other)
        .collect();
    for other in board.walk() {
        let oid = other.id();
        if oid.is_empty() || oid == id || connected.iter().any(|c| *c == oid) {
            continue;
        }
        // Only text that belongs to the card itself, not to cards inside an area.
        let links_here = match other {
            NodeRef::Card(c) => c
                .lines
                .iter()
                .any(|l| l.spans.iter().any(|s| s.link.is_some() && s.link == anchor)),
            NodeRef::Area(_) => false,
        };
        let mentions_title = title.as_ref().is_some_and(|t| {
            let text = match other {
                NodeRef::Card(_) => node_text(other).to_lowercase(),
                NodeRef::Area(_) => String::new(),
            };
            contains_word(&text, t)
        });
        if links_here || mentions_title {
            r.mentions.push(oid.to_string());
        }
    }
    r
}

/// `needle` appears in `hay` as whole words.
fn contains_word(hay: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(i) = hay[from..].find(needle) {
        let at = from + i;
        let before = hay[..at].chars().next_back();
        let after = hay[at + needle.len()..].chars().next();
        if !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric) {
            return true;
        }
        from = at + needle.len();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "# P

## People

### Anna Berg
Sponsor.
- Manages [Erik Lund](#erik-lund)

### Erik Lund
Tech lead.

### Johan Holm
Partner.

## Meetings

### 2026-10-20 Kickoff
Erik Lund presented the plan. Johan joined late.

### Budget review
Notes from [Anna](#anna-berg).
";

    #[test]
    fn search_finds_titles_first_then_text() {
        let b = Board::parse(SRC);
        let hits = search(&b, "erik");
        assert_eq!(hits[0], "erik-lund");
        assert!(hits.contains(&"2026-10-20-kickoff".to_string()));
        assert!(
            hits.contains(&"anna-berg".to_string()),
            "connection targets are searchable"
        );
        assert!(search(&b, "  ").is_empty());
        assert_eq!(search(&b, "johan late"), vec!["2026-10-20-kickoff"]);
    }

    #[test]
    fn related_collects_arrows_and_mentions() {
        let b = Board::parse(SRC);
        let erik = related(&b, "erik-lund");
        assert_eq!(erik.incoming.len(), 1);
        assert_eq!(erik.incoming[0].other, "anna-berg");
        assert_eq!(erik.incoming[0].label, "Manages");
        assert_eq!(erik.mentions, vec!["2026-10-20-kickoff"]);

        let anna = related(&b, "anna-berg");
        assert_eq!(anna.outgoing[0].other, "erik-lund");
        assert_eq!(
            anna.mentions,
            vec!["budget-review"],
            "a link in text is a mention"
        );
    }

    #[test]
    fn whole_words_only() {
        assert!(contains_word("met erik lund today", "erik lund"));
        assert!(!contains_word("frederik lundgren", "erik lund"));
    }
}
