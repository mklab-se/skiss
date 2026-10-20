//! Heading anchors, the way GitHub makes them.
//!
//! Connections point at cards through ordinary `[Name](#anchor)` links, so the anchors must match
//! what GitHub (and most Markdown viewers) generate: the links keep working outside skiss.

use std::collections::HashMap;

/// The GitHub-style anchor for a heading text: lowercase, punctuation removed, spaces as hyphens.
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.trim().chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            out.extend(c.to_lowercase());
        } else if c.is_whitespace() {
            out.push('-');
        }
    }
    out
}

/// Hands out unique anchors in document order: the second "Notes" becomes `notes-1`.
#[derive(Debug, Default)]
pub struct Slugger {
    seen: HashMap<String, usize>,
}

impl Slugger {
    pub fn next(&mut self, text: &str) -> String {
        let base = slug(text);
        let count = self.seen.entry(base.clone()).or_insert(0);
        let anchor = if *count == 0 {
            base.clone()
        } else {
            format!("{base}-{count}")
        };
        *count += 1;
        // Reserve the generated anchor too, so "notes-1" written by hand cannot collide later.
        if anchor != base {
            self.seen.entry(anchor.clone()).or_insert(1);
        }
        anchor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_like_github() {
        assert_eq!(slug("Anna Berg"), "anna-berg");
        assert_eq!(
            slug("2026-10-20 Meeting with Johan"),
            "2026-10-20-meeting-with-johan"
        );
        assert_eq!(slug("What's next?"), "whats-next");
        assert_eq!(slug("Åsa Öberg"), "åsa-öberg");
        assert_eq!(slug("API  freeze"), "api--freeze");
    }

    #[test]
    fn duplicates_get_numbered() {
        let mut s = Slugger::default();
        assert_eq!(s.next("Notes"), "notes");
        assert_eq!(s.next("Notes"), "notes-1");
        assert_eq!(s.next("Notes"), "notes-2");
        assert_eq!(s.next("Other"), "other");
    }
}
