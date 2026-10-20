//! Where everything goes on the canvas (in world units).
//!
//! Areas pack their children in rows, in document order, aiming for a pleasant landscape shape.
//! Anything the user placed by hand (an `at` line in the appendix) stays where it was put.

use std::collections::{HashMap, HashSet};

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use super::content;
use skiss_core::board::{Appendix, Area, Board, Card, CardKind, LineKind, Node, tree};

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
/// rest (as saved, not being edited). Packing uses the resting size, so typing in a card only
/// nudges what it covers instead of shuffling the board.
pub type Measure<'a> = dyn FnMut(&Card, f32) -> (f32, f32) + 'a;

/// Height / width of a picture, when known.
pub type Aspect<'a> = dyn Fn(&str) -> Option<f32> + 'a;

/// The shape the whole board aims for: about the shape of a window, so it fills the screen when
/// it opens zoomed to fit.
const SCREEN_ASPECT: f32 = 1.6;
/// The shape areas aim for.
const AREA_ASPECT: f32 = 1.5;

/// How wide a card is: by what it holds. `column` is one comfortable column of text (world
/// units, at the current text size) and `gap` the space between columns.
pub fn card_width(card: &Card, aspect: &Aspect<'_>, column: f32, gap: f32) -> f32 {
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
    // Long writing gets as many columns as keep the card about as wide as it is tall: a page
    // of prose reads like a newspaper spread, not a scroll.
    // Columns break between paragraphs, so three columns need enough of them to come out even.
    let paragraphs = card
        .lines
        .iter()
        .filter(|l| matches!(l.kind, LineKind::Paragraph | LineKind::Quote))
        .count();
    let cols = match words {
        0..=150 => 0,
        _ if words <= 300 || paragraphs < 6 => 2,
        _ => 3,
    };
    if cols > 0 && !has_image {
        return column * cols as f32 + gap * (cols as f32 - 1.0) + CARD_PAD * 2.0;
    }
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
    body: f32,
    tilt: f32,
) -> Layout {
    let app = &board.appendix;
    let mut linked = HashSet::new();
    for (from, to, _) in board.connections() {
        linked.insert(from);
        linked.insert(to);
    }
    let parents = tree::hierarchy(board);
    let owners = tree::owners(board);
    let mut ctx = Ctx {
        app,
        measure,
        aspect,
        title_h: area_title_h,
        column: body * content::COLUMN_EMS,
        column_gap: body * content::COLUMN_GAP_EMS,
        linked: &linked,
        parents: &parents,
        owners: &owners,
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
    /// One comfortable column of text, and the gap between columns (world units).
    column: f32,
    column_gap: f32,
    linked: &'a HashSet<String>,
    /// Child -> parent, for cards and areas in a hierarchy (see `tree::hierarchy`).
    parents: &'a tree::Parents,
    owners: &'a HashMap<String, String>,
}

fn place_card(card: &Card, ctx: &mut Ctx<'_, '_>, fixed: Option<f32>) -> Placed {
    let app = ctx.app;
    let mut width = card_width(card, ctx.aspect, ctx.column, ctx.column_gap);
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
    // A hierarchy among the items is arranged as a tree, which then packs as one block.
    let ids: Vec<&str> = items.iter().map(|(p, _)| p.id.as_str()).collect();
    let rests: Vec<Vec2> = items.iter().map(|(p, _)| p.rest).collect();
    let manual: Vec<bool> = items.iter().map(|(_, m)| m.is_some()).collect();
    let blocks = tree_blocks(&area.id, &ids, &rests, &manual, ctx, gap);
    let mut in_block: Vec<Option<(usize, Vec2)>> = vec![None; items.len()];
    for (b, block) in blocks.iter().enumerate() {
        for &(i, at) in &block.members {
            in_block[i] = Some((b, at));
        }
    }
    let mut sizes: Vec<(Vec2, Option<Vec2>)> = Vec::new();
    let mut slot_of: Vec<usize> = Vec::new();
    for (i, (p, m)) in items.iter().enumerate() {
        match in_block[i] {
            Some((b, _)) if blocks[b].root != i => continue,
            Some((b, _)) => sizes.push((blocks[b].size, None)),
            None => sizes.push((p.rest, m.map(|(x, y)| vec2(x as f32, y as f32)))),
        }
        slot_of.push(i);
    }
    let packed = pack_best(&sizes, gap, root);
    let mut positions = vec![Vec2::ZERO; items.len()];
    for (slot, &i) in slot_of.iter().enumerate() {
        match in_block[i] {
            Some((b, _)) => {
                for &(m, at) in &blocks[b].members {
                    positions[m] = packed[slot] + at;
                }
            }
            None => positions[i] = packed[slot],
        }
    }
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

/// A hierarchy among one area's items, arranged as a tree: the root on top, its children
/// centered in a row below, and so on.
struct Block {
    root: usize,
    size: Vec2,
    /// Item index and its position inside the block.
    members: Vec<(usize, Vec2)>,
}

fn tree_blocks(
    area: &str,
    ids: &[&str],
    sizes: &[Vec2],
    manual: &[bool],
    ctx: &Ctx<'_, '_>,
    gap: f32,
) -> Vec<Block> {
    let index: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    // The item of this area that holds `id` (itself, or the area or card it sits in).
    let item_of = |id: &str| -> Option<usize> {
        let mut cur = id;
        for _ in 0..64 {
            let owner = ctx.owners.get(cur)?;
            if owner == area {
                return index.get(cur).copied();
            }
            cur = owner;
        }
        None
    };
    let mut edges: Vec<(usize, usize)> = ctx
        .parents
        .iter()
        .filter_map(|(c, p)| Some((item_of(c)?, item_of(p)?)))
        .filter(|(c, p)| c != p)
        .collect();
    edges.sort();
    let mut parent: Vec<Option<usize>> = vec![None; ids.len()];
    for (c, p) in edges {
        let mut cur = Some(p);
        let mut cycle = false;
        while let Some(x) = cur {
            if x == c {
                cycle = true;
                break;
            }
            cur = parent[x];
        }
        if parent[c].is_none() && !cycle {
            parent[c] = Some(p);
        }
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
    for (c, p) in parent.iter().enumerate() {
        if let Some(p) = p {
            children[*p].push(c);
        }
    }
    let mut out = Vec::new();
    for r in 0..ids.len() {
        if parent[r].is_some() || children[r].is_empty() {
            continue;
        }
        let mut members = Vec::new();
        let mut stack = vec![r];
        while let Some(x) = stack.pop() {
            members.push(x);
            stack.extend(&children[x]);
        }
        // Lone pairs stay in the flow, unless they are all there is; hand-placed items win.
        let n = members.len();
        if (n < 3 && n < ids.len()) || members.iter().any(|&m| manual[m]) {
            continue;
        }
        let (size, members) = arrange(r, &children, sizes, gap, LINKED_GAP.max(gap));
        out.push(Block {
            root: r,
            size,
            members,
        });
    }
    out
}

/// A laid-out subtree: its size, and each item with its position inside it.
type Subtree = (Vec2, Vec<(usize, Vec2)>);

/// Lays out the tree under `i`: returns its size and where each item goes.
fn arrange(i: usize, children: &[Vec<usize>], sizes: &[Vec2], gap_x: f32, gap_y: f32) -> Subtree {
    let me = sizes[i];
    if children[i].is_empty() {
        return (me, vec![(i, Vec2::ZERO)]);
    }
    let subs: Vec<Subtree> = children[i]
        .iter()
        .map(|&c| arrange(c, children, sizes, gap_x, gap_y))
        .collect();
    // Many children wrap into rows of at most five.
    const PER_ROW: usize = 5;
    let rows: Vec<&[Subtree]> = {
        let n = subs.len();
        let per = n.div_ceil(n.div_ceil(PER_ROW));
        subs.chunks(per).collect()
    };
    let row_w = |row: &[Subtree]| {
        row.iter().map(|(s, _)| s.x).sum::<f32>() + gap_x * (row.len() as f32 - 1.0)
    };
    let width = rows.iter().map(|r| row_w(r)).fold(me.x, f32::max);
    let mut members = vec![(i, vec2((width - me.x) / 2.0, 0.0))];
    let mut y = me.y + gap_y;
    for row in &rows {
        let mut x = (width - row_w(row)) / 2.0;
        let mut h = 0.0f32;
        for (size, sub) in row.iter() {
            for &(m, at) in sub {
                members.push((m, at + vec2(x, y)));
            }
            x += size.x + gap_x;
            h = h.max(size.y);
        }
        y += h + gap_x;
    }
    (vec2(width, y - gap_x), members)
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
