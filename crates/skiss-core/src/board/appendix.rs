//! The layout appendix: decoration kept in one HTML comment at the end of the file.
//!
//! ```markdown
//! <!-- skiss
//! Johan Holm: at 220, 120; red
//! Core team: at 150, 100; size 290 x 128
//! Colors: red means Blocked, yellow means Waiting
//! -->
//! ```
//!
//! Every Markdown renderer hides HTML comments, so the file still reads as plain notes. One line
//! per decorated item keeps Git diffs to a single line when something is moved.

/// The paper colors a card or area can have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Gray,
}

impl Color {
    pub const ALL: [Color; 7] = [
        Color::Red,
        Color::Orange,
        Color::Yellow,
        Color::Green,
        Color::Blue,
        Color::Purple,
        Color::Gray,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Color::Red => "red",
            Color::Orange => "orange",
            Color::Yellow => "yellow",
            Color::Green => "green",
            Color::Blue => "blue",
            Color::Purple => "purple",
            Color::Gray => "gray",
        }
    }

    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim().to_lowercase();
        Color::ALL
            .into_iter()
            .find(|c| c.name() == s || (s == "grey" && *c == Color::Gray))
    }
}

/// Decoration for one item, keyed by its title.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub at: Option<(i32, i32)>,
    pub size: Option<(i32, i32)>,
    pub color: Option<Color>,
}

impl Entry {
    fn is_empty(&self) -> bool {
        self.at.is_none() && self.size.is_none() && self.color.is_none()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Appendix {
    pub entries: Vec<Entry>,
    /// What each color means, for example red means "Blocked".
    pub color_names: Vec<(Color, String)>,
}

pub const OPEN: &str = "<!-- skiss";

impl Appendix {
    /// Reads an appendix from the text of its HTML comment.
    pub fn parse(comment: &str) -> Appendix {
        let mut app = Appendix::default();
        let body = comment
            .trim()
            .strip_prefix(OPEN)
            .unwrap_or(comment)
            .trim_end()
            .strip_suffix("-->")
            .unwrap_or(comment);
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("Colors:") {
                for part in rest.split(',') {
                    if let Some((c, name)) = part.split_once(" means ")
                        && let Some(color) = Color::parse(c)
                    {
                        app.color_names.push((color, name.trim().to_string()));
                    }
                }
                continue;
            }
            if let Some(entry) = parse_entry(line) {
                app.entries.push(entry);
            }
        }
        app
    }

    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(Entry::is_empty) && self.color_names.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }

    pub fn entry_mut(&mut self, key: &str) -> &mut Entry {
        if let Some(i) = self.entries.iter().position(|e| e.key == key) {
            return &mut self.entries[i];
        }
        self.entries.push(Entry {
            key: key.to_string(),
            ..Entry::default()
        });
        self.entries.last_mut().expect("just pushed")
    }

    pub fn color_name(&self, color: Color) -> Option<&str> {
        self.color_names
            .iter()
            .find(|(c, _)| *c == color)
            .map(|(_, n)| n.as_str())
    }

    pub fn set_color_name(&mut self, color: Color, name: &str) {
        self.color_names.retain(|(c, _)| *c != color);
        let name = name.trim();
        if !name.is_empty() {
            self.color_names.push((color, name.to_string()));
            self.color_names
                .sort_by_key(|(c, _)| Color::ALL.iter().position(|x| x == c));
        }
    }

    pub fn rename(&mut self, old: &str, new: &str) {
        if old == new {
            return;
        }
        self.entries.retain(|e| e.key != new);
        if let Some(e) = self.entries.iter_mut().find(|e| e.key == old) {
            e.key = new.to_string();
        }
    }

    pub fn remove(&mut self, key: &str) {
        self.entries.retain(|e| e.key != key);
    }

    /// Writes the comment. Entries follow `order` (document order); unknown keys are dropped.
    pub fn to_comment(&self, order: &[String]) -> String {
        let mut out = String::from(OPEN);
        out.push('\n');
        for key in order {
            if let Some(e) = self.get(key)
                && !e.is_empty()
            {
                out.push_str(&format_entry(e));
                out.push('\n');
            }
        }
        if !self.color_names.is_empty() {
            let parts: Vec<String> = self
                .color_names
                .iter()
                .map(|(c, n)| format!("{} means {}", c.name(), n))
                .collect();
            out.push_str("Colors: ");
            out.push_str(&parts.join(", "));
            out.push('\n');
        }
        out.push_str("-->");
        out
    }
}

fn format_entry(e: &Entry) -> String {
    let mut parts = Vec::new();
    if let Some((x, y)) = e.at {
        parts.push(format!("at {x}, {y}"));
    }
    if let Some((w, h)) = e.size {
        parts.push(format!("size {w} x {h}"));
    }
    if let Some(c) = e.color {
        parts.push(c.name().to_string());
    }
    format!("{}: {}", e.key, parts.join("; "))
}

fn parse_entry(line: &str) -> Option<Entry> {
    // Keys may contain ": " themselves, so try each split point from the right.
    let mut search = line.len();
    while let Some(i) = line[..search].rfind(": ") {
        let (key, rest) = (&line[..i], &line[i + 2..]);
        if let Some(entry) = parse_attrs(key.trim(), rest) {
            return Some(entry);
        }
        search = i;
    }
    None
}

fn parse_attrs(key: &str, rest: &str) -> Option<Entry> {
    if key.is_empty() {
        return None;
    }
    let mut e = Entry {
        key: key.to_string(),
        ..Entry::default()
    };
    for part in rest.split(';') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if let Some(xy) = p.strip_prefix("at ") {
            let (x, y) = xy.split_once(',')?;
            e.at = Some((num(x)?, num(y)?));
        } else if let Some(wh) = p.strip_prefix("size ") {
            let (w, h) = wh.split_once('x')?;
            e.size = Some((num(w)?, num(h)?));
        } else {
            e.color = Some(Color::parse(p)?);
        }
    }
    Some(e)
}

fn num(s: &str) -> Option<i32> {
    let s = s.trim();
    s.parse::<i32>()
        .ok()
        .or_else(|| s.parse::<f32>().ok().map(|f| f.round() as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "<!-- skiss\nJohan Holm: at 220, 120; red\nCore team: at 150, -100; size 290 x 128\nMeeting: kickoff: blue\nColors: red means Blocked, yellow means Waiting\n-->";

    #[test]
    fn parses_entries_and_color_names() {
        let a = Appendix::parse(SAMPLE);
        assert_eq!(a.entries.len(), 3);
        assert_eq!(a.get("Johan Holm").unwrap().at, Some((220, 120)));
        assert_eq!(a.get("Johan Holm").unwrap().color, Some(Color::Red));
        assert_eq!(a.get("Core team").unwrap().size, Some((290, 128)));
        assert_eq!(a.get("Core team").unwrap().at, Some((150, -100)));
        assert_eq!(a.get("Meeting: kickoff").unwrap().color, Some(Color::Blue));
        assert_eq!(a.color_name(Color::Red), Some("Blocked"));
        assert_eq!(a.color_name(Color::Yellow), Some("Waiting"));
    }

    #[test]
    fn writes_back_identically_in_document_order() {
        let a = Appendix::parse(SAMPLE);
        let order = vec![
            "Johan Holm".to_string(),
            "Core team".to_string(),
            "Meeting: kickoff".to_string(),
        ];
        assert_eq!(a.to_comment(&order), SAMPLE);
    }

    #[test]
    fn drops_keys_that_no_longer_exist() {
        let a = Appendix::parse(SAMPLE);
        let out = a.to_comment(&["Core team".to_string()]);
        assert!(!out.contains("Johan"));
        assert!(out.contains("Core team: at 150, -100; size 290 x 128"));
    }

    #[test]
    fn rename_moves_the_entry() {
        let mut a = Appendix::parse(SAMPLE);
        a.rename("Johan Holm", "Johan H");
        assert!(a.get("Johan Holm").is_none());
        assert_eq!(a.get("Johan H").unwrap().color, Some(Color::Red));
    }
}
