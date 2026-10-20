//! Hierarchies hidden in a board's connections: who reports to whom, what is part of what.
//!
//! Nothing is configured. A kind of connection (the same sentence, such as "Reports to") forms
//! a hierarchy when there are no cycles, it is big enough to be worth showing as one (at least
//! three connections, with three cards in a chain or under one parent), and one of these holds:
//!
//! - It is a chain: every card has at most one of them going out and one coming in ("Followed
//!   by", "Then"). It is a timeline: the source goes first, and the layout puts it in a row
//!   from left to right.
//! - Every card has at most one going out: the target is the parent ("Reports to", "Part of").
//! - Every card has at most one coming in: the source is the parent ("Manages", "Leads").
//!
//! Lone pairs stay ordinary arrows.

use std::collections::{BTreeMap, HashMap};

use super::model::{Area, Board, Card, Node};

/// Child id -> parent id, for every card or area that sits in a hierarchy.
pub type Parents = HashMap<String, String>;

pub fn hierarchy(board: &Board) -> Parents {
    let mut kinds: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (from, to, c) in board.connections() {
        if from == to {
            continue;
        }
        let edges = kinds.entry(c.label.trim().to_lowercase()).or_default();
        if !edges.contains(&(from.clone(), to.clone())) {
            edges.push((from, to));
        }
    }
    // Biggest hierarchy first; a card keeps the first parent it gets.
    let mut kinds: Vec<Vec<(String, String)>> = kinds.into_values().collect();
    kinds.sort_by_key(|e| std::cmp::Reverse(e.len()));

    let mut parents = Parents::new();
    for edges in kinds {
        let Some(pairs) = as_tree(&edges) else {
            continue;
        };
        for (child, parent) in pairs {
            if !parents.contains_key(&child) && !reaches(&parents, &parent, &child) {
                parents.insert(child, parent);
            }
        }
    }
    parents
}

/// The (child, parent) pairs, if these connections form a hierarchy worth laying out.
fn as_tree(edges: &[(String, String)]) -> Option<Vec<(String, String)>> {
    if edges.len() < 3 {
        return None;
    }
    let mut out_deg: HashMap<&str, usize> = HashMap::new();
    let mut in_deg: HashMap<&str, usize> = HashMap::new();
    for (f, t) in edges {
        *out_deg.entry(f).or_default() += 1;
        *in_deg.entry(t).or_default() += 1;
    }
    let one_out = out_deg.values().all(|&d| d <= 1);
    let one_in = in_deg.values().all(|&d| d <= 1);
    let pairs: Vec<(String, String)> = if one_out && !one_in {
        edges.to_vec()
    } else if one_in {
        edges.iter().map(|(f, t)| (t.clone(), f.clone())).collect()
    } else {
        return None;
    };
    let parent: Parents = pairs.iter().cloned().collect();
    if parent.keys().any(|c| reaches(&parent, &parent[c], c)) {
        return None;
    }
    // Some part of it must be more than a pair.
    let mut size: HashMap<&str, usize> = HashMap::new();
    for (_, p) in &pairs {
        *size.entry(top(&parent, p)).or_insert(1) += 1;
    }
    size.values().any(|&n| n >= 3).then_some(pairs)
}

/// Whether following parents up from `from` arrives at `target`.
fn reaches(parents: &Parents, from: &str, target: &str) -> bool {
    let mut cur = from;
    for _ in 0..=parents.len() {
        if cur == target {
            return true;
        }
        match parents.get(cur) {
            Some(p) => cur = p,
            None => return false,
        }
    }
    true
}

fn top<'a>(parents: &'a Parents, from: &'a str) -> &'a str {
    let mut cur = from;
    for _ in 0..=parents.len() {
        match parents.get(cur) {
            Some(p) => cur = p,
            None => break,
        }
    }
    cur
}

/// Node id -> the id of what contains it (an area, or the card a nested card sits in).
pub fn owners(board: &Board) -> HashMap<String, String> {
    fn card(c: &Card, out: &mut HashMap<String, String>) {
        for n in &c.children {
            out.insert(n.id.clone(), c.id.clone());
            card(n, out);
        }
    }
    fn area(a: &Area, out: &mut HashMap<String, String>) {
        for child in &a.children {
            match child {
                Node::Area(x) => {
                    out.insert(x.id.clone(), a.id.clone());
                    area(x, out);
                }
                Node::Card(c) => {
                    out.insert(c.id.clone(), a.id.clone());
                    card(c, out);
                }
            }
        }
    }
    let mut out = HashMap::new();
    area(&board.root, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORG: &str = "# Org

## Leadership

### Amara

### Lukas
- Reports to [Amara](#amara)

## Engineering

### Mei
- Reports to [Lukas](#lukas)

### Rafael
- Reports to [Mei](#mei)

### Priya
- Reports to [Mei](#mei)
- Works with [Rafael](#rafael)
";

    #[test]
    fn reports_to_is_a_hierarchy() {
        let p = hierarchy(&Board::parse(ORG));
        assert_eq!(p.get("lukas").map(String::as_str), Some("amara"));
        assert_eq!(p.get("mei").map(String::as_str), Some("lukas"));
        assert_eq!(p.get("priya").map(String::as_str), Some("mei"));
        assert!(!p.contains_key("amara"));
        assert_eq!(p.len(), 4, "a lone 'Works with' is not part of it");
    }

    #[test]
    fn manages_points_down() {
        let src = "# Team\n\n## A\n- Manages [B](#b) and [C](#c)\n\n## B\n- Manages [D](#d)\n\n## C\n\n## D\n";
        let p = hierarchy(&Board::parse(src));
        assert_eq!(p.get("b").map(String::as_str), Some("a"));
        assert_eq!(p.get("c").map(String::as_str), Some("a"));
        assert_eq!(p.get("d").map(String::as_str), Some("b"));
    }

    #[test]
    fn chains_read_along_their_arrows() {
        let src = "# Flights\n\n## A\n- Followed by [B](#b)\n\n## B\n- Followed by [C](#c)\n\n## C\n- Followed by [D](#d)\n\n## D\n";
        let p = hierarchy(&Board::parse(src));
        assert_eq!(
            p.get("b").map(String::as_str),
            Some("a"),
            "the first goes on top"
        );
        assert_eq!(p.get("d").map(String::as_str), Some("c"));
        assert!(!p.contains_key("a"));
    }

    #[test]
    fn pairs_and_tangles_are_not() {
        let pairs = "# Ideas\n\n## A\n- Became [X](#x)\n\n## B\n- Became [Y](#y)\n\n## C\n- Became [Z](#z)\n\n## X\n\n## Y\n\n## Z\n";
        assert!(hierarchy(&Board::parse(pairs)).is_empty());
        let tangle = "# Likes\n\n## A\n- Likes [B](#b) and [C](#c)\n\n## B\n- Likes [C](#c) and [A](#a)\n\n## C\n- Likes [A](#a)\n";
        assert!(hierarchy(&Board::parse(tangle)).is_empty());
    }

    #[test]
    fn owners_know_the_nesting() {
        let o = owners(&Board::parse(ORG));
        assert_eq!(o.get("mei").map(String::as_str), Some("engineering"));
        let root = Board::parse(ORG).root.id.clone();
        assert_eq!(o.get("engineering"), Some(&root));
    }
}
