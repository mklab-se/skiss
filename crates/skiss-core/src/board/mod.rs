//! A Markdown file as a board: parsing, the layout appendix and minimal-diff edits.

pub mod appendix;
pub mod blocks;
pub mod edit;
pub mod inline;
pub mod model;
pub mod query;
pub mod slug;

pub use appendix::{Appendix, Color};
pub use inline::{Span, Style};
pub use model::{Area, Board, Card, CardKind, Connection, Heading, Line, LineKind, Node, NodeRef};
