//! Where everything goes on the canvas (in world units).
//!
//! Areas pack their children in rows, in document order, aiming for a pleasant landscape shape.
//! Anything the user placed by hand (an `at` line in the appendix) stays where it was put.

use std::collections::{HashMap, HashSet};

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
    /// The board title's font size (world units): bigger on bigger boards, so the title still
    /// reads as a headline when the whole board is in view.
    pub title_size: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            nodes: HashMap::new(),
            order: Vec::new(),
            bounds: Rect::NOTHING,
            title_pos: None,
            title_size: 0.0,
        }
    }
}

impl Layout {
    pub fn get(&self, id: &str) -> Option<&LNode> {
        self.nodes.get(id)
    }
}

/// Measures the text height of a card for a given width (world units): as shown now, and at
/// rest (collapsed, not being edited). Packing uses the resting size, so opening or typing in a
/// card only nudges what it covers instead of shuffling the board.
pub type Measure<'a> = dyn FnMut(&Card, f32) -> (f32, f32) + 'a;

/// Height / width of a picture, when known.
pub type Aspect<'a> = dyn Fn(&str) -> Option<f32> + 'a;

/// The shape the whole board aims for: about the shape of a window, so it fills the screen when
/// it opens zoomed to fit.
const SCREEN_ASPECT: f32 = 1.6;
/// The shape areas aim for.
const AREA_ASPECT: f32 = 1.5;

pub fn card_width(card: &Card, aspect: &Aspect<'_>) -> f32 {
    let image = card.lines.iter().find_map(|l| match &l.kind {
        LineKind::Image { url, .. } => Some(url.as_str()),
        _ => None,
    });
    let has_image = image.is_some();
    // Square and portrait pictures (people, paintings) get a narrower card, so they do not
    // tower over everything else.
    if let Some(a) = image.and_then(aspect)
        && a > 0.8
    {
        return if a > 1.15 { 220.0 } else { 250.0 };
    }
    let has_code = card
        .lines
        .iter()
        .any(|l| matches!(l.kind, LineKind::Code(_) | LineKind::Raw | LineKind::Table));
    // Long prose reads best in a wider column, like a page.
    let words: usize = card
        .lines
        .iter()
        .filter(|l| matches!(l.kind, LineKind::Paragraph | LineKind::Quote))
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.text.split_whitespace().count())
                .sum::<usize>()
        })
        .sum();
    match card.kind {
        _ if has_image => 340.0,
        _ if words > 150 => 520.0,
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
    /// The size at rest, which decides where things go.
    rest: Vec2,
    header: f32,
    origin: Vec2,
    children: Vec<Placed>,
}

pub fn compute(
    board: &Board,
    measure: &mut Measure<'_>,
    aspect: &Aspect<'_>,
    area_title_h: f32,
    board_title_size: f32,
    tilt: f32,
) -> Layout {
    let app = &board.appendix;
    let mut linked = HashSet::new();
    for (from, to, _) in board.connections() {
        linked.insert(from);
        linked.insert(to);
    }
    let mut ctx = Ctx {
        app,
        measure,
        aspect,
        title_h: area_title_h,
        linked: &linked,
    };
    let root = place_area(&board.root, &mut ctx, true);
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
        let width = if bounds == Rect::NOTHING {
            0.0
        } else {
            bounds.width()
        };
        let size = (width * 0.026).clamp(board_title_size, board_title_size * 4.0);
        let h = size * 1.2;
        let p = pos2(0.0, -h - size * 0.45);
        layout.title_pos = Some(p);
        layout.title_size = size;
        bounds = bounds.union(Rect::from_min_size(p, vec2(size * 8.0, h)));
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

struct Ctx<'a, 'm> {
    app: &'a Appendix,
    measure: &'a mut Measure<'m>,
    aspect: &'a Aspect<'a>,
    title_h: f32,
    linked: &'a HashSet<String>,
}

fn place_card(card: &Card, ctx: &mut Ctx<'_, '_>, fixed: Option<f32>) -> Placed {
    let app = ctx.app;
    let mut width = card_width(card, ctx.aspect);
    if let Some((w, _)) = app.get(&card.key).and_then(|e| e.size) {
        width = (w as f32).max(160.0);
    }
    if let Some(w) = fixed {
        width = w;
    }
    let inner = width - CARD_PAD * 2.0;
    let (text_h, rest_h) = (ctx.measure)(card, inner);
    let mut h = text_h + CARD_PAD * 2.0;
    let mut rh = rest_h + CARD_PAD * 2.0;
    let mut children = Vec::new();
    if !card.children.is_empty() {
        let mut y = CARD_PAD + text_h + 12.0;
        let mut ry = CARD_PAD + rest_h + 12.0;
        for c in &card.children {
            // Children always fit inside their parent.
            let w = (width - CARD_PAD * 2.0).max(140.0);
            let mut p = place_card(c, ctx, Some(w));
            p.pos = vec2(CARD_PAD, y);
            y += p.size.y + 10.0;
            ry += p.rest.y + 10.0;
            children.push(p);
        }
        h = y + CARD_PAD - 10.0;
        rh = ry + CARD_PAD - 10.0;
    }
    if let Some((_, eh)) = app.get(&card.key).and_then(|e| e.size) {
        h = h.max(eh as f32);
        rh = rh.max(eh as f32);
    }
    Placed {
        id: card.id.clone(),
        kind: Kind::Card,
        pos: Vec2::ZERO,
        size: vec2(width, h),
        rest: vec2(width, rh),
        header: 0.0,
        origin: Vec2::ZERO,
        children,
    }
}

fn place_area(area: &Area, ctx: &mut Ctx<'_, '_>, root: bool) -> Placed {
    let app = ctx.app;
    let title_h = ctx.title_h;
    let linked = ctx.linked;
    let mut items: Vec<(Placed, Option<(i32, i32)>)> = Vec::new();
    for child in &area.children {
        match child {
            Node::Area(a) => {
                let p = place_area(a, ctx, false);
                items.push((p, app.get(&a.key).and_then(|e| e.at)));
            }
            Node::Card(c) => {
                let p = place_card(c, ctx, None);
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
    let sizes: Vec<(Vec2, Option<Vec2>)> = items
        .iter()
        .map(|(p, m)| (p.rest, m.map(|(x, y)| vec2(x as f32, y as f32))))
        .collect();
    let positions = pack_best(&sizes, gap, root);
    let mut placed = Vec::new();
    let mut max = Vec2::ZERO;
    let mut rest_max = Vec2::ZERO;
    for ((mut p, _), pos) in items.into_iter().zip(positions) {
        p.pos = pos;
        rest_max = rest_max.max(pos + p.rest);
        placed.push(p);
    }
    // Nothing may overlap: when something grew (a card being typed in, an area that got a new
    // card), whatever it now covers is nudged down just enough. Only the view changes; the
    // positions in the file stay as they were.
    resolve_overlaps(&mut placed, gap);
    for p in &placed {
        max = max.max(p.pos + p.size);
    }
    if root {
        return Placed {
            id: area.id.clone(),
            kind: Kind::Area,
            pos: Vec2::ZERO,
            size: max,
            rest: rest_max,
            header: 0.0,
            origin: Vec2::ZERO,
            children: placed,
        };
    }
    let header = if area.heading.is_some() { title_h } else { 0.0 };
    let origin = vec2(AREA_PAD, header + AREA_PAD * 0.55);
    let frame = |content: Vec2| {
        let mut size = origin + content.max(vec2(220.0, 70.0)) + vec2(AREA_PAD, AREA_PAD);
        if let Some((w, h)) = app.get(&area.key).and_then(|e| e.size) {
            size = size.max(vec2(w as f32, h as f32));
        }
        size
    };
    let (size, rest) = (frame(max), frame(rest_max));
    Placed {
        id: area.id.clone(),
        kind: Kind::Area,
        pos: Vec2::ZERO,
        size,
        rest,
        header,
        origin,
        children: placed,
    }
}

/// Packs items for the best shape: tries a range of widths and keeps the one that fits a
/// screen best (the board) or wastes the least room in a pleasant shape (areas).
fn pack_best(items: &[(Vec2, Option<Vec2>)], gap: f32, root: bool) -> Vec<Vec2> {
    let free: Vec<Vec2> = items
        .iter()
        .filter(|(_, m)| m.is_none())
        .map(|(s, _)| *s)
        .collect();
    let widest = free.iter().map(|s| s.x).fold(0.0f32, f32::max);
    let all: f32 = free.iter().map(|s| s.x + gap).sum::<f32>();
    let used: f32 = free.iter().map(|s| s.x * s.y).sum();
    let mut best: Option<(f32, Vec<Vec2>)> = None;
    const STEPS: usize = 24;
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32;
        // Denser sampling at the narrow end, where the interesting shapes are.
        let width = widest + (all - widest).max(0.0) * t * t;
        let pos = pack(items, width, gap);
        let mut size = Vec2::ZERO;
        for ((s, _), p) in items.iter().zip(&pos) {
            size = size.max(*p + *s);
        }
        if size.x <= 0.0 || size.y <= 0.0 {
            return pos;
        }
        let score = if root {
            (size.x / SCREEN_ASPECT).max(size.y)
        } else {
            let shape = ((size.x / size.y) / AREA_ASPECT).ln().abs();
            let waste = size.x * size.y / used.max(1.0);
            waste * (1.0 + 0.6 * shape)
        };
        if best.as_ref().is_none_or(|(b, _)| score < *b - 0.5) {
            best = Some((score, pos));
        }
    }
    best.map(|(_, p)| p).unwrap_or_default()
}

/// Skyline packing in document order: each item drops into the highest free spot (leftmost on
/// a tie) within `width`, so short cards leave no holes beneath them. Hand-placed items stay
/// where they are and everything else flows around them.
fn pack(items: &[(Vec2, Option<Vec2>)], width: f32, gap: f32) -> Vec<Vec2> {
    let mut placed: Vec<Rect> = items
        .iter()
        .filter_map(|(s, m)| m.map(|p| Rect::from_min_size(p.to_pos2(), *s)))
        .collect();
    let mut out = Vec::with_capacity(items.len());
    for (size, m) in items {
        if let Some(p) = m {
            out.push(*p);
            continue;
        }
        let mut xs: Vec<f32> = vec![0.0];
        xs.extend(placed.iter().map(|r| r.max.x + gap));
        let mut best: Option<Vec2> = None;
        for &x in &xs {
            if x > 0.0 && x + size.x > width + 1.0 {
                continue;
            }
            let mut y = 0.0f32;
            // Drop down past everything in this column, then past any hand-placed item.
            for _ in 0..64 {
                let me = Rect::from_min_size(pos2(x, y), *size);
                let hit = placed
                    .iter()
                    .filter(|r| r.expand(gap * 0.5).intersects(me.expand(gap * 0.5 - 0.5)))
                    .map(|r| r.max.y + gap)
                    .fold(None, |a: Option<f32>, b| Some(a.map_or(b, |a| a.max(b))));
                match hit {
                    Some(b) => y = b,
                    None => break,
                }
            }
            let better = match best {
                None => true,
                Some(b) => y < b.y - 1.0 || ((y - b.y).abs() <= 1.0 && x < b.x),
            };
            if better {
                best = Some(vec2(x, y));
            }
        }
        let p = best.unwrap_or(Vec2::ZERO);
        placed.push(Rect::from_min_size(p.to_pos2(), *size));
        out.push(p);
    }
    out
}

fn resolve_overlaps(items: &mut [Placed], gap: f32) {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        (items[a].pos.y, items[a].pos.x)
            .partial_cmp(&(items[b].pos.y, items[b].pos.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (k, &i) in order.iter().enumerate() {
        for _ in 0..64 {
            let me = Rect::from_min_size(pos2(items[i].pos.x, items[i].pos.y), items[i].size);
            let hit = order[..k].iter().find_map(|&j| {
                let other =
                    Rect::from_min_size(pos2(items[j].pos.x, items[j].pos.y), items[j].size);
                other
                    .expand(gap * 0.4)
                    .intersects(me)
                    .then_some(other.max.y)
            });
            match hit {
                Some(bottom) => items[i].pos.y = bottom + gap * 0.5,
                None => break,
            }
        }
    }
}
