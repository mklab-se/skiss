//! Where everything goes on the canvas (in world units).
//!
//! Areas pack their children in rows, in document order, aiming for a pleasant landscape shape.
//! Anything the user placed by hand (an `at` line in the appendix) stays where it was put.

use std::collections::HashMap;

use egui::{Pos2, Rect, Vec2, pos2, vec2};
use skiss_core::board::{Appendix, Area, Board, Card, CardKind, LineKind, Node};

pub const CARD_PAD: f32 = 16.0;
pub const AREA_PAD: f32 = 28.0;
const GAP: f32 = 26.0;
const ROOT_GAP: f32 = 72.0;
const LINKED_GAP: f32 = 76.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Area,
    Card,
}

#[derive(Clone, Debug)]
pub struct LNode {
    pub kind: Kind,
    pub rect: Rect,
    pub depth: usize,
    pub parent: Option<String>,
    /// For areas: where child positions (appendix `at`) are measured from.
    pub content_origin: Pos2,
    pub tilt: f32,
    /// Height of an area's title band.
    pub header: f32,
}

pub struct Layout {
    pub nodes: HashMap<String, LNode>,
    /// Draw order: outer things first.
    pub order: Vec<String>,
    pub bounds: Rect,
    pub title_pos: Option<Pos2>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            nodes: HashMap::new(),
            order: Vec::new(),
            bounds: Rect::NOTHING,
            title_pos: None,
        }
    }
}

impl Layout {
    pub fn get(&self, id: &str) -> Option<&LNode> {
        self.nodes.get(id)
    }
}

/// Measures the text height of a card for a given width (world units).
pub type Measure<'a> = dyn FnMut(&Card, f32) -> f32 + 'a;

pub fn card_width(card: &Card) -> f32 {
    let has_image = card
        .lines
        .iter()
        .any(|l| matches!(l.kind, LineKind::Image { .. }));
    let has_code = card
        .lines
        .iter()
        .any(|l| matches!(l.kind, LineKind::Code(_) | LineKind::Raw | LineKind::Table));
    match card.kind {
        _ if has_image => 340.0,
        _ if has_code => 360.0,
        CardKind::Section => 270.0,
        CardKind::Block => 300.0,
        CardKind::Item => 240.0,
        CardKind::Tasks => 300.0,
    }
}

struct Placed {
    id: String,
    kind: Kind,
    pos: Vec2,
    size: Vec2,
    header: f32,
    origin: Vec2,
    children: Vec<Placed>,
}

pub fn compute(
    board: &Board,
    measure: &mut Measure<'_>,
    area_title_h: f32,
    board_title_h: f32,
    tilt: f32,
) -> Layout {
    let app = &board.appendix;
    let mut linked = std::collections::HashSet::new();
    for (from, to, _) in board.connections() {
        linked.insert(from);
        linked.insert(to);
    }
    let root = place_area(&board.root, app, measure, area_title_h, true, &linked);
    let mut layout = Layout::default();
    let origin = pos2(0.0, 0.0);
    for child in &root.children {
        flatten(child, origin, None, 0, &mut layout, tilt);
    }
    let mut bounds = Rect::NOTHING;
    for n in layout.nodes.values() {
        bounds = bounds.union(n.rect);
    }
    if board.title.is_some() {
        let p = pos2(0.0, -board_title_h - 24.0);
        layout.title_pos = Some(p);
        bounds = bounds.union(Rect::from_min_size(p, vec2(400.0, board_title_h)));
    }
    if bounds == Rect::NOTHING {
        bounds = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
    }
    layout.bounds = bounds;
    layout
}

fn flatten(
    p: &Placed,
    parent_origin: Pos2,
    parent: Option<&str>,
    depth: usize,
    out: &mut Layout,
    tilt: f32,
) {
    let min = parent_origin + p.pos;
    let rect = Rect::from_min_size(min, p.size);
    let content_origin = min + p.origin;
    let t = if p.kind == Kind::Card {
        tilt * wobble_of(&p.id)
    } else {
        0.0
    };
    out.nodes.insert(
        p.id.clone(),
        LNode {
            kind: p.kind,
            rect,
            depth,
            parent: parent.map(str::to_string),
            content_origin,
            tilt: t,
            header: p.header,
        },
    );
    out.order.push(p.id.clone());
    for c in &p.children {
        flatten(c, content_origin, Some(&p.id), depth + 1, out, tilt);
    }
}

/// A stable pseudo-random number in -1..1 for an id.
pub fn wobble_of(id: &str) -> f32 {
    let mut h: u32 = 2166136261;
    for b in id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    (h % 2001) as f32 / 1000.0 - 1.0
}

fn place_card(
    card: &Card,
    app: &Appendix,
    measure: &mut Measure<'_>,
    fixed: Option<f32>,
) -> Placed {
    let mut width = card_width(card);
    if let Some((w, _)) = app.get(&card.key).and_then(|e| e.size) {
        width = (w as f32).max(160.0);
    }
    if let Some(w) = fixed {
        width = w;
    }
    let inner = width - CARD_PAD * 2.0;
    let text_h = measure(card, inner);
    let mut h = text_h + CARD_PAD * 2.0;
    let mut children = Vec::new();
    if !card.children.is_empty() {
        let mut y = CARD_PAD + text_h + 12.0;
        for c in &card.children {
            // Children always fit inside their parent.
            let w = (width - CARD_PAD * 2.0).max(140.0);
            let mut p = place_card(c, app, measure, Some(w));
            p.pos = vec2(CARD_PAD, y);
            y += p.size.y + 10.0;
            children.push(p);
        }
        h = y + CARD_PAD - 10.0;
    }
    if let Some((_, eh)) = app.get(&card.key).and_then(|e| e.size) {
        h = h.max(eh as f32);
    }
    Placed {
        id: card.id.clone(),
        kind: Kind::Card,
        pos: Vec2::ZERO,
        size: vec2(width, h),
        header: 0.0,
        origin: Vec2::ZERO,
        children,
    }
}

fn place_area(
    area: &Area,
    app: &Appendix,
    measure: &mut Measure<'_>,
    title_h: f32,
    root: bool,
    linked: &std::collections::HashSet<String>,
) -> Placed {
    let mut items: Vec<(Placed, Option<(i32, i32)>)> = Vec::new();
    for child in &area.children {
        match child {
            Node::Area(a) => {
                let p = place_area(a, app, measure, title_h, false, linked);
                items.push((p, app.get(&a.key).and_then(|e| e.at)));
            }
            Node::Card(c) => {
                let p = place_card(c, app, measure, None);
                items.push((p, app.get(&c.key).and_then(|e| e.at)));
            }
        }
    }
    // Connected cards get room between them for their arrows and labels.
    let has_links = area.children.iter().any(|c| linked.contains(c.id()));
    let gap = if root {
        ROOT_GAP
    } else if has_links {
        LINKED_GAP
    } else {
        GAP
    };
    let widest = items
        .iter()
        .filter(|(_, m)| m.is_none())
        .map(|(p, _)| p.size.x)
        .fold(0.0f32, f32::max);
    let total: f32 = items
        .iter()
        .filter(|(_, m)| m.is_none())
        .map(|(p, _)| (p.size.x + gap) * (p.size.y + gap))
        .sum();
    let aspect = if root { 2.6 } else { 3.0 };
    let target = widest.max((total * aspect).sqrt());

    // Hand-placed items stay put; the rest flow around them in document order.
    let manual_rects: Vec<Rect> = items
        .iter()
        .filter_map(|(p, m)| m.map(|(x, y)| Rect::from_min_size(pos2(x as f32, y as f32), p.size)))
        .collect();
    let blocked = |r: Rect| {
        manual_rects
            .iter()
            .find(|m| m.expand(gap * 0.5).intersects(r))
            .copied()
    };

    // Shelf packing in document order.
    let (mut x, mut y, mut row_h) = (0.0f32, 0.0f32, 0.0f32);
    let mut placed = Vec::new();
    let mut max = Vec2::ZERO;
    for (mut p, manual) in items {
        match manual {
            Some((mx, my)) => p.pos = vec2(mx as f32, my as f32),
            None => {
                let mut guard = 0;
                loop {
                    guard += 1;
                    if x > 0.0 && x + p.size.x > target + 1.0 {
                        x = 0.0;
                        y += row_h.max(1.0) + gap;
                        row_h = 0.0;
                    }
                    let cand = Rect::from_min_size(pos2(x, y), p.size);
                    match blocked(cand) {
                        Some(m) if guard < 200 => {
                            x = m.max.x + gap;
                            row_h = row_h.max(1.0);
                        }
                        _ => break,
                    }
                }
                p.pos = vec2(x, y);
                x += p.size.x + gap;
                row_h = row_h.max(p.size.y);
            }
        }
        max = max.max(p.pos + p.size);
        placed.push(p);
    }
    if root {
        return Placed {
            id: area.id.clone(),
            kind: Kind::Area,
            pos: Vec2::ZERO,
            size: max,
            header: 0.0,
            origin: Vec2::ZERO,
            children: placed,
        };
    }
    let header = if area.heading.is_some() { title_h } else { 0.0 };
    let content = max.max(vec2(220.0, 70.0));
    let origin = vec2(AREA_PAD, header + AREA_PAD * 0.55);
    let mut size = origin + content + vec2(AREA_PAD, AREA_PAD);
    if let Some((w, h)) = app.get(&area.key).and_then(|e| e.size) {
        size = size.max(vec2(w as f32, h as f32));
    }
    Placed {
        id: area.id.clone(),
        kind: Kind::Area,
        pos: Vec2::ZERO,
        size,
        header,
        origin,
        children: placed,
    }
}
