//! The board on screen: painting, pointer and keyboard, editing, and talking to the document.

use std::collections::HashMap;

use egui::{
    Align2, Color32, CursorIcon, Event, FontFamily, FontId, Key, Modifiers, PointerButton, Pos2,
    Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use skiss_core::board::edit::{self, Edited};
use skiss_core::board::query;
use skiss_core::board::{Board, Card, CardKind, Color, Line, LineKind, NodeRef, Span};

use super::ai;
use super::content::{self, CardText, Marker, RowKind, TextStyle};
use super::document::Document;
use super::draw;
use super::editor::{Action, Editor, Pos};
use super::images::{self, Images};
use super::layout::{self, CARD_PAD, Kind, Layout};
use super::motion::{Camera, Motion};
use super::theme::Theme;

#[derive(Clone, Debug)]
enum Drag {
    None,
    Pan,
    /// Pressed on a node; becomes a move once the pointer travels.
    Press {
        id: String,
        at: Pos2,
    },
    Move {
        ids: Vec<String>,
        offsets: Vec<(String, Vec2)>,
    },
    Lasso {
        start: Pos2,
    },
    Connect {
        from: String,
        to: Option<String>,
    },
    SelectText,
    /// Dragging the corner grip of a card or area.
    Resize {
        id: String,
        min: Pos2,
    },
}

#[derive(Clone, Debug)]
enum Hot {
    Resize {
        id: String,
    },
    Checkbox {
        card: String,
        at: Option<usize>,
        line: usize,
    },
    Link {
        url: String,
    },
    Handle {
        id: String,
    },
    Label {
        from: String,
        to: String,
    },
}

struct Prompt {
    kind: PromptKind,
    text: String,
    anchor: Pos2,
    focus: bool,
}

#[derive(Clone)]
enum PromptKind {
    Label {
        from: String,
        to: String,
    },
    Relabel {
        from: String,
        to: String,
    },
    BoardTitle,
    NewArea {
        parent: String,
    },
    TranslateTo {
        card: String,
    },
    Picture {
        card: String,
        replace: Option<std::ops::Range<usize>>,
    },
    AreaTitle {
        id: String,
        start: usize,
    },
    ColorName {
        color: Color,
    },
}

pub struct BoardView {
    pub doc: Document,
    layout: Layout,
    layout_key: (u64, f32, u64),
    motion: Motion,
    camera: Camera,
    camera_ready: bool,
    /// Frame the board once more after the first paint, arrows included.
    refit: bool,
    /// Where the arrows went in the last paint (world units).
    ink: Rect,
    selection: Vec<String>,
    hover: Option<String>,
    drag: Drag,
    editor: Option<Editor>,
    prompt: Option<Prompt>,
    pub text_scale: f32,
    images: Images,
    hots: Vec<(Rect, Hot)>,
    /// Editing text layout from the last paint (for clicks and arrows).
    edit_text: Option<(Rect, CardText, f32)>,
    toast: Option<(String, f64)>,
    help: bool,
    tilts: HashMap<String, f32>,
    last_time: Option<f64>,
    blink_start: f64,
    theme_rev: u64,
    last_screen: Rect,
    /// A selected connection (by its label), for deleting it.
    selected_conn: Option<(String, String)>,
    /// Bring the card being edited into view once its layout is known.
    reveal_pending: bool,
    /// Place the caret here once the card being edited has been laid out.
    pending_caret: Option<Pos2>,
    /// Until when scrolling belongs to a mouse-wheel zoom.
    wheel_until: f64,
    /// When Esc was last pressed with nothing to cancel (Esc twice quits).
    esc_at: f64,
    /// Connection curves on screen (for clicking a line), with their label anchor.
    conn_paths: Vec<(String, String, Vec<Pos2>, Pos2)>,
    /// The connection under the pointer.
    hover_conn: Option<(String, String)>,
    /// The board title on screen (double-click to rename).
    title_rect: Option<Rect>,
    /// Dock buttons on screen (for scripted tests).
    dock_buttons: Vec<(String, Rect)>,
    /// The ⌘F search bar.
    search: Option<Search>,
    /// The card whose related things are shown in the side panel.
    about: Option<String>,
    /// Ids in focus this frame (search hits, related things), with their areas.
    focus: Option<std::collections::HashSet<String>>,
    /// AI requests in flight.
    ai_jobs: Vec<ai::Job>,
    /// Chosen arrow sides per connection, with the card rects they were chosen for.
    route_cache: HashMap<(String, String), Route>,
    /// The AI menu under the toolbar, and where it opens.
    ai_menu: Option<Pos2>,
}

struct Search {
    query: String,
    color: Option<Color>,
    results: Vec<String>,
    index: usize,
    focus: bool,
}

/// Arrow sides chosen for a connection: the two card rects and the layout they were chosen
/// for, and the sides.
/// A remembered arrow route: both cards' rects, the layout revision, the sides and the bend.
type Route = (Rect, Rect, u64, (u8, u8), f32);

/// A connection label to draw on top of the cards.
type Label = (Rect, std::sync::Arc<egui::Galley>, Color32);

const LIST_EPS: f32 = 4.0;

impl BoardView {
    pub fn new(doc: Document) -> Self {
        BoardView {
            doc,
            layout: Layout::default(),
            layout_key: (u64::MAX, 0.0, 0),
            motion: Motion::new(),
            camera: Camera::new(Pos2::ZERO, 1.0),
            camera_ready: false,
            refit: false,
            ink: Rect::NOTHING,
            selection: Vec::new(),
            hover: None,
            drag: Drag::None,
            editor: None,
            prompt: None,
            text_scale: 1.0,
            images: Images::default(),
            hots: Vec::new(),
            edit_text: None,
            toast: None,
            help: false,
            tilts: HashMap::new(),
            last_time: None,
            blink_start: 0.0,
            theme_rev: 0,
            last_screen: Rect::NOTHING,
            selected_conn: None,
            reveal_pending: false,
            pending_caret: None,
            wheel_until: 0.0,
            esc_at: -10.0,
            conn_paths: Vec::new(),
            hover_conn: None,
            title_rect: None,
            dock_buttons: Vec::new(),
            search: None,
            about: None,
            focus: None,
            ai_jobs: Vec::new(),
            route_cache: HashMap::new(),
            ai_menu: None,
        }
    }

    /// Where a node is on screen (points), as of the last frame.
    pub fn screen_rect(&self, id: &str) -> Option<Rect> {
        if id == "title" {
            return self.title_rect;
        }
        if let Some(rest) = id.strip_prefix("conn:") {
            let (f, t) = rest.split_once('>')?;
            return self
                .conn_paths
                .iter()
                .find(|(a, b, _, _)| a == f && b == t)
                .map(|(_, _, _, m)| Rect::from_center_size(*m, Vec2::splat(2.0)));
        }
        if let Some(name) = id.strip_prefix("dock:") {
            return self
                .dock_buttons
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, r)| *r);
        }
        let id = if id == "root" { "" } else { id };
        self.motion
            .rect(id)
            .map(|r| self.camera.rect_to_screen(self.last_screen, r))
    }

    pub fn theme_changed(&mut self) {
        self.theme_rev += 1;
    }

    fn toast(&mut self, ui: &Ui, msg: impl Into<String>) {
        self.toast = Some((msg.into(), ui.input(|i| i.time)));
    }

    // =========================================================================================
    // The frame
    // =========================================================================================

    pub fn ui(&mut self, ui: &mut Ui, theme: &Theme) -> Option<UiRequest> {
        let ctx = ui.ctx().clone();
        let now = ui.input(|i| i.time);
        let dt = self.last_time.map_or(1.0 / 60.0, |t| (now - t) as f32);
        self.last_time = Some(now);
        let screen = ui.max_rect();
        self.last_screen = screen;
        let mut request = None;

        // Outside changes to the file.
        if self.doc.poll_disk() {
            self.images.forget_all();
            if let Some(ed) = &self.editor
                && edit::node_at(&self.doc.board, ed.card_start).is_none()
            {
                self.editor = None;
            }
            self.toast(ui, "Updated from disk");
        }

        // Keyboard (unless a prompt has focus).
        // Only a text field takes the keyboard; a clicked button must not swallow shortcuts.
        let field_focus = self.prompt.is_some() || ui.ctx().text_edit_focused();
        if !field_focus {
            request = self.keyboard(ui, screen, theme);
        }

        // Layout (only when the text, theme or text size changed).
        self.relayout(&ctx, theme);

        // Camera: frame the board on first show.
        if !self.camera_ready {
            let b = self.layout.bounds;
            self.camera.jump_to(screen, b, 80.0, 1.15);
            // Start with the whole board in view; zoom in from there.
            let target = b;
            self.camera.jump_to(screen, target, 70.0, 1.15);
            let (z, c) = (self.camera.zoom, self.camera.center);
            // A gentle glide in as the cards pop up.
            self.camera = Camera::new(c, z * 0.92);
            self.camera.fly_to(screen, target, 70.0, 1.15);
            self.camera_ready = true;
            self.refit = true;
        }

        // Pointer input that moves the camera.
        let response = ui.interact(
            screen,
            ui.scope_id().with("canvas"),
            Sense::click_and_drag(),
        );
        self.scroll_and_zoom(ui, screen, &response);

        // Animate towards the layout.
        let overrides = self.drag_overrides(ui, screen);
        let targets: Vec<(String, Rect)> = self
            .layout
            .order
            .iter()
            .filter_map(|id| self.layout.get(id).map(|n| (id.clone(), n.rect)))
            .collect();
        let mut moving = self.motion.step(&targets, dt, &overrides);
        if self.reveal_pending {
            self.reveal_pending = false;
            if let Some(id) = self.editor.as_ref().map(|e| e.card_id.clone())
                && let Some(n) = self.layout.get(&id)
            {
                // The layout target, not the animated rect: that is where it is going.
                self.camera.reveal(screen, n.rect, 70.0);
            }
        }
        moving |= self.camera.step(dt);
        moving |= self.step_tilts(dt);

        self.compute_focus();

        // Paint, collecting clickable spots.
        let painter = ui.painter_at(screen);
        self.paint(&ctx, &painter, screen, theme, now);
        // An arrow that arcs outside the cards still belongs in the opening view.
        if std::mem::take(&mut self.refit) && self.ink != Rect::NOTHING {
            let all = self.layout.bounds.union(self.ink);
            if all != self.layout.bounds {
                self.camera.fly_to(screen, all, 70.0, 1.15);
            }
        }

        if let Some(at) = self.pending_caret.take()
            && let Some(pos) = self.editor_pos_at(at)
            && let Some(ed) = &mut self.editor
        {
            ed.set_cursor(pos, false);
        }

        // Pointer input on things.
        self.pointer(ui, screen, &response, theme);

        // Overlays.
        self.toolbar(ui, screen, theme);
        self.ai_menu_ui(ui, theme);
        self.poll_ai(ui);
        self.search_ui(ui, screen, theme);
        self.about_ui(ui, screen, theme);
        if let Some(r) = self.dock(ui, screen, theme) {
            request = Some(r);
        }
        self.prompt_ui(ui, theme);
        self.status(ui, screen, theme, now);
        if self.help {
            self.help_overlay(ui, screen, theme);
        }

        // Autosave and repaint scheduling: nothing moves, nothing repaints.
        if let Some(wait) = self.doc.autosave(false) {
            ctx.request_repaint_after(wait);
        }
        if moving || !matches!(self.drag, Drag::None) || !self.ai_jobs.is_empty() {
            ctx.request_repaint();
        }
        if self.editor.is_some() {
            let phase = ((now - self.blink_start) % 1.06) as f32;
            let next = if phase < 0.53 {
                0.53 - phase
            } else {
                1.06 - phase
            };
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(next.max(0.01)));
        }
        if let Some((_, t)) = &self.toast {
            if now - t > 2.2 {
                self.toast = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        request
    }

    pub fn save_now(&mut self) {
        self.commit_editor(false);
        self.doc.autosave(true);
    }

    // =========================================================================================
    // Layout
    // =========================================================================================

    fn relayout(&mut self, ctx: &egui::Context, theme: &Theme) {
        // The card being edited is measured from the editor (blank lines included), not the file.
        let editing: Option<(String, Option<Vec<Span>>, Vec<Line>)> =
            self.editor.as_ref().map(|ed| {
                let title = ed.lines.first().filter(|l| l.title).map(|l| l.spans());
                let mut lines = ed.body_lines();
                // Show empty lines while typing (they are not written to the file yet).
                for l in &mut lines {
                    if l.spans.is_empty() || l.spans.iter().all(|s| s.text.is_empty()) {
                        l.spans = vec![Span::plain(" ")];
                    }
                }
                (ed.card_id.clone(), title, lines)
            });
        let edit_hash = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            if let Some((id, t, lines)) = &editing {
                id.hash(&mut h);
                format!("{t:?}{lines:?}").hash(&mut h);
            }
            h.finish()
        };
        let key = (
            self.doc.revision ^ edit_hash.rotate_left(31),
            self.text_scale,
            self.theme_rev,
        );
        if key == self.layout_key {
            return;
        }
        self.layout_key = key;
        let dir = self.doc.dir();
        let st = TextStyle {
            theme,
            scale: 1.0,
            color: theme.text,
            text_scale: self.text_scale,
        };
        // Load images first: textures cannot be created while the fonts are locked.
        for node in self.doc.board.walk() {
            if let NodeRef::Card(c) = node {
                for l in &c.lines {
                    match &l.kind {
                        LineKind::Image { url, .. }
                        | LineKind::Html {
                            image: Some(url), ..
                        } => {
                            self.images.aspect(ctx, &dir, url);
                        }
                        _ => {}
                    }
                }
            }
        }
        let images = &self.images;
        let board = &self.doc.board;
        let area_h = theme.area_size * 1.7 * self.text_scale;
        let layout = ctx.fonts_mut(|fonts| {
            let mut measure = |card: &Card, width: f32| -> (f32, f32) {
                let mut aspect = |url: &str| images.peek_aspect(&dir, url);
                let saved = card.heading.as_ref().map(|h| title_spans(&h.spans));
                let mut height = |title: Option<&[Span]>, lines: &[Line]| {
                    let t = content::layout(fonts, &st, title, lines, width, &mut aspect);
                    t.size.y.max(theme.body_size * self.text_scale)
                };
                // Every card shows all of its text, always.
                let saved_h = height(saved.as_deref(), &card.lines);
                let shown = match &editing {
                    Some((id, t, lines)) if *id == card.id => height(t.as_deref(), lines),
                    _ => saved_h,
                };
                (shown, saved_h)
            };
            let aspect = |url: &str| images.peek_aspect(&dir, url);
            layout::compute(
                board,
                &mut measure,
                &aspect,
                area_h,
                theme.board_title_size,
                theme.tilt,
            )
        });
        self.layout = layout;
    }

    fn step_tilts(&mut self, dt: f32) -> bool {
        let mut moving = false;
        let t = 1.0 - (-dt.clamp(0.0, 0.05) * 12.0).exp();
        for (id, n) in &self.layout.nodes {
            let straight = self.selection.contains(id)
                || self.editor.as_ref().is_some_and(|e| &e.card_id == id)
                || matches!(&self.drag, Drag::Move { ids, .. } if ids.contains(id));
            let target = if straight { 0.0 } else { n.tilt };
            let cur = self.tilts.entry(id.clone()).or_insert(target);
            if (*cur - target).abs() > 0.0005 {
                *cur += (target - *cur) * t;
                moving = true;
            } else {
                *cur = target;
            }
        }
        moving
    }

    // =========================================================================================
    // Painting
    // =========================================================================================

    fn paint(
        &mut self,
        ctx: &egui::Context,
        p: &egui::Painter,
        screen: Rect,
        theme: &Theme,
        now: f64,
    ) {
        self.hots.clear();
        p.rect_filled(screen, 0.0, theme.bg);
        draw::grid(p, screen, self.camera.center, self.camera.zoom, theme);
        let zoom = self.camera.zoom;
        let cam = self.camera;
        let to_screen = |r: Rect| cam.rect_to_screen(screen, r);

        // Board title.
        if let (Some(t), Some(pos)) = (&self.doc.board.title, self.layout.title_pos) {
            let size = self.layout.title_size * zoom;
            self.title_rect = None;
            if size > 4.0 {
                let g = p.layout_no_wrap(
                    t.text.clone(),
                    FontId::new(size, theme.title.clone()),
                    theme.text,
                );
                self.title_rect = Some(Rect::from_min_size(cam.to_screen(screen, pos), g.size()));
                p.text(
                    cam.to_screen(screen, pos),
                    Align2::LEFT_TOP,
                    &t.text,
                    FontId::new(size, theme.title.clone()),
                    theme.text,
                );
            }
        }

        // Areas, outer first.
        let areas: Vec<String> = self
            .layout
            .order
            .iter()
            .filter(|id| self.layout.get(id).is_some_and(|n| n.kind == Kind::Area))
            .cloned()
            .collect();
        for id in &areas {
            self.paint_area(
                p,
                screen,
                theme,
                id,
                to_screen(self.motion.rect(id).unwrap_or(Rect::NOTHING)),
            );
        }

        // Connections under the cards (their labels go on top, after the cards).
        let labels = self.paint_connections(ctx, p, screen, theme);

        // Cards.
        let cards: Vec<String> = self
            .layout
            .order
            .iter()
            .filter(|id| self.layout.get(id).is_some_and(|n| n.kind == Kind::Card))
            .cloned()
            .collect();
        let dragging: Vec<String> = match &self.drag {
            Drag::Move { ids, .. } => ids.clone(),
            _ => Vec::new(),
        };
        self.edit_text = None;
        for id in cards.iter().filter(|id| !dragging.contains(id)) {
            self.paint_card(ctx, p, screen, theme, id, now);
        }
        // Dragged cards float above everything.
        for id in &dragging {
            if self.layout.get(id).is_some_and(|n| n.kind == Kind::Card) {
                self.paint_card(ctx, p, screen, theme, id, now);
            }
        }

        for (r, g, color) in labels {
            p.rect_filled(r, r.height() / 2.0, theme.label_bg.gamma_multiply(0.94));
            if theme.dark {
                p.rect_stroke(
                    r,
                    r.height() / 2.0,
                    Stroke::new(1.0, color.gamma_multiply(0.5)),
                    StrokeKind::Inside,
                );
            }
            p.galley(r.min + vec2(7.0, 2.0), g, color);
        }

        // Area labels when zoomed far out: the map view.
        let mut drawn_labels: Vec<Rect> = Vec::new();
        if zoom < 0.45 {
            for id in &areas {
                let Some(r) = self.motion.rect(id) else {
                    continue;
                };
                let Some(area) = self.doc.board.area(id) else {
                    continue;
                };
                let Some(h) = &area.heading else { continue };
                let sr = to_screen(r);
                if sr.width() < 30.0 || !screen.intersects(sr) {
                    continue;
                }
                let fade = map_fade(zoom);
                let depth = self.layout.get(id).map_or(0, |n| n.depth);
                let base = if depth == 0 { 22.0 } else { 15.0 };
                let size = (theme.area_size * 1.6 * zoom)
                    .max(base * self.text_scale)
                    .min(sr.width() / 3.0)
                    .max(9.0);
                let font = FontId::new(size, theme.area_font.clone());
                let g = p.layout_no_wrap(h.text.clone(), font, theme.text.gamma_multiply(fade));
                // On the frame's top edge, like a folder tab; a nested name that would collide
                // with one already drawn is left out at this zoom.
                let pos = pos2(sr.min.x + 8.0, sr.min.y - g.size().y * 0.55);
                let label_rect = Rect::from_min_size(pos, g.size()).expand2(vec2(10.0, 5.0));
                if drawn_labels.iter().any(|r: &Rect| r.intersects(label_rect)) {
                    continue;
                }
                drawn_labels.push(label_rect);
                let bg = Rect::from_min_size(pos, g.size()).expand2(vec2(10.0, 5.0));
                p.rect_filled(bg, 8.0, theme.bg.gamma_multiply(0.85 * fade));
                p.galley(pos, g, theme.text.gamma_multiply(fade));
            }
        }

        // Connection being drawn.
        if let Drag::Connect { from, to } = &self.drag
            && let Some(fr) = self.motion.rect(from)
            && let Some(ptr) = ctx.pointer_latest_pos()
        {
            let a = to_screen(fr);
            let b = match to.as_ref().and_then(|t| self.motion.rect(t)) {
                Some(r) => to_screen(r),
                None => Rect::from_center_size(ptr, Vec2::splat(2.0)),
            };
            let (pts, _, dir) = draw::connection_curve(a, b);
            let stroke = Stroke::new(2.0, theme.accent);
            p.extend(Shape::dashed_line(&pts, stroke, 7.0, 4.0));
            if let Some(tip) = pts.last() {
                draw::arrowhead(p, *tip, dir, 11.0, stroke, theme.dark);
            }
            if let Some(t) = to.as_ref().and_then(|t| self.motion.rect(t)) {
                p.rect_stroke(
                    to_screen(t).expand(4.0),
                    10.0,
                    Stroke::new(2.0, theme.accent),
                    StrokeKind::Outside,
                );
            }
        }

        // Lasso.
        if let Drag::Lasso { start } = &self.drag
            && let Some(ptr) = ctx.pointer_latest_pos()
        {
            let r = Rect::from_two_pos(*start, ptr);
            p.rect_filled(r, 4.0, theme.accent_soft.gamma_multiply(0.25));
            p.rect_stroke(r, 4.0, Stroke::new(1.2, theme.accent), StrokeKind::Inside);
        }
    }

    fn paint_area(&mut self, p: &egui::Painter, screen: Rect, theme: &Theme, id: &str, sr: Rect) {
        if !screen.expand(40.0).intersects(sr) {
            return;
        }
        let zoom = self.camera.zoom;
        let Some(area) = self.doc.board.area(id) else {
            return;
        };
        let color = self.doc.board.appendix.get(&area.key).and_then(|e| e.color);
        let selected = self.selection.iter().any(|s| s == id);
        let fill = match color {
            Some(c) => theme.paper(c).gamma_multiply(0.45),
            None => theme.area_fill,
        };
        let stroke_color = match color {
            Some(c) => theme.ink(c),
            None => theme.area_stroke,
        };
        let radius = 14.0 * zoom.clamp(0.2, 2.0);
        p.rect_filled(sr, radius, fill);
        let stroke = Stroke::new(if theme.dashed { 1.6 } else { 1.0 }, stroke_color);
        if theme.dashed {
            let dash = (9.0 * zoom).clamp(2.0, 14.0);
            draw::dashed_frame(p, sr, radius, stroke, dash, dash * 0.55);
        } else {
            p.rect_stroke(sr, radius, stroke, StrokeKind::Inside);
        }
        if selected {
            if theme.glow {
                draw::glow(p, sr, radius, theme.accent, 0.8);
            }
            p.rect_stroke(
                sr.expand(2.0),
                radius + 2.0,
                Stroke::new(2.0, theme.accent),
                StrokeKind::Outside,
            );
        }
        // Title band.
        if let Some(h) = &area.heading {
            let size = theme.area_size * zoom * self.text_scale;
            // Zoomed out, the map labels take over (one name per area, never two).
            let shown_alpha = 1.0 - map_fade(zoom);
            if size >= 5.0 && shown_alpha > 0.0 {
                let pad = layout::AREA_PAD * zoom;
                let mut text = h.text.clone();
                if text.is_empty() {
                    text = "Untitled".into();
                }
                let shown = if theme.dark {
                    text.to_uppercase()
                } else {
                    text
                };
                let font = FontId::new(size, theme.area_font.clone());
                let col = if h.text.is_empty() {
                    theme.muted
                } else {
                    theme.area_title
                }
                .gamma_multiply(shown_alpha);
                let g = p.layout(shown, font, col, (sr.width() - pad * 2.0).max(10.0));
                let pos = sr.min + vec2(pad, pad * 0.55);
                let hidden = matches!(&self.prompt, Some(Prompt { kind: PromptKind::AreaTitle { id: pid, .. }, .. }) if pid == id);
                if !hidden {
                    p.galley(pos, g, col);
                }
            }
        }
        self.paint_handles(p, id, sr, theme);
    }

    fn paint_handles(&mut self, p: &egui::Painter, id: &str, sr: Rect, theme: &Theme) {
        // The corner grip for resizing the selected card or area.
        if self.editor.is_none()
            && self.selection.len() == 1
            && self.selection[0] == id
            && sr.width() > 50.0
            && matches!(self.drag, Drag::None | Drag::Resize { .. })
        {
            let c = sr.right_bottom() - vec2(5.0, 5.0);
            for k in [4.0f32, 8.0, 12.0] {
                p.line_segment(
                    [c + vec2(-k, 0.0), c + vec2(0.0, -k)],
                    Stroke::new(1.4, theme.accent.gamma_multiply(0.85)),
                );
            }
            self.hots.push((
                Rect::from_center_size(c - vec2(4.0, 4.0), Vec2::splat(20.0)),
                Hot::Resize { id: id.to_string() },
            ));
        }
        let show = self.editor.is_none()
            && matches!(self.drag, Drag::None)
            && (self.hover.as_deref() == Some(id)
                || (self.selection.len() == 1 && self.selection[0] == id))
            && self.connectable(id)
            && sr.width() > 60.0;
        if !show {
            return;
        }
        let r = 5.0;
        for c in [
            pos2(sr.center().x, sr.min.y),
            pos2(sr.max.x, sr.center().y),
            pos2(sr.center().x, sr.max.y),
            pos2(sr.min.x, sr.center().y),
        ] {
            p.circle(c, r, theme.card_fill, Stroke::new(1.6, theme.accent));
            self.hots.push((
                Rect::from_center_size(c, Vec2::splat(r * 5.5)),
                Hot::Handle { id: id.to_string() },
            ));
        }
    }

    fn connectable(&self, id: &str) -> bool {
        self.doc.board.anchor_of(id).is_some()
    }

    fn paint_connections(
        &mut self,
        ctx: &egui::Context,
        p: &egui::Painter,
        screen: Rect,
        theme: &Theme,
    ) -> Vec<Label> {
        let mut labels = Vec::new();
        let zoom = self.camera.zoom;
        let cam = self.camera;
        let conns: Vec<(String, String, String, String)> = self
            .doc
            .board
            .connections()
            .into_iter()
            .map(|(f, t, c)| (f, t, c.label.clone(), c.note.clone()))
            .collect();
        // Where each end attaches: spread several arrows along the same side.
        struct C {
            from: String,
            to: String,
            label: String,
            note: String,
            a: Rect,
            b: Rect,
            sides: (u8, u8),
            bend: f32,
            fracs: (f32, f32),
        }
        let mut cs: Vec<C> = Vec::new();
        // Cards are obstacles; arrows look for the way around them that crosses the fewest.
        let obstacles: Vec<(String, Rect)> = self
            .layout
            .order
            .iter()
            .filter(|id| self.layout.get(id).is_some_and(|n| n.kind == Kind::Card))
            .filter_map(|id| {
                self.motion
                    .rect(id)
                    .map(|r| (id.clone(), cam.rect_to_screen(screen, r).shrink(2.0)))
            })
            .collect();
        for (from, to, label, note) in conns {
            let (Some(a), Some(b)) = (self.motion.rect(&from), self.motion.rect(&to)) else {
                continue;
            };
            let a = cam.rect_to_screen(screen, a);
            let b = cam.rect_to_screen(screen, b);
            // Choosing sides is the costly part: decide in world space, and only again when
            // either card or the layout moved.
            let (wa, wb) = (
                self.motion.rect(&from).unwrap_or(Rect::NOTHING),
                self.motion.rect(&to).unwrap_or(Rect::NOTHING),
            );
            let key = (from.clone(), to.clone());
            let (sides, bend) = match self.route_cache.get(&key) {
                Some((ra, rb, rev, sides, bend))
                    if *rev == self.layout_key.0
                        && (ra.min - wa.min).length() < 0.5
                        && (rb.min - wb.min).length() < 0.5
                        && (ra.size() - wa.size()).length() < 0.5
                        && (rb.size() - wb.size()).length() < 0.5 =>
                {
                    (*sides, *bend)
                }
                _ => {
                    let frame = cam.rect_to_screen(screen, self.layout.bounds);
                    let (sides, bend) = best_route(a, b, &obstacles, frame, &from, &to);
                    self.route_cache
                        .insert(key, (wa, wb, self.layout_key.0, sides, bend));
                    (sides, bend)
                }
            };
            cs.push(C {
                from,
                to,
                label,
                note,
                a,
                b,
                sides,
                bend,
                fracs: (0.5, 0.5),
            });
        }
        // (node, side) -> [(connection index, is the start, position of the other end)]
        type Ports = HashMap<(String, u8), Vec<(usize, bool, f32)>>;
        let mut ports: Ports = HashMap::new();
        for (i, c) in cs.iter().enumerate() {
            let along = |side: u8, p: Pos2| if side < 2 { p.y } else { p.x };
            ports.entry((c.from.clone(), c.sides.0)).or_default().push((
                i,
                true,
                along(c.sides.0, c.b.center()),
            ));
            ports.entry((c.to.clone(), c.sides.1)).or_default().push((
                i,
                false,
                along(c.sides.1, c.a.center()),
            ));
        }
        for list in ports.values_mut() {
            list.sort_by(|x, y| x.2.total_cmp(&y.2));
            let n = list.len() as f32;
            for (k, (i, start, _)) in list.iter().enumerate() {
                let f = (k as f32 + 1.0) / (n + 1.0);
                if *start {
                    cs[*i].fracs.0 = f;
                } else {
                    cs[*i].fracs.1 = f;
                }
            }
        }
        let card_rects: Vec<(String, Rect)> = self
            .layout
            .order
            .iter()
            .filter(|id| self.layout.get(id).is_some_and(|n| n.kind == Kind::Card))
            .filter_map(|id| {
                self.motion
                    .rect(id)
                    .map(|r| (id.clone(), cam.rect_to_screen(screen, r)))
            })
            .collect();
        let mut placed: Vec<Rect> = Vec::new();
        self.conn_paths.clear();
        self.ink = Rect::NOTHING;
        for c in cs {
            let C {
                from,
                to,
                label,
                note,
                a,
                b,
                sides,
                bend,
                fracs,
            } = c;
            if !screen.intersects(a.union(b)) {
                continue;
            }
            let chosen = self
                .selected_conn
                .iter()
                .chain(self.hover_conn.iter())
                .any(|(f, t)| *f == from && *t == to);
            let hot = chosen
                || self.selection.iter().any(|s| s == &from || s == &to)
                || self.hover.as_deref().is_some_and(|h| h == from || h == to);
            let dim = self.dim_factor(&from).min(self.dim_factor(&to)).max(0.35);
            let color = if hot { theme.accent } else { theme.arrow }.gamma_multiply(dim);
            let width = (if chosen {
                3.0
            } else if hot {
                2.0
            } else {
                1.4
            }) * zoom.clamp(0.5, 1.6)
                * self.text_scale.sqrt();
            let stroke = Stroke::new(width, color);
            let (p0, n0) = draw::port(a, sides.0, fracs.0);
            let (p3, n3) = draw::port(b, sides.1, fracs.1);
            let (pts, mid, dir) = draw::curve_with(p0, n0, p3, n3, bend);
            for q in &pts {
                let w = cam.to_world(screen, *q);
                self.ink = self.ink.union(Rect::from_min_max(w, w));
            }
            self.conn_paths
                .push((from.clone(), to.clone(), pts.clone(), mid));
            draw::pencil_line(p, &pts, stroke, theme, &format!("{from}>{to}"), zoom);
            if let Some(tip) = pts.last() {
                draw::arrowhead(
                    p,
                    *tip,
                    dir,
                    (11.0 * zoom.clamp(0.5, 1.5)).max(6.0),
                    stroke,
                    theme.dark,
                );
            }
            let size = 12.5 * zoom.min(1.8) * self.text_scale * if theme.dark { 1.0 } else { 1.45 };
            if size >= 8.0 && !label.is_empty() {
                let mut text = lowercase_first(&label);
                if !note.is_empty() {
                    text = format!("{text} · {note}");
                }
                let family = if theme.dark {
                    FontFamily::Proportional
                } else {
                    theme.body.clone()
                };
                let g = p.layout_no_wrap(text, FontId::new(size, family), color);
                // Slide the label along the line until it sits on empty paper.
                let size_r = g.size() + vec2(14.0, 4.0);
                // The spot along the line that covers the least of the cards and other labels.
                let mut r = Rect::from_center_size(mid, size_r);
                let mut best = f32::INFINITY;
                for f in [
                    0.5, 0.42, 0.58, 0.34, 0.66, 0.26, 0.74, 0.18, 0.82, 0.12, 0.88,
                ] {
                    let i = ((pts.len().saturating_sub(1)) as f32 * f) as usize;
                    let Some(q) = pts.get(i) else { continue };
                    let cand = Rect::from_center_size(*q, size_r);
                    let overlap: f32 = card_rects
                        .iter()
                        .map(|(_, cr)| cr.intersect(cand))
                        .chain(placed.iter().map(|o| o.intersect(cand)))
                        .filter(|x| x.is_positive())
                        .map(|x| x.area())
                        .sum();
                    if overlap < best - 0.5 {
                        best = overlap;
                        r = cand;
                    }
                    if overlap <= 0.0 {
                        break;
                    }
                }
                placed.push(r);
                self.hots.push((
                    r,
                    Hot::Label {
                        from: from.clone(),
                        to: to.clone(),
                    },
                ));
                labels.push((r, g, color));
            }
        }
        let _ = ctx;
        labels
    }

    fn paint_card(
        &mut self,
        ctx: &egui::Context,
        p: &egui::Painter,
        screen: Rect,
        theme: &Theme,
        id: &str,
        now: f64,
    ) {
        let Some(anim) = self.motion.get(id).copied() else {
            return;
        };
        if anim.delay > 0.0 {
            return;
        }
        let zoom = self.camera.zoom;
        let cam = self.camera;
        let world = anim.rect();
        let mut sr = cam.rect_to_screen(screen, world);
        let s = anim.scale();
        if (s - 1.0).abs() > 0.001 {
            sr = Rect::from_center_size(sr.center(), sr.size() * s);
        }
        if !screen.expand(60.0).intersects(sr) {
            return;
        }
        let alpha = anim.alpha() * self.dim_factor(id);
        let Some(card) = self.doc.board.card(id).cloned() else {
            return;
        };
        let deco = self.doc.board.appendix.get(&card.key).cloned();
        let color = deco.and_then(|d| d.color);
        let angle = self.tilts.get(id).copied().unwrap_or(0.0);
        let selected = self.selection.iter().any(|s| s == id);
        let editing = self.editor.as_ref().is_some_and(|e| e.card_id == id);
        let hovered = self.hover.as_deref() == Some(id);
        let dragged = matches!(&self.drag, Drag::Move { ids, .. } if ids.iter().any(|x| x == id));
        let radius = theme.card_radius * zoom.clamp(0.3, 2.0);
        let fill = match color {
            Some(c) => theme.paper(c),
            None => theme.card_fill,
        };
        let ink = match color {
            Some(c) if !theme.dark => theme.ink(c).gamma_multiply(0.9),
            _ => theme.card_stroke,
        };
        let is_child = self
            .layout
            .get(id)
            .and_then(|n| n.parent.as_ref())
            .is_some_and(|p| self.doc.board.card(p).is_some());

        // Shadow: lifts when hovered or dragged.
        let lift = if dragged {
            3.0
        } else if hovered || selected {
            1.6
        } else {
            1.0
        };
        let shadow_off = vec2(1.5, 2.5) * zoom.clamp(0.3, 1.5) * lift;
        let blur = 10.0 * zoom.clamp(0.3, 1.5) * lift;
        if !is_child {
            p.add(
                egui::epaint::RectShape::filled(
                    sr.translate(shadow_off),
                    radius,
                    theme.card_shadow.gamma_multiply(alpha),
                )
                .with_blur_width(blur)
                .with_angle(angle),
            );
        }
        p.add(
            egui::epaint::RectShape::filled(sr, radius, fill.gamma_multiply(alpha))
                .with_angle(angle),
        );
        let stroke_w = if theme.dark {
            1.0
        } else {
            1.5 * zoom.clamp(0.4, 1.4)
        };
        if theme.wobble > 0.0 {
            draw::sketch_outline(
                p,
                sr,
                radius,
                angle,
                Stroke::new(stroke_w, ink.gamma_multiply(alpha)),
                id,
                theme,
                zoom,
            );
        } else {
            p.add(
                egui::epaint::RectShape::stroke(
                    sr,
                    radius,
                    Stroke::new(stroke_w, ink.gamma_multiply(alpha)),
                    StrokeKind::Inside,
                )
                .with_angle(angle),
            );
        }
        if selected || editing {
            if theme.glow {
                draw::glow(p, sr, radius, theme.accent, 1.0);
                p.add(
                    egui::epaint::RectShape::stroke(
                        sr,
                        radius,
                        Stroke::new(1.2, theme.accent),
                        StrokeKind::Outside,
                    )
                    .with_angle(angle),
                );
            } else {
                p.add(
                    egui::epaint::RectShape::stroke(
                        sr.expand(1.5),
                        radius + 1.5,
                        Stroke::new(4.0, theme.accent_soft),
                        StrokeKind::Outside,
                    )
                    .with_angle(angle),
                );
            }
        }

        // Text, with level of detail.
        let body_px = theme.body_size * zoom * self.text_scale;
        let pad = CARD_PAD * zoom;
        let inner = Rect::from_min_max(sr.min + Vec2::splat(pad), sr.max - Vec2::splat(pad));
        let center = sr.center();
        if body_px < 5.0 {
            // Too small to read: keep the title readable like a map label, suggest the rest.
            let mut top = inner.min.y;
            if let Some(h) = &card.heading
                && sr.width() > 40.0
            {
                let size = (theme.title_size * zoom * self.text_scale)
                    .max(11.0 * self.text_scale)
                    .min(sr.width() / 5.0);
                if size >= 6.0 {
                    let mut job = egui::text::LayoutJob::simple(
                        h.text.clone(),
                        FontId::new(size, theme.title.clone()),
                        theme.text.gamma_multiply(alpha),
                        (sr.width() - pad * 2.0).max(10.0),
                    );
                    job.wrap.max_rows = 2;
                    let g = p.layout_job(job);
                    let mut shape = egui::epaint::TextShape::new(
                        draw::rotate(inner.min, center, angle),
                        g.clone(),
                        theme.text,
                    );
                    shape.angle = angle;
                    p.add(shape);
                    top += g.size().y + 2.0;
                }
            }
            if inner.height() > 3.0 && inner.width() > 6.0 {
                // Lay the card out at full size and draw each text row as a soft bar, so the
                // shape of the text stays recognizable; pictures stay pictures at any zoom.
                let st = TextStyle {
                    theme,
                    scale: 1.0,
                    color: theme.text,
                    text_scale: self.text_scale,
                };
                let dir = self.doc.dir();
                let images = &self.images;
                let width = world.width() - CARD_PAD * 2.0;
                let title = card.heading.as_ref().map(|h| title_spans(&h.spans));
                let text = ctx.fonts_mut(|fonts| {
                    let mut aspect = |url: &str| images.peek_aspect(&dir, url);
                    content::layout(
                        fonts,
                        &st,
                        title.as_deref(),
                        &card.lines,
                        width,
                        &mut aspect,
                    )
                });
                let k = zoom * s;
                let clip = Rect::from_min_max(pos2(inner.min.x, top), inner.max);
                let bar = theme.muted.gamma_multiply(0.35 * alpha);
                for row in &text.rows {
                    let pos = inner.min + row.offset * k;
                    if row.kind == RowKind::Title && card.heading.is_some() && top > inner.min.y {
                        continue;
                    }
                    match &row.kind {
                        RowKind::Image(url) => {
                            let r = Rect::from_min_size(pos, row.size * k);
                            if r.min.y > inner.max.y {
                                continue;
                            }
                            let p = &p.with_clip_rect(clip.intersect(p.clip_rect()));
                            if let Some(tex) = self.images.texture(ctx, &dir, url) {
                                let mut mesh = egui::Mesh::with_texture(tex.id());
                                mesh.add_rect_with_uv(
                                    r,
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    Color32::WHITE.gamma_multiply(alpha),
                                );
                                if angle != 0.0 {
                                    mesh.rotate(egui::emath::Rot2::from_angle(angle), center);
                                }
                                p.add(Shape::mesh(mesh));
                            }
                        }
                        RowKind::Table => {
                            // A grid of short bars, with the row rules: still reads as a table.
                            for (g, off) in &row.cells {
                                for prow in &g.rows {
                                    let w = prow.rect().width() * k;
                                    let h = (prow.rect().height() * k * 0.55).max(1.0);
                                    let at = pos
                                        + vec2(
                                            off.x * k,
                                            (off.y + prow.pos.y + prow.rect().height() * 0.25) * k,
                                        );
                                    let r = Rect::from_min_size(at, vec2(w, h)).intersect(clip);
                                    if r.is_positive() && w > 1.0 {
                                        p.add(
                                            egui::epaint::RectShape::filled(r, h / 2.0, bar)
                                                .with_angle_and_pivot(angle, center),
                                        );
                                    }
                                }
                            }
                            for y in row.rules.iter().take(1) {
                                let a = pos + vec2(0.0, y * k);
                                let r = Rect::from_min_size(a, vec2(row.size.x * k, 1.0))
                                    .intersect(clip);
                                if r.is_positive() {
                                    p.add(
                                        egui::epaint::RectShape::filled(r, 0.0, bar)
                                            .with_angle_and_pivot(angle, center),
                                    );
                                }
                            }
                        }
                        _ => {
                            for prow in &row.galley.rows {
                                let w = prow.rect().width() * k;
                                let h = (prow.rect().height() * k * 0.55).max(1.0);
                                let y = pos.y + (prow.pos.y + prow.rect().height() * 0.25) * k;
                                let r =
                                    Rect::from_min_size(pos2(pos.x, y), vec2(w, h)).intersect(clip);
                                if r.is_positive() && w > 1.0 {
                                    p.add(
                                        egui::epaint::RectShape::filled(r, h / 2.0, bar)
                                            .with_angle_and_pivot(angle, center),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        } else {
            let scale = quantize(zoom);
            let st = TextStyle {
                theme,
                scale,
                color: theme.text.gamma_multiply(alpha),
                text_scale: self.text_scale,
            };
            let width_scaled = (world.width() - CARD_PAD * 2.0) * scale;
            let dir = self.doc.dir();
            let (title, lines): (Option<Vec<Span>>, Vec<Line>) = match &self.editor {
                Some(ed) if ed.card_id == id => {
                    let t = ed.lines.first().filter(|l| l.title).map(|l| l.spans());
                    (t, ed.body_lines())
                }
                _ => (
                    card.heading.as_ref().map(|h| title_spans(&h.spans)),
                    card.lines.clone(),
                ),
            };
            let images = &self.images;
            let text = ctx.fonts_mut(|fonts| {
                let mut aspect = |url: &str| images.peek_aspect(&dir, url);
                content::layout(
                    fonts,
                    &st,
                    title.as_deref(),
                    &lines,
                    width_scaled,
                    &mut aspect,
                )
            });
            // Scaled text units -> screen: factor zoom/scale (close to 1). Text wraps a little
            // differently at each zoom step; if it would run past the bottom edge, it shrinks a
            // hair instead, so every word stays inside the card.
            let mut k = zoom / scale * s;
            if !editing && text.size.y * k > inner.height() && text.size.y > 0.0 {
                k = inner.height() / text.size.y;
            }
            let origin = inner.min;
            let placeholder = editing
                && title
                    .as_ref()
                    .is_some_and(|t| t.iter().all(|s| s.text.is_empty()));
            for row in &text.rows {
                let pos = origin + row.offset * k;
                let rpos = draw::rotate(pos, center, angle);
                match &row.kind {
                    RowKind::Table => {
                        let ink = theme.muted.gamma_multiply(0.45 * alpha);
                        let w = row.size.x * k;
                        if let Some(first) = row.rules.first() {
                            let head = Rect::from_min_size(pos, vec2(w, first * k));
                            let tint = if theme.dark {
                                Color32::from_white_alpha(10)
                            } else {
                                Color32::from_black_alpha(10)
                            };
                            p.add(
                                egui::epaint::RectShape::filled(head, 3.0, tint)
                                    .with_angle_and_pivot(angle, center),
                            );
                        }
                        for y in row.rules.iter().take(row.rules.len().saturating_sub(1)) {
                            let a = draw::rotate(pos + vec2(0.0, y * k), center, angle);
                            let b = draw::rotate(pos + vec2(w, y * k), center, angle);
                            p.line_segment([a, b], Stroke::new(1.0, ink));
                        }
                        for (g, off) in &row.cells {
                            let mut sh = egui::epaint::TextShape::new(
                                draw::rotate(pos + *off * k, center, angle),
                                g.clone(),
                                theme.text,
                            );
                            sh.angle = angle;
                            sh.opacity_factor = alpha;
                            p.add(sh);
                        }
                        continue;
                    }
                    RowKind::Image(url) => {
                        let size = row.size * k;
                        let r = Rect::from_min_size(pos, size);
                        if let Some(tex) = self.images.texture(ctx, &dir, url) {
                            let mut mesh = egui::Mesh::with_texture(tex.id());
                            mesh.add_rect_with_uv(
                                r,
                                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                Color32::WHITE.gamma_multiply(alpha),
                            );
                            if angle != 0.0 {
                                mesh.rotate(egui::emath::Rot2::from_angle(angle), center);
                            }
                            p.add(Shape::mesh(mesh));
                        } else {
                            p.rect_filled(r, 4.0, theme.muted.gamma_multiply(0.2));
                            p.text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                "image not found",
                                FontId::proportional(11.0),
                                theme.muted,
                            );
                        }
                        continue;
                    }
                    RowKind::Code => {
                        let r = Rect::from_min_size(
                            origin
                                + (row.offset
                                    - vec2(
                                        theme.body_size * scale * 0.4,
                                        theme.body_size * scale * 0.3,
                                    ))
                                    * k,
                            row.size * k,
                        );
                        let bg = if theme.dark {
                            Color32::from_rgba_unmultiplied(255, 255, 255, 10)
                        } else {
                            Color32::from_rgba_unmultiplied(60, 40, 20, 14)
                        };
                        p.add(
                            egui::epaint::RectShape::filled(r, 5.0 * zoom.min(1.5), bg)
                                .with_angle_and_pivot(angle, center),
                        );
                    }
                    RowKind::Quote => {
                        let x = origin.x + row.marker_x * k + 2.0;
                        let a = draw::rotate(pos2(x, pos.y), center, angle);
                        let b =
                            draw::rotate(pos2(x, pos.y + row.galley.size().y * k), center, angle);
                        p.line_segment(
                            [a, b],
                            Stroke::new(3.0 * zoom.min(1.5), theme.muted.gamma_multiply(0.5)),
                        );
                    }
                    _ => {}
                }
                if (k - 1.0).abs() < 0.02 || true {
                    let mut shape =
                        egui::epaint::TextShape::new(rpos, row.galley.clone(), theme.text);
                    shape.angle = angle;
                    shape.opacity_factor = alpha;
                    if placeholder && row.kind == RowKind::Title {
                        let g = p.layout_no_wrap(
                            "Title".into(),
                            FontId::new(
                                theme.title_size * scale * self.text_scale,
                                theme.title.clone(),
                            ),
                            theme.muted.gamma_multiply(0.6),
                        );
                        let mut ph = egui::epaint::TextShape::new(rpos, g, theme.muted);
                        ph.angle = angle;
                        p.add(ph);
                    }
                    p.add(shape);
                }
                // Markers.
                let mh = row
                    .galley
                    .rows
                    .first()
                    .map_or(row.galley.size().y, |r| r.height())
                    * k;
                let mx = origin.x + row.marker_x * k;
                let mid_y = pos.y + mh * 0.5;
                match &row.marker {
                    Marker::Bullet => {
                        let c = draw::rotate(pos2(mx + body_px * 0.3, mid_y), center, angle);
                        p.circle_filled(
                            c,
                            (body_px * 0.11).max(1.5),
                            theme.text.gamma_multiply(alpha),
                        );
                    }
                    Marker::Number(n) => {
                        let c = draw::rotate(pos2(mx, pos.y), center, angle);
                        let mut sh = egui::epaint::TextShape::new(
                            c,
                            p.layout_no_wrap(
                                n.clone(),
                                FontId::new(body_px, theme.body.clone()),
                                theme.muted,
                            ),
                            theme.muted,
                        );
                        sh.angle = angle;
                        p.add(sh);
                    }
                    Marker::Checkbox { checked } => {
                        let side = (body_px * 0.62).max(6.0);
                        let c = draw::rotate(pos2(mx + side * 0.6, mid_y), center, angle);
                        let r = Rect::from_center_size(c, Vec2::splat(side));
                        let ink = if *checked { theme.accent } else { theme.text };
                        if *checked {
                            p.add(
                                egui::epaint::RectShape::filled(
                                    r,
                                    side * 0.2,
                                    ink.gamma_multiply(alpha),
                                )
                                .with_angle(angle),
                            );
                            let tick = [
                                r.min + vec2(side * 0.22, side * 0.52),
                                r.min + vec2(side * 0.42, side * 0.74),
                                r.min + vec2(side * 0.8, side * 0.28),
                            ];
                            p.add(Shape::line(
                                tick.to_vec(),
                                Stroke::new((side * 0.14).max(1.2), theme.card_fill),
                            ));
                        } else {
                            p.add(
                                egui::epaint::RectShape::stroke(
                                    r,
                                    side * 0.2,
                                    Stroke::new((side * 0.1).max(1.0), ink.gamma_multiply(alpha)),
                                    StrokeKind::Inside,
                                )
                                .with_angle(angle),
                            );
                        }
                        let line = row.line.unwrap_or(0);
                        let at = card.lines.get(line).and_then(|l| l.task_at);
                        self.hots.push((
                            r.expand(4.0),
                            Hot::Checkbox {
                                card: id.to_string(),
                                at,
                                line,
                            },
                        ));
                    }
                    Marker::None => {}
                }
                // Links are clickable (when not editing this card).
                if !editing {
                    self.collect_links(row, pos, k, &lines);
                }
            }
            if editing {
                self.paint_editor(p, &text, origin, k, theme, now);
                self.edit_text = Some((sr, text, k));
            }
        }
        // A card the AI is working on shimmers, with a note of what is happening.
        if let Some(job) = self.ai_jobs.iter().find(|j| j.card_id == id) {
            let t = (now * 0.9).fract() as f32;
            let x = sr.min.x - sr.width() * 0.4 + t * sr.width() * 1.8;
            let band = Rect::from_min_max(pos2(x, sr.min.y), pos2(x + sr.width() * 0.35, sr.max.y))
                .intersect(sr);
            if band.is_positive() {
                let glow = if theme.dark {
                    Color32::from_rgba_unmultiplied(255, 120, 60, 38)
                } else {
                    Color32::from_rgba_unmultiplied(255, 200, 60, 70)
                };
                let mut mesh = egui::Mesh::default();
                let i = mesh.vertices.len() as u32;
                mesh.colored_vertex(band.left_top(), Color32::TRANSPARENT);
                mesh.colored_vertex(pos2(band.center().x, band.min.y), glow);
                mesh.colored_vertex(band.right_top(), Color32::TRANSPARENT);
                mesh.colored_vertex(band.right_bottom(), Color32::TRANSPARENT);
                mesh.colored_vertex(pos2(band.center().x, band.max.y), glow);
                mesh.colored_vertex(band.left_bottom(), Color32::TRANSPARENT);
                mesh.add_triangle(i, i + 1, i + 4);
                mesh.add_triangle(i, i + 4, i + 5);
                mesh.add_triangle(i + 1, i + 2, i + 3);
                mesh.add_triangle(i + 1, i + 3, i + 4);
                p.add(Shape::mesh(mesh));
            }
            let dots = ".".repeat(1 + ((now * 2.0) as usize % 3));
            p.text(
                pos2(sr.center().x, sr.max.y + 12.0),
                Align2::CENTER_TOP,
                format!("AI · {}{dots}", job.label),
                FontId::proportional(13.0),
                theme.muted,
            );
        }
        // Handles for connecting.
        self.paint_handles(p, id, sr, theme);
    }

    fn collect_links(&mut self, row: &content::Row, pos: Pos2, k: f32, lines: &[Line]) {
        let Some(li) = row.line else { return };
        let Some(line) = lines.get(li) else { return };
        if !line.spans.iter().any(|s| s.link.is_some()) {
            return;
        }
        let mut idx = 0usize;
        for span in &line.spans {
            let n = span.text.chars().count();
            if let Some(url) = &span.link {
                let a = row.galley.pos_from_cursor(egui::text::CCursor::new(idx));
                let b = row
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(idx + n));
                if (a.min.y - b.min.y).abs() < 1.0 {
                    let r = Rect::from_min_max(
                        pos + a.min.to_vec2() * k,
                        pos + vec2(b.min.x, b.max.y) * k,
                    );
                    self.hots.push((r, Hot::Link { url: url.clone() }));
                } else {
                    // Wrapped link: make the whole rows clickable.
                    let r = Rect::from_min_max(
                        pos + vec2(0.0, a.min.y) * k,
                        pos + vec2(row.galley.size().x, b.max.y) * k,
                    );
                    self.hots.push((r, Hot::Link { url: url.clone() }));
                }
            }
            idx += n;
        }
    }

    fn paint_editor(
        &self,
        p: &egui::Painter,
        text: &CardText,
        origin: Pos2,
        k: f32,
        theme: &Theme,
        now: f64,
    ) {
        let Some(ed) = &self.editor else { return };
        let has_title = ed.has_title();
        let row_for = |line: usize| {
            text.rows.iter().find(|r| {
                let li = match r.line {
                    None => 0,
                    Some(i) => i + usize::from(has_title),
                };
                li == line
            })
        };
        // Selection.
        let sel_color = if theme.dark {
            theme.accent.gamma_multiply(0.35)
        } else {
            Color32::from_rgba_unmultiplied(255, 200, 60, 120)
        };
        for (li, a, b) in ed.selection_ranges() {
            let Some(row) = row_for(li) else { continue };
            let pos = origin + row.offset * k;
            let mut acc = 0usize;
            for prow in &row.galley.rows {
                let count = prow.char_count_including_newline().0;
                let (ra, rb) = (
                    a.max(acc),
                    b.min(acc + prow.char_count_excluding_newline().0),
                );
                if ra < rb || (ra == rb && a < b && ra == acc + count && false) {
                    let xa = prow.x_offset(egui::epaint::text::CharIndex(ra - acc));
                    let xb = prow.x_offset(egui::epaint::text::CharIndex(rb - acc));
                    let r = Rect::from_min_max(
                        pos + vec2(prow.pos.x + xa, prow.pos.y + prow.min_y()) * k,
                        pos + vec2(prow.pos.x + xb, prow.pos.y + prow.max_y()) * k,
                    );
                    p.rect_filled(r, 2.0, sel_color);
                }
                acc += count;
            }
            // An empty selected line still shows a sliver.
            if a == b && row.galley.rows.len() == 1 {
                let r = Rect::from_min_size(pos, vec2(4.0, row.galley.size().y * k));
                p.rect_filled(r, 1.0, sel_color);
            }
        }
        // Caret (blinking).
        let on = ((now - self.blink_start) % 1.06) < 0.53;
        if on && let Some(row) = row_for(ed.cursor.line) {
            let pos = origin + row.offset * k;
            let r = row
                .galley
                .pos_from_cursor(egui::text::CCursor::new(ed.cursor.col));
            let top = pos + r.min.to_vec2() * k;
            let h = r.height() * k;
            let w = (1.6 * self.camera.zoom.clamp(0.6, 1.6)).max(1.4);
            let caret = Rect::from_min_size(top, vec2(w, h));
            p.rect_filled(
                caret,
                0.0,
                if theme.dark { theme.accent } else { theme.text },
            );
        }
    }

    // =========================================================================================
    // Pointer
    // =========================================================================================

    fn scroll_and_zoom(&mut self, ui: &Ui, screen: Rect, response: &egui::Response) {
        if self
            .prompt
            .as_ref()
            .is_some_and(|_| ui.ctx().egui_wants_pointer_input() && !response.hovered())
        {
            return;
        }
        if !response.hovered() && !response.dragged() {
            return;
        }
        let (scroll, zoom, ptr, mods) = ui.input(|i| {
            (
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.pointer.hover_pos(),
                i.modifiers,
            )
        });
        let at = ptr.unwrap_or(screen.center());
        if zoom != 1.0 {
            self.camera.zoom_at(screen, at, zoom);
        }
        // A physical mouse wheel zooms (like Miro); trackpad scrolling pans.
        let now = ui.input(|i| i.time);
        let wheel: f32 = ui.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| match e {
                    Event::MouseWheel {
                        unit,
                        delta,
                        source,
                        modifiers,
                        ..
                    } if !modifiers.shift
                        && (*source == egui::MouseWheelSource::Wheel
                            || (*source == egui::MouseWheelSource::Unknown
                                && *unit == egui::MouseWheelUnit::Line)) =>
                    {
                        let lines = match unit {
                            egui::MouseWheelUnit::Point => delta.y / 40.0,
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Page => delta.y * 10.0,
                        };
                        Some(lines)
                    }
                    _ => None,
                })
                .sum()
        });
        if wheel != 0.0 {
            self.wheel_until = now + 0.35;
            self.camera.glide_zoom(screen, at, (wheel * 0.18).exp());
            return;
        }
        if now < self.wheel_until && !(mods.command || mods.ctrl) {
            // egui keeps easing the wheel's scroll for a moment: do not pan with it.
            return;
        }
        if scroll != Vec2::ZERO {
            if mods.command || mods.ctrl {
                let f = (scroll.y * 0.0035).exp();
                self.camera.zoom_at(screen, at, f);
            } else {
                self.camera.pan(scroll);
            }
        }
        // Middle or secondary drag always pans.
        if response.dragged_by(PointerButton::Middle)
            || response.dragged_by(PointerButton::Secondary)
        {
            self.camera.pan(response.drag_delta());
        }
    }

    /// The connection whose line passes close to a screen point.
    fn conn_at(&self, at: Pos2) -> Option<(String, String)> {
        self.conn_paths
            .iter()
            .map(|(f, t, pts, _)| (f, t, distance_to(pts, at)))
            .filter(|(_, _, d)| *d < 7.0)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(f, t, _)| (f.clone(), t.clone()))
    }

    /// A connectable card or area whose border is right at a screen point (a band of a few
    /// points either side), so a drag can start a connection without hitting a dot exactly.
    fn edge_of(&self, screen: Rect, at: Pos2) -> Option<String> {
        let band = 9.0;
        let mut best: Option<(String, usize)> = None;
        for id in &self.layout.order {
            let Some(r) = self.motion.rect(id) else {
                continue;
            };
            let sr = self.camera.rect_to_screen(screen, r);
            if sr.width() < 40.0 || !sr.expand(band).contains(at) || sr.shrink(band).contains(at) {
                continue;
            }
            if !self.connectable(id) {
                continue;
            }
            let n = self.layout.get(id)?;
            // Areas only from their side and bottom edges (the top is the title band).
            if n.kind == Kind::Area && at.y < sr.min.y + band {
                continue;
            }
            let depth = n.depth + if n.kind == Kind::Card { 100 } else { 0 };
            if best.as_ref().is_none_or(|(_, d)| depth > *d) {
                best = Some((id.clone(), depth));
            }
        }
        best.map(|b| b.0)
    }

    /// Hit test: the topmost node under a screen point (cards above areas, inner above outer).
    fn hit(&self, screen: Rect, at: Pos2) -> Option<String> {
        let cam = self.camera;
        let mut best: Option<(String, usize, bool)> = None;
        for id in &self.layout.order {
            let Some(r) = self.motion.rect(id) else {
                continue;
            };
            let sr = cam.rect_to_screen(screen, r);
            if !sr.contains(at) {
                continue;
            }
            let n = self.layout.get(id)?;
            let is_card = n.kind == Kind::Card;
            let better = match &best {
                None => true,
                Some((_, d, c)) => (is_card && !c) || (is_card == *c && n.depth >= *d),
            };
            if better {
                best = Some((id.clone(), n.depth, is_card));
            }
        }
        best.map(|b| b.0)
    }

    /// The deepest area under a world point, skipping `skip` and anything inside it.
    fn area_at(&self, world: Pos2, skip: &[String]) -> String {
        let mut best = (String::new(), 0usize);
        for (id, n) in &self.layout.nodes {
            if n.kind != Kind::Area || skip.contains(id) {
                continue;
            }
            if skip.iter().any(|s| self.is_inside(id, s)) {
                continue;
            }
            let r = self.motion.rect(id).unwrap_or(n.rect);
            if r.contains(world) && n.depth + 1 > best.1 {
                best = (id.clone(), n.depth + 1);
            }
        }
        best.0
    }

    fn is_inside(&self, id: &str, ancestor: &str) -> bool {
        let mut cur = self.layout.get(id).and_then(|n| n.parent.clone());
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            cur = self.layout.get(&p).and_then(|n| n.parent.clone());
        }
        false
    }

    fn hot_at(&self, at: Pos2) -> Option<Hot> {
        self.hots
            .iter()
            .rev()
            .find(|(r, _)| r.contains(at))
            .map(|(_, h)| h.clone())
    }

    fn drag_overrides(&self, ui: &Ui, screen: Rect) -> HashMap<String, Rect> {
        let mut out = HashMap::new();
        if let Drag::Resize { id, min } = &self.drag
            && let Some(ptr) = ui.input(|i| i.pointer.latest_pos())
        {
            let w = self.camera.to_world(screen, ptr);
            let max = pos2(w.x.max(min.x + 160.0), w.y.max(min.y + 60.0));
            out.insert(id.clone(), Rect::from_min_max(*min, max));
        }
        if let Drag::Move { offsets, .. } = &self.drag
            && let Some(ptr) = ui.input(|i| i.pointer.latest_pos())
        {
            let w = self.camera.to_world(screen, ptr);
            for (id, off) in offsets {
                if let Some(n) = self.layout.get(id) {
                    out.insert(id.clone(), Rect::from_min_size(w + *off, n.rect.size()));
                }
            }
        }
        out
    }

    fn pointer(&mut self, ui: &mut Ui, screen: Rect, response: &egui::Response, theme: &Theme) {
        let ptr = ui.input(|i| i.pointer.latest_pos());
        let mods = ui.input(|i| i.modifiers);
        if std::env::var("SKISS_DEBUG").is_ok() && ui.input(|i| i.pointer.any_pressed()) {
            eprintln!(
                "press at {ptr:?}: canvas hovered {} layer {:?} hot {:?}",
                response.hovered(),
                ui.ctx().layer_id_at(ptr.unwrap_or_default()),
                ptr.and_then(|p| self.hot_at(p))
            );
        }
        if response.hovered()
            && let Some(at) = ptr
        {
            self.hover = self.hit(screen, at);
            let hovered_area_or_none = self
                .hover
                .as_ref()
                .is_none_or(|h| self.layout.get(h).is_some_and(|n| n.kind == Kind::Area));
            self.hover_conn = if hovered_area_or_none && self.hot_at(at).is_none() {
                self.conn_at(at)
            } else {
                None
            };
            if self.hover_conn.is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            } else if self.editor.is_none() && self.edge_of(screen, at).is_some() {
                // On a card's border: dragging from here draws a connection.
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
            }
            match self.hot_at(at) {
                Some(Hot::Link { url }) => {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    egui::Tooltip::always_open(
                        ui.ctx().clone(),
                        ui.layer_id(),
                        egui::Id::unique("link-tip"),
                        egui::PopupAnchor::Pointer,
                    )
                    .show(|ui| {
                        ui.label(if url.starts_with('#') {
                            "Click to go there".to_string()
                        } else {
                            format!("{url}\nClick to open")
                        });
                    });
                }
                Some(Hot::Resize { .. }) => ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe),
                Some(Hot::Checkbox { .. }) | Some(Hot::Handle { .. }) | Some(Hot::Label { .. }) => {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand)
                }
                None => {
                    if self
                        .editor
                        .as_ref()
                        .is_some_and(|e| Some(&e.card_id) == self.hover.as_ref())
                    {
                        ui.ctx().set_cursor_icon(CursorIcon::Text);
                    }
                }
            }
        } else if !response.dragged() {
            self.hover = None;
        }

        // Press: decide what a drag would do.
        if response.drag_started_by(PointerButton::Primary)
            && let Some(at) = ui.input(|i| i.pointer.press_origin())
        {
            let hit = self.hit(screen, at);
            let hot = self.hot_at(at);
            if std::env::var("SKISS_DEBUG").is_ok() {
                eprintln!(
                    "drag start at {at:?}: hit {hit:?}, hot {hot:?}, hots {}",
                    self.hots.len()
                );
            }
            let edge = if self.editor.is_none() {
                self.edge_of(screen, at)
            } else {
                None
            };
            if let Some(Hot::Resize { id }) = &hot {
                let min = self.motion.rect(id).map_or(Pos2::ZERO, |r| r.min);
                self.drag = Drag::Resize {
                    id: id.clone(),
                    min,
                };
            } else if let Some(Hot::Handle { id }) = hot {
                self.drag = Drag::Connect { from: id, to: None };
            } else if let Some(id) = edge {
                self.drag = Drag::Connect { from: id, to: None };
            } else if let Some(ed) = &self.editor
                && hit.as_deref() == Some(ed.card_id.as_str())
            {
                self.drag = Drag::SelectText;
            } else if let Some(id) = hit {
                let is_area_body = self.layout.get(&id).is_some_and(|n| n.kind == Kind::Area)
                    && !self.on_area_title(screen, &id, at);
                if is_area_body {
                    self.drag = if mods.shift {
                        Drag::Lasso { start: at }
                    } else {
                        Drag::Pan
                    };
                } else {
                    self.drag = Drag::Press { id, at };
                }
            } else {
                self.drag = if mods.shift {
                    Drag::Lasso { start: at }
                } else {
                    Drag::Pan
                };
            }
        }

        match self.drag.clone() {
            Drag::Pan if response.dragged_by(PointerButton::Primary) => {
                self.camera.pan(response.drag_delta());
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }
            Drag::Press { id, at } => {
                if let Some(p) = ptr
                    && (p - at).length() > LIST_EPS
                {
                    if self.editor.is_some() {
                        self.finish_editing();
                    }
                    let ids = if self.selection.contains(&id) {
                        self.selection.clone()
                    } else {
                        self.selection = vec![id.clone()];
                        vec![id.clone()]
                    };
                    // Only move top-level picks (not children of other picked items).
                    let ids: Vec<String> = ids
                        .iter()
                        .filter(|x| !ids.iter().any(|y| y != *x && self.is_inside(x, y)))
                        .cloned()
                        .collect();
                    let grab = self.camera.to_world(screen, at);
                    let offsets = ids
                        .iter()
                        .filter_map(|x| self.motion.rect(x).map(|r| (x.clone(), r.min - grab)))
                        .collect();
                    self.drag = Drag::Move { ids, offsets };
                }
            }
            Drag::Connect { from, .. } => {
                if let Some(p) = ptr {
                    let to = self
                        .hit(screen, p)
                        .filter(|t| *t != from && self.connectable(t));
                    self.drag = Drag::Connect { from, to };
                }
            }
            Drag::SelectText => {
                if let Some(p) = ptr
                    && let Some(pos) = self.editor_pos_at(p)
                    && let Some(ed) = &mut self.editor
                {
                    ed.set_cursor(pos, true);
                    self.blink_start = ui.input(|i| i.time);
                }
            }
            _ => {}
        }

        if response.drag_stopped() {
            let drag = std::mem::replace(&mut self.drag, Drag::None);
            match drag {
                Drag::Move { ids, .. } => self.drop_moved(ui, screen, &ids, ptr),
                Drag::Resize { id, min } => {
                    if let Some(p) = ptr {
                        let w = self.camera.to_world(screen, p);
                        let size = (
                            (w.x - min.x).max(160.0).round() as i32,
                            (w.y - min.y).max(60.0).round() as i32,
                        );
                        if let Some(key) = self.doc.board.node(&id).map(|n| n.key().to_string()) {
                            let src = edit::with_appendix(&self.doc.board, &self.doc.source, |a| {
                                a.entry_mut(&key).size = Some(size);
                            });
                            self.doc.apply(src, &self.selection, None);
                        }
                    }
                }
                Drag::Lasso { start } => {
                    if let Some(end) = ptr {
                        let r = Rect::from_two_pos(start, end);
                        let picked: Vec<String> = self
                            .layout
                            .order
                            .iter()
                            .filter(|id| self.layout.get(id).is_some_and(|n| n.kind == Kind::Card))
                            .filter(|id| {
                                self.motion.rect(id).is_some_and(|w| {
                                    r.intersects(self.camera.rect_to_screen(screen, w))
                                })
                            })
                            .cloned()
                            .collect();
                        if mods.shift {
                            for id in picked {
                                if !self.selection.contains(&id) {
                                    self.selection.push(id);
                                }
                            }
                        } else {
                            self.selection = picked;
                        }
                    }
                }
                Drag::Connect { from, to: Some(to) } => {
                    let anchor = ptr.unwrap_or(screen.center());
                    self.prompt = Some(Prompt {
                        kind: PromptKind::Label { from, to },
                        text: String::new(),
                        anchor,
                        focus: true,
                    });
                }
                _ => {}
            }
        }

        // Clicks.
        if response.clicked_by(PointerButton::Primary)
            && let Some(at) = ui.input(|i| i.pointer.interact_pos())
        {
            self.drag = Drag::None;
            self.click(ui, screen, at, mods, response.double_clicked(), theme);
        } else if response.double_clicked()
            && let Some(at) = ui.input(|i| i.pointer.interact_pos())
        {
            self.click(ui, screen, at, mods, true, theme);
        }
    }

    fn on_area_title(&self, screen: Rect, id: &str, at: Pos2) -> bool {
        let Some(n) = self.layout.get(id) else {
            return false;
        };
        let Some(r) = self.motion.rect(id) else {
            return false;
        };
        let sr = self.camera.rect_to_screen(screen, r);
        at.y < sr.min.y + (n.header + layout::AREA_PAD * 0.55) * self.camera.zoom
    }

    fn click(
        &mut self,
        ui: &mut Ui,
        screen: Rect,
        at: Pos2,
        mods: Modifiers,
        double: bool,
        _theme: &Theme,
    ) {
        let now = ui.input(|i| i.time);
        let hot = self.hot_at(at);
        let hit = self.hit(screen, at);
        let editing_id = self.editor.as_ref().map(|e| e.card_id.clone());
        if let Some(Hot::Resize { id }) = &hot {
            if double && let Some(key) = self.doc.board.node(id).map(|n| n.key().to_string()) {
                let src = edit::with_appendix(&self.doc.board, &self.doc.source, |a| {
                    a.entry_mut(&key).size = None
                });
                self.doc.apply(src, &self.selection, None);
            }
            return;
        }

        if let Some(Hot::Checkbox {
            card,
            at: off,
            line,
        }) = &hot
            && !double
        {
            if editing_id.as_deref() == Some(card.as_str()) {
                let has_title = self.editor.as_ref().is_some_and(Editor::has_title);
                if let Some(ed) = &mut self.editor {
                    ed.toggle_check(line + usize::from(has_title));
                }
                self.commit_editor(true);
            } else if let Some(off) = off
                && let Some(src) = edit::toggle_task(&self.doc.source, *off)
            {
                self.doc.apply(src, &self.selection, None);
            }
            return;
        }
        // Connections: click a label or the line itself to select, double-click to rename.
        let conn = match &hot {
            Some(Hot::Label { from, to }) => Some((from.clone(), to.clone())),
            _ if hot.is_none()
                && hit
                    .as_ref()
                    .is_none_or(|h| self.layout.get(h).is_some_and(|n| n.kind == Kind::Area)) =>
            {
                self.conn_at(at)
            }
            _ => None,
        };
        if let Some((from, to)) = conn
            && editing_id.is_none()
        {
            if double {
                let label = self
                    .doc
                    .board
                    .connections()
                    .into_iter()
                    .find(|(f, t, _)| *f == from && *t == to)
                    .map(|(_, _, c)| lowercase_first(&c.label))
                    .unwrap_or_default();
                let anchor = self
                    .conn_paths
                    .iter()
                    .find(|(f, t, _, _)| *f == from && *t == to)
                    .map_or(at, |(_, _, _, m)| *m);
                self.prompt = Some(Prompt {
                    kind: PromptKind::Relabel {
                        from: from.clone(),
                        to: to.clone(),
                    },
                    text: label,
                    anchor: anchor + vec2(-60.0, 14.0),
                    focus: true,
                });
            }
            self.selected_conn = Some((from, to));
            self.selection.clear();
            return;
        }
        self.selected_conn = None;
        if double
            && editing_id.is_none()
            && self.title_rect.is_some_and(|r| r.expand(6.0).contains(at))
            && let Some(t) = &self.doc.board.title
        {
            let r = self.title_rect.unwrap_or(Rect::NOTHING);
            self.prompt = Some(Prompt {
                kind: PromptKind::BoardTitle,
                text: t.text.clone(),
                anchor: r.left_bottom() + vec2(0.0, 6.0),
                focus: true,
            });
            return;
        }
        if let Some(Hot::Link { url }) = &hot
            && !double
            && editing_id.as_deref() != hit.as_deref()
        {
            if let Some(anchor) = url.strip_prefix('#') {
                if let Some(target) = self.doc.board.anchors.get(anchor).cloned()
                    && let Some(r) = self.motion.rect(&target)
                {
                    self.camera.fly_to(screen, r.expand(120.0), 60.0, 1.4);
                    self.selection = vec![target];
                }
            } else {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
            return;
        }

        // Clicking inside the card being edited moves the caret.
        if let Some(eid) = &editing_id
            && hit.as_deref() == Some(eid.as_str())
        {
            if let Some(pos) = self.editor_pos_at(at)
                && let Some(ed) = &mut self.editor
            {
                if double {
                    ed.select_word(pos);
                } else {
                    ed.set_cursor(pos, mods.shift);
                }
                self.blink_start = now;
            }
            return;
        }

        if self.editor.is_some() {
            self.finish_editing();
        }

        match hit {
            Some(id) if self.layout.get(&id).is_some_and(|n| n.kind == Kind::Card) => {
                if double {
                    self.selection = vec![id.clone()];
                    self.start_editing(&id, Some(at), false, now);
                } else if mods.shift || mods.command {
                    if let Some(i) = self.selection.iter().position(|s| *s == id) {
                        self.selection.remove(i);
                    } else {
                        self.selection.push(id);
                    }
                } else {
                    self.selection = vec![id];
                }
            }
            Some(id) => {
                // An area.
                let on_title = self.on_area_title(screen, &id, at);
                if double && on_title {
                    self.selection = vec![id.clone()];
                    self.rename_area(screen, &id);
                } else if double {
                    self.new_card_at(screen, &id, None, at, now);
                } else if on_title {
                    if mods.shift || mods.command {
                        if !self.selection.contains(&id) {
                            self.selection.push(id);
                        }
                    } else {
                        self.selection = vec![id];
                    }
                } else if !mods.shift {
                    self.selection.clear();
                }
            }
            None => {
                if double {
                    self.new_card_at(screen, "", None, at, now);
                } else if !mods.shift {
                    self.selection.clear();
                }
            }
        }
    }

    fn drop_moved(&mut self, ui: &Ui, screen: Rect, ids: &[String], ptr: Option<Pos2>) {
        let Some(ptr) = ptr else { return };
        let world_ptr = self.camera.to_world(screen, ptr);
        let target = self.area_at(world_ptr, ids);
        let mut src = self.doc.source.clone();
        let mut board = self.doc.board.clone();
        let mut placements: Vec<(String, Pos2)> = Vec::new();
        let mut keep_ids: Vec<(usize, String)> = Vec::new();
        for id in ids {
            let Some(rect) = self.motion.rect(id) else {
                continue;
            };
            placements.push((id.clone(), rect.min));
        }
        // Reparent in the Markdown where needed.
        for id in ids {
            let parent = self
                .layout
                .get(id)
                .and_then(|n| n.parent.clone())
                .unwrap_or_default();
            let parent_is_card = board.card(&parent).is_some();
            if parent != target
                && !parent_is_card
                && let Some(Edited { source, at }) = edit::move_node(&board, &src, id, &target)
            {
                src = source;
                board = Board::parse(&src);
                keep_ids.push((at, id.clone()));
            }
        }
        // Positions relative to the new parent's content origin.
        let origin = if target.is_empty() {
            Pos2::ZERO
        } else {
            self.layout
                .get(&target)
                .map(|n| n.content_origin)
                .unwrap_or(Pos2::ZERO)
        };
        let keys: Vec<(String, (i32, i32))> = placements
            .iter()
            .filter_map(|(id, min)| {
                let node = match keep_ids.iter().find(|(_, k)| k == id) {
                    Some((at, _)) => edit::node_at(&board, *at),
                    None => board.node(id),
                }?;
                let rel = *min - origin;
                let clamp = !target.is_empty();
                let (x, y) = if clamp {
                    (rel.x.max(0.0), rel.y.max(0.0))
                } else {
                    (rel.x, rel.y)
                };
                Some((node.key().to_string(), (x.round() as i32, y.round() as i32)))
            })
            .collect();
        // Nothing else should move: pin the neighbors where they are now, in the target area
        // and in every area around it (an area that grows must not push its neighbors).
        let moved: Vec<String> = ids.to_vec();
        let mut pins: Vec<(String, (i32, i32))> = Vec::new();
        // The target's chain of areas, and the chains the items came from.
        let mut containers: Vec<String> = Vec::new();
        let mut starts: Vec<String> = vec![target.clone()];
        for id in ids {
            starts.push(
                self.layout
                    .get(id)
                    .and_then(|n| n.parent.clone())
                    .unwrap_or_default(),
            );
        }
        for start in starts {
            let mut c = start;
            loop {
                if !containers.contains(&c) {
                    containers.push(c.clone());
                }
                if c.is_empty() {
                    break;
                }
                c = self
                    .layout
                    .get(&c)
                    .and_then(|n| n.parent.clone())
                    .unwrap_or_default();
            }
        }
        for container in containers {
            let children: Vec<String> = if container.is_empty() {
                self.doc
                    .board
                    .root
                    .children
                    .iter()
                    .map(|c| c.id().to_string())
                    .collect()
            } else {
                self.doc
                    .board
                    .area(&container)
                    .map(|a| a.children.iter().map(|c| c.id().to_string()).collect())
                    .unwrap_or_default()
            };
            let origin = if container.is_empty() {
                Pos2::ZERO
            } else {
                self.layout
                    .get(&container)
                    .map(|n| n.content_origin)
                    .unwrap_or(Pos2::ZERO)
            };
            for child in children {
                if moved.contains(&child) {
                    continue;
                }
                let (Some(node), Some(n)) = (self.doc.board.node(&child), self.layout.get(&child))
                else {
                    continue;
                };
                let key = node.key().to_string();
                if self
                    .doc
                    .board
                    .appendix
                    .get(&key)
                    .is_some_and(|e| e.at.is_some())
                {
                    continue;
                }
                let rel = n.rect.min - origin;
                if !pins.iter().any(|(k, _)| *k == key) {
                    pins.push((key, (rel.x.round() as i32, rel.y.round() as i32)));
                }
            }
        }
        src = edit::with_appendix(&board, &src, |a| {
            for (k, at) in pins.iter().chain(keys.iter()) {
                a.entry_mut(k).at = Some(*at);
            }
        });
        let sel = self.selection.clone();
        self.doc.apply(src, &sel, None);
        // Keep the moved items selected under their (possibly new) ids.
        let mut new_sel = Vec::new();
        for (at, old) in &keep_ids {
            if let Some(n) = edit::node_at(&self.doc.board, *at) {
                self.motion.rename(old, n.id());
                new_sel.push(n.id().to_string());
            }
        }
        if !keep_ids.is_empty() {
            self.selection = new_sel;
        }
        let _ = ui;
    }

    // =========================================================================================
    // Editing
    // =========================================================================================

    fn start_editing(&mut self, id: &str, at: Option<Pos2>, fresh: bool, now: f64) {
        let Some(card) = self.doc.board.card(id) else {
            return;
        };
        let mut ed = Editor::new(card);
        self.edit_text = None;
        if let Some(at) = at {
            // The card's text layout is known after the next paint: place the caret then.
            let end = ed.end();
            ed.set_cursor(end, false);
            self.editor = Some(ed);
            self.pending_caret = Some(at);
        } else {
            if !fresh {
                let end = ed.end();
                ed.set_cursor(end, false);
            }
            self.editor = Some(ed);
        }
        self.reveal_pending = true;
        // A fresh card and its first words are one undo step.
        if !fresh {
            self.doc.break_coalescing();
        }
        self.blink_start = now;
    }

    /// Writes the editor's text into the document.
    fn commit_editor(&mut self, force_step: bool) {
        let Some(ed) = &mut self.editor else { return };
        if !ed.dirty {
            return;
        }
        ed.dirty = false;
        let board = &self.doc.board;
        let Some(card) = edit::node_at(board, ed.card_start) else {
            return;
        };
        let card_id = card.id().to_string();
        let lines = ed.body_lines();
        let title = ed.title();
        if let Some(Edited { source, at }) =
            edit::set_card_text(board, &self.doc.source, &card_id, title.as_deref(), &lines)
        {
            let key = format!("edit:{}", ed.card_start);
            if force_step {
                self.doc.break_coalescing();
            }
            self.doc.apply(source, &self.selection, Some(&key));
            if let Some(NodeRef::Card(card)) = edit::node_at(&self.doc.board, at) {
                let old = ed.card_id.clone();
                ed.refresh_origins(card);
                if old != card.id {
                    self.motion.rename(&old, &card.id);
                    if let Some(t) = self.tilts.remove(&old) {
                        self.tilts.insert(card.id.clone(), t);
                    }
                    for s in &mut self.selection {
                        if *s == old {
                            *s = card.id.clone();
                        }
                    }
                }
            } else {
                // The card disappeared (emptied loose card).
                self.editor = None;
            }
        }
    }

    fn finish_editing(&mut self) {
        self.commit_editor(false);
        if let Some(ed) = self.editor.take() {
            // A card left without any text is removed (a fresh card abandoned, or text erased).
            if ed.is_empty()
                && let Some(n) = edit::node_at(&self.doc.board, ed.card_start)
            {
                let id = n.id().to_string();
                if let Some(src) = edit::delete_node(&self.doc.board, &self.doc.source, &id) {
                    self.doc.apply(
                        src,
                        &self.selection,
                        Some(&format!("edit:{}", ed.card_start)),
                    );
                    self.selection.retain(|s| *s != id);
                }
            }
            self.doc.break_coalescing();
        }
    }

    fn editor_pos_at(&self, at: Pos2) -> Option<Pos> {
        let (_, text, k) = self.edit_text.as_ref()?;
        let ed = self.editor.as_ref()?;
        let card_rect = self.motion.rect(&ed.card_id)?;
        let _ = card_rect;
        let has_title = ed.has_title();
        // Find the row whose vertical span contains the point (or the nearest).
        let (sr, _, _) = self.edit_text.as_ref()?;
        let pad = CARD_PAD * self.camera.zoom;
        let origin = sr.min + Vec2::splat(pad);
        let mut best: Option<(&content::Row, f32)> = None;
        for row in &text.rows {
            let top = origin.y + row.offset.y * k;
            let bottom = top + row.galley.size().y.max(row.size.y) * k;
            let d = if at.y < top {
                top - at.y
            } else if at.y > bottom {
                at.y - bottom
            } else {
                0.0
            };
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((row, d));
            }
        }
        let (row, _) = best?;
        let local = (at - (origin + row.offset * *k)) / *k;
        let c = row.galley.cursor_from_pos(local);
        let line = match row.line {
            None => 0,
            Some(i) => i + usize::from(has_title),
        };
        Some(Pos {
            line,
            col: c.index.0,
        })
    }

    fn new_card_at(
        &mut self,
        screen: Rect,
        area_id: &str,
        after: Option<&str>,
        at: Pos2,
        now: f64,
    ) {
        let area_id = if self.doc.board.area(area_id).is_some() {
            area_id.to_string()
        } else {
            String::new()
        };
        if let Some(Edited { source, at: off }) =
            edit::insert_card(&self.doc.board, &self.doc.source, &area_id, after, "")
        {
            self.doc.break_coalescing();
            self.doc
                .apply(source, &self.selection, Some(&format!("edit:{off}")));
            self.motion.spawn_from = Some(self.camera.to_world(screen, at));
            if let Some(n) = edit::node_at(&self.doc.board, off) {
                let id = n.id().to_string();
                self.selection = vec![id.clone()];
                self.start_editing(&id, None, true, now);
                if let Some(ed) = &mut self.editor {
                    ed.set_cursor(Pos { line: 0, col: 0 }, false);
                    ed.card_start = off;
                }
            }
        }
    }

    fn rename_area(&mut self, screen: Rect, id: &str) {
        let Some(area) = self.doc.board.area(id) else {
            return;
        };
        let Some(h) = &area.heading else { return };
        let Some(r) = self.motion.rect(id) else {
            return;
        };
        let sr = self.camera.rect_to_screen(screen, r);
        self.prompt = Some(Prompt {
            kind: PromptKind::AreaTitle {
                id: id.to_string(),
                start: area.section.start,
            },
            text: h.text.clone(),
            anchor: sr.min
                + vec2(
                    layout::AREA_PAD * self.camera.zoom,
                    layout::AREA_PAD * 0.4 * self.camera.zoom,
                ),
            focus: true,
        });
    }

    // =========================================================================================
    // Keyboard
    // =========================================================================================

    fn keyboard(&mut self, ui: &mut Ui, screen: Rect, _theme: &Theme) -> Option<UiRequest> {
        let events = ui.input(|i| i.events.clone());
        let now = ui.input(|i| i.time);
        let mut request = None;
        for ev in events {
            if self.editor.is_some() && self.editor_event(ui, &ev, now) {
                continue;
            }
            match ev {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if let Some(r) = self.board_key(ui, screen, key, modifiers, now) {
                        request = Some(r);
                    }
                }
                Event::Paste(text) => self.paste_text(screen, &text, now),
                Event::PasteImage(img) => self.paste_image(ui, screen, &img),
                Event::Copy | Event::Cut if self.editor.is_none() && !self.selection.is_empty() => {
                    let mut parts = Vec::new();
                    for id in &self.selection {
                        if let Some(n) = self.doc.board.node(id) {
                            let r = n.range();
                            parts.push(self.doc.source[r].trim().to_string());
                        }
                    }
                    ui.ctx().copy_text(parts.join("\n\n"));
                    if matches!(ev, Event::Cut) {
                        self.delete_selected();
                    }
                }
                _ => {}
            }
        }
        request
    }

    /// Handles an event while editing. Returns true if it was consumed.
    fn editor_event(&mut self, ui: &mut Ui, ev: &Event, now: f64) -> bool {
        let ctx = ui.ctx().clone();
        let mut action = Action::None;
        let mut consumed = true;
        let Some(ed) = &mut self.editor else {
            return false;
        };
        if std::env::var("SKISS_DEBUG").is_ok() {
            eprintln!("editor event: {ev:?}");
        }
        match ev {
            Event::Text(t) => action = ed.insert_text(t),
            Event::Ime(egui::ImeEvent::Commit(t)) => action = ed.insert_text(t),
            Event::Paste(t) => action = ed.paste(t),
            Event::Copy => {
                if ed.has_selection() {
                    ctx.copy_text(ed.selected_text());
                }
            }
            Event::Cut => {
                if ed.has_selection() {
                    ctx.copy_text(ed.selected_text());
                    action = ed.backspace(false);
                }
            }
            Event::PasteImage(_) => consumed = false,
            Event::Key {
                key,
                pressed: true,
                modifiers: m,
                ..
            } => {
                let word = m.alt || (m.ctrl && !cfg!(target_os = "macos"));
                let line_jump =
                    m.command && cfg!(target_os = "macos") || (!cfg!(target_os = "macos") && false);
                match key {
                    Key::Escape => {
                        self.finish_editing();
                        return true;
                    }
                    Key::Enter if m.command => {
                        self.finish_editing();
                        return true;
                    }
                    Key::Enter if m.shift => action = ed.line_break(),
                    Key::Enter => action = ed.newline(),
                    Key::Backspace => action = ed.backspace(word || line_jump),
                    Key::Delete => action = ed.delete_forward(word),
                    Key::Tab => {
                        action = ed.indent(m.shift);
                        if action == Action::None && !m.shift {
                            // Tab outside a list: finish and add the next card.
                            let id = ed.card_id.clone();
                            self.finish_editing();
                            self.new_sibling(&id, now);
                            return true;
                        }
                    }
                    Key::ArrowLeft => {
                        let p = if line_jump {
                            Pos {
                                line: ed.cursor.line,
                                col: 0,
                            }
                        } else if ed.has_selection() && !m.shift {
                            ed.selection().0
                        } else if word {
                            ed.word_left(ed.cursor)
                        } else {
                            ed.left(ed.cursor)
                        };
                        ed.set_cursor(p, m.shift);
                        ed.preferred_x = None;
                    }
                    Key::ArrowRight => {
                        let p = if line_jump {
                            Pos {
                                line: ed.cursor.line,
                                col: ed.lines[ed.cursor.line].chars.len(),
                            }
                        } else if ed.has_selection() && !m.shift {
                            ed.selection().1
                        } else if word {
                            ed.word_right(ed.cursor)
                        } else {
                            ed.right(ed.cursor)
                        };
                        ed.set_cursor(p, m.shift);
                        ed.preferred_x = None;
                    }
                    Key::ArrowUp | Key::ArrowDown => {
                        let down = *key == Key::ArrowDown;
                        if m.command {
                            let p = if down { ed.end() } else { Pos::default() };
                            ed.set_cursor(p, m.shift);
                        } else if let Some(p) = self.vertical(down)
                            && let Some(ed) = &mut self.editor
                        {
                            ed.set_cursor(p, m.shift);
                        }
                    }
                    Key::Home => {
                        let p = Pos {
                            line: ed.cursor.line,
                            col: 0,
                        };
                        ed.set_cursor(p, m.shift);
                    }
                    Key::End => {
                        let p = Pos {
                            line: ed.cursor.line,
                            col: ed.lines[ed.cursor.line].chars.len(),
                        };
                        ed.set_cursor(p, m.shift);
                    }
                    Key::A if m.command => ed.select_all(),
                    Key::B if m.command => action = ed.toggle_style(|s| &mut s.bold),
                    Key::I if m.command => action = ed.toggle_style(|s| &mut s.italic),
                    Key::E if m.command => action = ed.toggle_style(|s| &mut s.code),
                    Key::X if m.command && m.shift => action = ed.toggle_style(|s| &mut s.strike),
                    Key::L if m.command => action = ed.toggle_task(),
                    Key::Z if m.command => {
                        // Undo leaves editing and undoes the last change.
                        self.commit_editor(true);
                        self.editor = None;
                        consumed = false;
                    }
                    Key::Y if m.command => {
                        self.commit_editor(true);
                        self.editor = None;
                        consumed = false;
                    }
                    _ => {
                        consumed = !(m.command
                            && matches!(
                                key,
                                Key::Equals | Key::Plus | Key::Minus | Key::Num0 | Key::T
                            ));
                    }
                }
            }
            Event::Key { pressed: false, .. } => {}
            _ => consumed = false,
        }
        self.blink_start = now;
        match action {
            Action::Changed => {
                let structural = matches!(
                    ev,
                    Event::Key {
                        key: Key::Enter | Key::Tab,
                        ..
                    }
                ) || matches!(ev, Event::Key { modifiers, .. } if modifiers.command);
                self.commit_editor(structural);
                self.reveal_caret_card();
            }
            Action::NewSibling => {
                let id = self
                    .editor
                    .as_ref()
                    .map(|e| e.card_id.clone())
                    .unwrap_or_default();
                self.commit_editor(true);
                self.finish_editing();
                self.new_sibling(&id, now);
            }
            Action::None => {}
        }
        consumed
    }

    fn reveal_caret_card(&mut self) {
        self.reveal_pending = true;
    }

    /// Up/down by visual row inside the editing card.
    fn vertical(&self, down: bool) -> Option<Pos> {
        let ed = self.editor.as_ref()?;
        let (_, text, _) = self.edit_text.as_ref()?;
        let has_title = ed.has_title();
        let line_of = |r: &content::Row| match r.line {
            None => 0,
            Some(i) => i + usize::from(has_title),
        };
        let idx = text
            .rows
            .iter()
            .position(|r| line_of(r) == ed.cursor.line)?;
        let row = &text.rows[idx];
        let cur = row
            .galley
            .pos_from_cursor(egui::text::CCursor::new(ed.cursor.col));
        let x = row.offset.x + cur.min.x;
        // Within the same paragraph?
        let target_y = if down {
            cur.max.y + 1.0
        } else {
            cur.min.y - 1.0
        };
        if target_y >= 0.0 && target_y < row.galley.size().y {
            let c = row.galley.cursor_from_pos(vec2(cur.min.x, target_y));
            return Some(Pos {
                line: ed.cursor.line,
                col: c.index.0,
            });
        }
        let next = if down {
            text.rows.get(idx + 1)?
        } else {
            text.rows.get(idx.checked_sub(1)?)?
        };
        let y = if down {
            1.0
        } else {
            next.galley.size().y - 1.0
        };
        let c = next.galley.cursor_from_pos(vec2(x - next.offset.x, y));
        Some(Pos {
            line: line_of(next),
            col: c.index.0,
        })
    }

    fn new_sibling(&mut self, id: &str, now: f64) {
        let parent = self
            .doc
            .board
            .parent_area(id)
            .map(|a| a.id.clone())
            .unwrap_or_default();
        let after_card = self.doc.board.card(id).map(|c| c.id.clone());
        let parent_card = self
            .layout
            .get(id)
            .and_then(|n| n.parent.clone())
            .filter(|p| self.doc.board.card(p).is_some());
        let _ = parent_card;
        if let Some(Edited { source, at }) = edit::insert_card(
            &self.doc.board,
            &self.doc.source,
            &parent,
            after_card.as_deref(),
            "",
        ) {
            self.doc.break_coalescing();
            self.doc
                .apply(source, &self.selection, Some(&format!("edit:{at}")));
            if let Some(r) = self.motion.rect(id) {
                self.motion.spawn_from = Some(r.right_top() + vec2(20.0, 20.0));
            }
            if let Some(n) = edit::node_at(&self.doc.board, at) {
                let nid = n.id().to_string();
                self.selection = vec![nid.clone()];
                self.start_editing(&nid, None, true, now);
                if let Some(ed) = &mut self.editor {
                    ed.set_cursor(Pos { line: 0, col: 0 }, false);
                }
            }
        }
    }

    fn board_key(
        &mut self,
        ui: &mut Ui,
        screen: Rect,
        key: Key,
        m: Modifiers,
        now: f64,
    ) -> Option<UiRequest> {
        match key {
            Key::Z if m.command && m.shift => self.redo(),
            Key::Z if m.command => self.undo(),
            Key::Y if m.command => self.redo(),
            Key::T if m.command => return Some(UiRequest::ToggleTheme),
            Key::Equals | Key::Plus if m.command && m.shift => {
                self.text_scale = (self.text_scale * 1.15).min(3.0)
            }
            Key::Minus if m.command && m.shift => {
                self.text_scale = (self.text_scale / 1.15).max(0.6)
            }
            Key::Equals | Key::Plus if m.command => {
                self.camera.glide_zoom(screen, screen.center(), 1.35)
            }
            Key::Minus if m.command => self.camera.glide_zoom(screen, screen.center(), 1.0 / 1.35),
            Key::Num0 if m.command => self.fit(screen),
            Key::Num1 if m.command => {
                let c = self.camera.center;
                self.camera
                    .fly_to(screen, Rect::from_center_size(c, screen.size()), 0.0, 1.0);
            }
            Key::F if !m.command && !m.shift => self.fit(screen),
            Key::A if m.command => {
                self.selection = self
                    .layout
                    .order
                    .iter()
                    .filter(|id| {
                        self.layout.get(id).is_some_and(|n| {
                            n.kind == Kind::Card && n.depth == 0 || n.kind == Kind::Card
                        })
                    })
                    .cloned()
                    .collect();
            }
            Key::G if m.command => self.group_selected(screen),
            Key::F if m.command => self.open_search(),
            Key::A
                if !m.command
                    && self.selection.len() == 1
                    && self.doc.board.card(&self.selection[0]).is_some() =>
            {
                let at = self
                    .motion
                    .rect(&self.selection[0])
                    .map(|r| self.camera.rect_to_screen(screen, r).center_top())
                    .unwrap_or(screen.center());
                self.ai_menu = Some(at);
            }
            Key::R if !m.command => {
                if let Some(id) = self.selection.first().cloned() {
                    self.about = if self.about.as_deref() == Some(id.as_str()) {
                        None
                    } else {
                        Some(id)
                    };
                }
            }
            Key::L if m.command && m.shift => self.tidy_up(),
            Key::Escape => {
                if self.help {
                    self.help = false;
                } else if self.search.is_some() || self.about.is_some() {
                    self.search = None;
                    self.about = None;
                } else if !self.selection.is_empty() || self.selected_conn.is_some() {
                    self.selection.clear();
                    self.selected_conn = None;
                } else if now - self.esc_at < 1.6 {
                    // Esc twice closes skiss, like MDeck (everything is already saved).
                    self.save_now();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.esc_at = now;
                    self.toast = Some(("Press Esc again to close skiss".into(), now));
                }
            }
            Key::Questionmark | Key::F1 => self.help = !self.help,
            Key::Slash if m.shift => self.help = !self.help,
            Key::Delete | Key::Backspace => {
                if let Some((from, to)) = self.selected_conn.take() {
                    if let Some(src) =
                        edit::remove_connection(&self.doc.board, &self.doc.source, &from, &to)
                    {
                        self.doc.apply(src, &self.selection, None);
                    }
                } else {
                    self.delete_selected();
                }
            }
            Key::Enter | Key::F2 => {
                if let Some(id) = self.selection.first().cloned() {
                    if self.doc.board.card(&id).is_some() {
                        self.start_editing(&id, None, false, now);
                    } else if self.doc.board.area(&id).is_some() {
                        self.rename_area(screen, &id);
                    }
                }
            }
            Key::Tab => {
                if let Some(id) = self.selection.first().cloned() {
                    if self.doc.board.card(&id).is_some() {
                        self.new_sibling(&id, now);
                    }
                } else {
                    let center = screen.center();
                    self.new_card_at(screen, "", None, center, now);
                }
            }
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => {
                self.select_neighbor(screen, key)
            }
            Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4 | Key::Num5 | Key::Num6 | Key::Num7
                if !m.command && !self.selection.is_empty() =>
            {
                let i = [
                    Key::Num1,
                    Key::Num2,
                    Key::Num3,
                    Key::Num4,
                    Key::Num5,
                    Key::Num6,
                    Key::Num7,
                ]
                .iter()
                .position(|k| *k == key)
                .unwrap_or(0);
                self.set_color(Color::ALL[i]);
            }
            Key::Num0 if !m.command && !self.selection.is_empty() => self.clear_color(),
            _ => {}
        }
        let _ = ui;
        None
    }

    /// Forgets hand-placed positions (of the selected areas, or everywhere) so skiss lays the
    /// board out afresh. Colors stay.
    pub fn tidy_up(&mut self) {
        let scope: Vec<String> = self
            .selection
            .iter()
            .filter(|id| self.doc.board.area(id).is_some())
            .cloned()
            .collect();
        let keys: Vec<String> = self
            .doc
            .board
            .walk()
            .into_iter()
            .filter(|n| !n.id().is_empty())
            .filter(|n| scope.is_empty() || scope.iter().any(|a| self.is_inside(n.id(), a)))
            .map(|n| n.key().to_string())
            .collect();
        let src = edit::with_appendix(&self.doc.board, &self.doc.source, |a| {
            for k in &keys {
                let e = a.entry_mut(k);
                e.at = None;
                e.size = None;
            }
        });
        if src == self.doc.source {
            self.toast = Some(("Already tidy".into(), self.last_time.unwrap_or(0.0)));
        } else {
            self.doc.apply(src, &self.selection, None);
        }
    }

    fn fit(&mut self, screen: Rect) {
        let b = self.layout.bounds;
        self.camera.fly_to(screen, b, 80.0, 1.15);
    }

    fn select_neighbor(&mut self, screen: Rect, key: Key) {
        let Some(cur) = self.selection.first().and_then(|id| self.motion.rect(id)) else {
            // Nothing selected: pick the first card.
            if let Some(id) = self
                .layout
                .order
                .iter()
                .find(|id| self.layout.get(id).is_some_and(|n| n.kind == Kind::Card))
            {
                self.selection = vec![id.clone()];
            }
            return;
        };
        let dir = match key {
            Key::ArrowLeft => vec2(-1.0, 0.0),
            Key::ArrowRight => vec2(1.0, 0.0),
            Key::ArrowUp => vec2(0.0, -1.0),
            _ => vec2(0.0, 1.0),
        };
        let mut best: Option<(String, f32)> = None;
        for id in &self.layout.order {
            if self.selection.contains(id)
                || self.layout.get(id).is_none_or(|n| n.kind != Kind::Card)
            {
                continue;
            }
            let Some(r) = self.motion.rect(id) else {
                continue;
            };
            let d = r.center() - cur.center();
            let along = d.dot(dir);
            if along <= 1.0 {
                continue;
            }
            let across = (d - dir * along).length();
            let score = along + across * 2.5;
            if best.as_ref().is_none_or(|(_, s)| score < *s) {
                best = Some((id.clone(), score));
            }
        }
        if let Some((id, _)) = best {
            if let Some(r) = self.motion.rect(&id) {
                self.camera.reveal(screen, r, 60.0);
            }
            self.selection = vec![id];
        }
    }

    fn undo(&mut self) {
        self.editor = None;
        if let Some(sel) = self.doc.undo(&self.selection) {
            self.selection = sel;
        }
    }

    fn redo(&mut self) {
        self.editor = None;
        if let Some(sel) = self.doc.redo(&self.selection) {
            self.selection = sel;
        }
    }

    fn delete_selected(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let mut src = self.doc.source.clone();
        // Delete from the end of the file backwards so earlier offsets stay valid.
        let mut ids: Vec<(usize, String)> = self
            .selection
            .iter()
            .filter_map(|id| {
                self.doc
                    .board
                    .node(id)
                    .map(|n| (n.range().start, id.clone()))
            })
            .collect();
        ids.sort_by_key(|x| std::cmp::Reverse(x.0));
        for (_, id) in ids {
            let board = Board::parse(&src);
            if let Some(out) = edit::delete_node(&board, &src, &id) {
                src = out;
            }
        }
        let sel = self.selection.clone();
        self.doc.apply(src, &sel, None);
        self.selection.clear();
    }

    fn group_selected(&mut self, screen: Rect) {
        let cards: Vec<String> = self.selection.clone();
        if cards.is_empty() {
            return;
        }
        match edit::group(&self.doc.board, &self.doc.source, &cards, "") {
            Some(Edited { source, at }) => {
                self.doc.apply(source, &self.selection, None);
                if let Some(n) = edit::node_at(&self.doc.board, at) {
                    let id = n.id().to_string();
                    self.selection = vec![id.clone()];
                    // Let the layout settle one frame, then ask for the title.
                    self.relayout_pending_rename(screen, &id);
                }
            }
            None => {
                self.toast = Some((
                    "Group things that share the same area".into(),
                    self.last_time.unwrap_or(0.0),
                ))
            }
        }
    }

    fn relayout_pending_rename(&mut self, screen: Rect, id: &str) {
        let start = self
            .doc
            .board
            .area(id)
            .map(|a| a.section.start)
            .unwrap_or(0);
        let anchor = self
            .motion
            .rect(id)
            .map(|r| self.camera.rect_to_screen(screen, r).min)
            .unwrap_or(screen.center());
        self.prompt = Some(Prompt {
            kind: PromptKind::AreaTitle {
                id: id.to_string(),
                start,
            },
            text: String::new(),
            anchor,
            focus: true,
        });
    }

    fn target_area(&self) -> String {
        self.selection
            .first()
            .and_then(|id| {
                if self.doc.board.area(id).is_some() {
                    Some(id.clone())
                } else {
                    self.doc.board.parent_area(id).map(|a| a.id.clone())
                }
            })
            .unwrap_or_default()
    }

    fn paste_text(&mut self, screen: Rect, text: &str, now: f64) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let area = self.target_area();
        if let Some(Edited { source, at }) =
            edit::paste_markdown(&self.doc.board, &self.doc.source, &area, text)
        {
            self.doc.apply(source, &self.selection, None);
            self.motion.spawn_from = Some(self.camera.center);
            if let Some(n) = edit::node_at(&self.doc.board, at) {
                self.selection = vec![n.id().to_string()];
            }
        }
        let _ = (screen, now);
    }

    fn paste_image(&mut self, ui: &Ui, screen: Rect, img: &egui::ColorImage) {
        let assets = self.doc.assets_dir();
        let dir = self.doc.dir();
        let rel = match images::save_pasted(&assets, &dir, img) {
            Ok(r) => r,
            Err(e) => {
                self.toast(ui, format!("Could not save the image: {e}"));
                return;
            }
        };
        if let Some(ed) = &mut self.editor {
            ed.insert_image("Pasted image", &rel);
            self.commit_editor(true);
            return;
        }
        let md = format!("![Pasted image]({rel})");
        let sel = self.selection.first().cloned();
        let result = match sel.as_deref().and_then(|id| self.doc.board.card(id)) {
            Some(c) if c.kind == CardKind::Section => {
                edit::append_to_card(&self.doc.board, &self.doc.source, &c.id.clone(), &md)
            }
            _ => {
                let area = self.target_area();
                edit::insert_loose(&self.doc.board, &self.doc.source, &area, &md)
            }
        };
        if let Some(Edited { source, at }) = result {
            self.doc.apply(source, &self.selection, None);
            if let Some(ptr) = ui.input(|i| i.pointer.latest_pos()) {
                self.motion.spawn_from = Some(self.camera.to_world(screen, ptr));
            }
            if let Some(n) = edit::node_at(&self.doc.board, at) {
                self.selection = vec![n.id().to_string()];
            }
        }
    }

    // =========================================================================================
    // Overlays
    // =========================================================================================

    fn toolbar(&mut self, ui: &mut Ui, screen: Rect, theme: &Theme) {
        if self.selection.is_empty() || !matches!(self.drag, Drag::None) || self.prompt.is_some() {
            return;
        }
        let mut bounds = Rect::NOTHING;
        for id in &self.selection {
            if let Some(r) = self.motion.rect(id) {
                bounds = bounds.union(self.camera.rect_to_screen(screen, r));
            }
        }
        if bounds == Rect::NOTHING || !screen.intersects(bounds) {
            return;
        }
        let pos = pos2(
            bounds.center().x,
            (bounds.min.y - 12.0).max(screen.min.y + 40.0),
        );
        let editing = self.editor.is_some();
        let single_card =
            self.selection.len() == 1 && self.doc.board.card(&self.selection[0]).is_some();
        let mut actions: Vec<ToolbarAction> = Vec::new();
        let current_color = self.selection.first().and_then(|id| {
            let key = self.doc.board.node(id)?.key().to_string();
            self.doc.board.appendix.get(&key)?.color
        });
        egui::Area::new(egui::Id::unique("skiss-toolbar"))
            .fixed_pos(pos)
            .pivot(Align2::CENTER_BOTTOM)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme.ui_bg)
                    .corner_radius(9.0)
                    .inner_margin(egui::Margin::symmetric(8, 5))
                    .shadow(egui::Shadow {
                        offset: [0, 3],
                        blur: 12,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let tb = |ui: &mut Ui, text: egui::RichText, tip: &str| {
                                ui.add(
                                    egui::Button::new(text.color(theme.ui_text))
                                        .frame(false)
                                        .min_size(vec2(22.0, 22.0)),
                                )
                                .on_hover_text(keys(tip))
                                .clicked()
                            };
                            if editing {
                                if tb(
                                    ui,
                                    egui::RichText::new("B").strong().size(15.0),
                                    "Bold (⌘B)",
                                ) {
                                    actions.push(ToolbarAction::Bold);
                                }
                                if tb(
                                    ui,
                                    egui::RichText::new("I").italics().size(15.0),
                                    "Italic (⌘I)",
                                ) {
                                    actions.push(ToolbarAction::Italic);
                                }
                                if tb(
                                    ui,
                                    egui::RichText::new("S").strikethrough().size(15.0),
                                    "Strikethrough (⌘⇧X)",
                                ) {
                                    actions.push(ToolbarAction::Strike);
                                }
                                if tb(ui, egui::RichText::new("Task").size(13.0), "Task (⌘L)") {
                                    actions.push(ToolbarAction::Task);
                                }
                                ui.add_space(2.0);
                                ui.separator();
                            }
                            for c in Color::ALL {
                                let fill = theme.paper(c);
                                let (r, resp) =
                                    ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                                let on = current_color == Some(c);
                                ui.painter().circle(
                                    r.center(),
                                    7.5,
                                    fill,
                                    Stroke::new(
                                        if on { 2.5 } else { 1.2 },
                                        if on { theme.accent } else { theme.ink(c) },
                                    ),
                                );
                                let name =
                                    self.doc.board.appendix.color_name(c).map(str::to_string);
                                let tip = match &name {
                                    Some(n) => format!(
                                        "{} · {n}  (right-click to rename)",
                                        capitalize(c.name())
                                    ),
                                    None => format!(
                                        "{}  (right-click to name it)",
                                        capitalize(c.name())
                                    ),
                                };
                                let resp = resp.on_hover_text(tip);
                                if resp.clicked() {
                                    actions.push(ToolbarAction::Color(c));
                                }
                                if resp.secondary_clicked() {
                                    actions.push(ToolbarAction::NameColor(c, r.center()));
                                }
                            }
                            ui.separator();
                            if !editing
                                && tb(ui, egui::RichText::new("Edit").size(13.0), "Edit (Enter)")
                            {
                                actions.push(ToolbarAction::Edit);
                            }
                            if !editing
                                && tb(
                                    ui,
                                    egui::RichText::new("Related").size(13.0),
                                    "Everything about it (R)",
                                )
                            {
                                actions.push(ToolbarAction::Related);
                            }
                            if !editing
                                && single_card
                                && tb(
                                    ui,
                                    egui::RichText::new("AI").size(13.0),
                                    "Spelling, rewriting, translating, pictures",
                                )
                            {
                                actions.push(ToolbarAction::Ai);
                            }
                            if tb(ui, egui::RichText::new("Delete").size(13.0), "Delete (⌫)") {
                                actions.push(ToolbarAction::Delete);
                            }
                        });
                    });
            });
        let now = ui.input(|i| i.time);
        let toolbar_bottom = pos2(pos.x, pos.y + 6.0);
        for a in actions {
            match a {
                ToolbarAction::Bold
                | ToolbarAction::Italic
                | ToolbarAction::Strike
                | ToolbarAction::Task => {
                    if let Some(ed) = &mut self.editor {
                        let r = match a {
                            ToolbarAction::Bold => ed.toggle_style(|s| &mut s.bold),
                            ToolbarAction::Italic => ed.toggle_style(|s| &mut s.italic),
                            ToolbarAction::Strike => ed.toggle_style(|s| &mut s.strike),
                            _ => ed.toggle_task(),
                        };
                        if r == Action::Changed {
                            self.commit_editor(true);
                        }
                    }
                }
                ToolbarAction::Color(c) => self.set_color(c),
                ToolbarAction::NameColor(c, at) => {
                    self.prompt = Some(Prompt {
                        kind: PromptKind::ColorName { color: c },
                        text: self
                            .doc
                            .board
                            .appendix
                            .color_name(c)
                            .unwrap_or("")
                            .to_string(),
                        anchor: at + vec2(0.0, 18.0),
                        focus: true,
                    })
                }
                ToolbarAction::Edit => {
                    if let Some(id) = self.selection.first().cloned() {
                        if self.doc.board.card(&id).is_some() {
                            self.start_editing(&id, None, false, now);
                        } else {
                            self.rename_area(screen, &id);
                        }
                    }
                }
                ToolbarAction::Delete => {
                    self.editor = None;
                    self.delete_selected();
                }
                ToolbarAction::Related => {
                    self.about = self.selection.first().cloned();
                }
                ToolbarAction::Ai => {
                    self.ai_menu = if self.ai_menu.is_some() {
                        None
                    } else {
                        Some(toolbar_bottom)
                    };
                }
            }
        }
    }

    fn clear_color(&mut self) {
        let keys: Vec<String> = self
            .selection
            .iter()
            .filter_map(|id| self.doc.board.node(id).map(|n| n.key().to_string()))
            .collect();
        let src = edit::with_appendix(&self.doc.board, &self.doc.source, |a| {
            for k in &keys {
                a.entry_mut(k).color = None;
            }
        });
        self.doc.apply(src, &self.selection, None);
    }

    /// What is in focus while searching or showing related things (`None`: everything).
    fn compute_focus(&mut self) {
        let mut ids: Option<Vec<String>> = None;
        if let Some(s) = &self.search
            && (!s.query.trim().is_empty() || s.color.is_some())
        {
            ids = Some(s.results.clone());
        } else if let Some(a) = &self.about {
            let r = query::related(&self.doc.board, a);
            let mut v: Vec<String> = vec![a.clone()];
            v.extend(r.outgoing.iter().map(|l| l.other.clone()));
            v.extend(r.incoming.iter().map(|l| l.other.clone()));
            v.extend(r.mentions.iter().cloned());
            ids = Some(v);
        }
        self.focus = ids.map(|ids| {
            let mut set: std::collections::HashSet<String> = std::collections::HashSet::new();
            for id in &ids {
                // A focused card keeps the areas around it readable, and an area its cards.
                set.insert(id.clone());
                let mut cur = self.layout.get(id).and_then(|n| n.parent.clone());
                while let Some(p) = cur {
                    cur = self.layout.get(&p).and_then(|n| n.parent.clone());
                    set.insert(p);
                }
            }
            for id in &ids {
                for other in self.layout.nodes.keys() {
                    if self.is_inside(other, id) {
                        set.insert(other.clone());
                    }
                }
            }
            set
        });
    }

    /// 1 for things in focus, faint for the rest.
    fn dim_factor(&self, id: &str) -> f32 {
        match &self.focus {
            Some(set) if !set.contains(id) => 0.22,
            _ => 1.0,
        }
    }

    fn open_search(&mut self) {
        if let Some(ed) = self.editor.as_ref().map(|e| e.card_id.clone()) {
            let _ = ed;
            self.finish_editing();
        }
        self.search = Some(Search {
            query: String::new(),
            color: None,
            results: Vec::new(),
            index: 0,
            focus: true,
        });
    }

    fn run_search(&mut self, screen: Rect) {
        let Some(s) = &mut self.search else { return };
        let board = &self.doc.board;
        let mut results = if s.query.trim().is_empty() {
            board
                .walk()
                .into_iter()
                .filter(|n| !n.id().is_empty())
                .map(|n| n.id().to_string())
                .collect()
        } else {
            query::search(board, &s.query)
        };
        if let Some(c) = s.color {
            results.retain(|id| {
                board
                    .node(id)
                    .and_then(|n| board.appendix.get(n.key()))
                    .is_some_and(|e| e.color == Some(c))
            });
        } else if s.query.trim().is_empty() {
            results.clear();
        }
        s.results = results;
        s.index = 0;
        self.fly_to_hit(screen);
    }

    fn fly_to_hit(&mut self, screen: Rect) {
        let Some(s) = &self.search else { return };
        let Some(id) = s.results.get(s.index).cloned() else {
            return;
        };
        if let Some(r) = self.motion.rect(&id) {
            self.camera.fly_to(screen, r.expand(140.0), 60.0, 1.3);
        }
        self.selection = vec![id];
    }

    fn search_ui(&mut self, ui: &mut Ui, screen: Rect, theme: &Theme) {
        let Some(s) = &mut self.search else { return };
        let pc = PromptColors::of(theme);
        let mut changed = false;
        let mut step: i32 = 0;
        let mut close = false;
        let used: Vec<(Color, Option<String>)> = Color::ALL
            .into_iter()
            .filter(|c| {
                self.doc
                    .board
                    .appendix
                    .entries
                    .iter()
                    .any(|e| e.color == Some(*c))
            })
            .map(|c| (c, self.doc.board.appendix.color_name(c).map(str::to_string)))
            .collect();
        egui::Area::new(egui::Id::unique("skiss-search"))
            .fixed_pos(screen.center_top() + vec2(0.0, 16.0))
            .pivot(Align2::CENTER_TOP)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(pc.bg)
                    .stroke(Stroke::new(
                        if theme.dark { 0.0 } else { 1.5 },
                        theme.card_stroke,
                    ))
                    .corner_radius(12.0)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut s.query)
                                    .hint_text("Search the board")
                                    .desired_width(300.0)
                                    .text_color(pc.text)
                                    .background_color(pc.field)
                                    .font(FontId::proportional(16.0)),
                            );
                            if s.focus {
                                r.request_focus();
                                s.focus = false;
                            }
                            if r.changed() {
                                changed = true;
                            }
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                step = if ui.input(|i| i.modifiers.shift) {
                                    -1
                                } else {
                                    1
                                };
                                r.request_focus();
                            }
                            if ui.input(|i| i.key_pressed(Key::Escape)) {
                                close = true;
                            }
                            let n = s.results.len();
                            let label = if s.query.trim().is_empty() && s.color.is_none() {
                                String::new()
                            } else if n == 0 {
                                "no matches".to_string()
                            } else {
                                format!("{} of {n}", s.index + 1)
                            };
                            ui.label(egui::RichText::new(label).size(13.0).color(pc.muted));
                        });
                        if !used.is_empty() {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                for (c, name) in &used {
                                    let on = s.color == Some(*c);
                                    let text = name.clone().unwrap_or_else(|| capitalize(c.name()));
                                    let b = egui::Button::new(
                                        egui::RichText::new(text).size(13.0).color(theme.ink(*c)),
                                    )
                                    .fill(theme.paper(*c))
                                    .stroke(Stroke::new(if on { 2.0 } else { 0.0 }, theme.ink(*c)))
                                    .corner_radius(8.0);
                                    if ui.add(b).clicked() {
                                        s.color = if on { None } else { Some(*c) };
                                        changed = true;
                                    }
                                }
                            });
                        }
                    });
            });
        if close {
            self.search = None;
            return;
        }
        if changed {
            self.run_search(screen);
        } else if step != 0
            && let Some(s) = &mut self.search
            && !s.results.is_empty()
        {
            let n = s.results.len() as i32;
            s.index = ((s.index as i32 + step).rem_euclid(n)) as usize;
            self.fly_to_hit(screen);
        }
    }

    fn about_ui(&mut self, ui: &mut Ui, screen: Rect, theme: &Theme) {
        let Some(id) = self.about.clone() else { return };
        let board = &self.doc.board;
        let Some(node) = board.node(&id) else {
            self.about = None;
            return;
        };
        let title_of = |x: &str| -> String {
            board
                .node(x)
                .map(|n| match n.heading() {
                    Some(h) if !h.text.is_empty() => h.text.clone(),
                    _ => n.key().to_string(),
                })
                .unwrap_or_else(|| x.to_string())
        };
        let name = title_of(&id);
        let r = query::related(board, &id);
        let pc = PromptColors::of(theme);
        let mut go: Option<String> = None;
        let mut close = false;
        let _ = node;
        egui::Area::new(egui::Id::unique("skiss-about"))
            .fixed_pos(screen.right_top() + vec2(-16.0, 16.0))
            .pivot(Align2::RIGHT_TOP)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(pc.bg)
                    .stroke(Stroke::new(
                        if theme.dark { 0.0 } else { 1.5 },
                        theme.card_stroke,
                    ))
                    .corner_radius(12.0)
                    .inner_margin(egui::Margin::same(14))
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 18,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    })
                    .show(ui, |ui| {
                        ui.set_width(300.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&name)
                                    .size(19.0)
                                    .color(pc.text)
                                    .strong(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("×").size(16.0).color(pc.muted),
                                            )
                                            .frame(false),
                                        )
                                        .clicked()
                                    {
                                        close = true;
                                    }
                                },
                            );
                        });
                        if r.is_empty() {
                            ui.label(
                                egui::RichText::new("Nothing links here yet.").color(pc.muted),
                            );
                        }
                        let mut row = |ui: &mut Ui, text: String, target: &str| {
                            let b = egui::Button::new(
                                egui::RichText::new(text).size(14.0).color(pc.text),
                            )
                            .frame(false)
                            .wrap();
                            if ui.add(b).on_hover_text("Go there").clicked() {
                                go = Some(target.to_string());
                            }
                        };
                        if !r.outgoing.is_empty() {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Connections")
                                    .size(12.0)
                                    .color(pc.muted),
                            );
                            for l in &r.outgoing {
                                let note = if l.note.is_empty() {
                                    String::new()
                                } else {
                                    format!(" · {}", l.note)
                                };
                                row(
                                    ui,
                                    format!(
                                        "→ {} {}{note}",
                                        lowercase_first(&l.label),
                                        title_of(&l.other)
                                    ),
                                    &l.other,
                                );
                            }
                        }
                        if !r.incoming.is_empty() {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Connected from")
                                    .size(12.0)
                                    .color(pc.muted),
                            );
                            for l in &r.incoming {
                                let note = if l.note.is_empty() {
                                    String::new()
                                } else {
                                    format!(" · {}", l.note)
                                };
                                row(
                                    ui,
                                    format!(
                                        "← {} {} {name}{note}",
                                        title_of(&l.other),
                                        lowercase_first(&l.label)
                                    ),
                                    &l.other,
                                );
                            }
                        }
                        if !r.mentions.is_empty() {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Mentioned in")
                                    .size(12.0)
                                    .color(pc.muted),
                            );
                            for m in &r.mentions {
                                row(ui, format!("· {}", title_of(m)), m);
                            }
                        }
                    });
            });
        if close {
            self.about = None;
        }
        if let Some(t) = go
            && let Some(rect) = self.motion.rect(&t)
        {
            self.camera.fly_to(screen, rect.expand(140.0), 60.0, 1.3);
            self.selection = vec![t];
        }
    }

    /// The card's title and body as Markdown, without its connection sentences.
    fn card_markdown(&self, id: &str) -> Option<(usize, Option<String>, String)> {
        let card = self.doc.board.card(id)?;
        let body = edit::lines_to_markdown(&self.doc.source, &card.lines, &card.lines);
        Some((
            card.range.start,
            card.heading.as_ref().map(|h| h.text.clone()),
            body,
        ))
    }

    fn start_rewrite(&mut self, ctx: &egui::Context, id: &str, how: ai::Rewrite) {
        if !ai::available() {
            self.toast = Some((
                "AI is not set up: run `skiss ai config` in a terminal".into(),
                self.last_time.unwrap_or(0.0),
            ));
            return;
        }
        let Some((start, title, body)) = self.card_markdown(id) else {
            return;
        };
        if self.ai_jobs.iter().any(|j| j.card_id == id) {
            return;
        }
        self.ai_jobs
            .push(ai::rewrite(ctx, how, start, id, title, body));
    }

    fn start_picture(
        &mut self,
        ctx: &egui::Context,
        id: &str,
        prompt: String,
        replace: Option<std::ops::Range<usize>>,
    ) {
        if !ai::available() {
            self.toast = Some((
                "AI is not set up: run `skiss ai config` in a terminal".into(),
                self.last_time.unwrap_or(0.0),
            ));
            return;
        }
        let Some(card) = self.doc.board.card(id) else {
            return;
        };
        let start = card.range.start;
        self.ai_jobs
            .push(ai::picture(ctx, prompt, start, id, replace));
    }

    /// Applies finished AI requests as ordinary edits.
    fn poll_ai(&mut self, ui: &Ui) {
        let mut done = Vec::new();
        for (i, job) in self.ai_jobs.iter().enumerate() {
            if let Some(out) = job.poll() {
                done.push((i, out));
            }
        }
        for (i, out) in done.into_iter().rev() {
            let job = self.ai_jobs.remove(i);
            let now = ui.input(|x| x.time);
            let out = match out {
                Ok(o) => o,
                Err(e) => {
                    self.toast = Some((format!("AI: {e}"), now));
                    continue;
                }
            };
            // The card may have moved in the file since; find it again.
            let id = edit::node_at(&self.doc.board, job.card_start)
                .map(|n| n.id().to_string())
                .filter(|_| true)
                .or_else(|| self.doc.board.card(&job.card_id).map(|c| c.id.clone()));
            let Some(id) = id else { continue };
            match out {
                ai::Outcome::Text { title, body } => {
                    let mut lines = skiss_core::board::model::body_lines(&body);
                    // Line by line, the rewrite stands where the old line stood (keeps spacing).
                    if let Some(card) = self.doc.board.card(&id) {
                        for (new, old) in lines.iter_mut().zip(&card.lines) {
                            if new.kind == old.kind && new.depth == old.depth {
                                new.orig = old.orig.clone();
                            }
                        }
                    }
                    let keep_title = self
                        .doc
                        .board
                        .card(&id)
                        .and_then(|c| c.heading.as_ref())
                        .is_some();
                    let title = if keep_title { title } else { None };
                    if let Some(Edited { source, .. }) = edit::set_card_text(
                        &self.doc.board,
                        &self.doc.source,
                        &id,
                        title.as_deref(),
                        &lines,
                    ) {
                        self.doc.break_coalescing();
                        self.doc.apply(source, &self.selection, None);
                        self.toast = Some(("Done. ⌘Z undoes it".into(), now));
                    } else {
                        self.toast = Some(("Nothing to change".into(), now));
                    }
                }
                ai::Outcome::Image { bytes, ext } => {
                    let rel = match ai::save_picture(
                        &self.doc.assets_dir(),
                        &self.doc.dir(),
                        &bytes,
                        ext,
                    ) {
                        Ok(r) => r,
                        Err(e) => {
                            self.toast = Some((format!("Could not save the picture: {e}"), now));
                            continue;
                        }
                    };
                    let alt = job
                        .prompt
                        .clone()
                        .unwrap_or_default()
                        .replace(['[', ']', '\n'], " ");
                    let md = format!("![{}]({rel})", alt.trim());
                    let result = match &job.replace {
                        Some(r) => Some(edit::replace_range(&self.doc.source, r.clone(), &md)),
                        None => match self.doc.board.card(&id) {
                            Some(c) if c.kind == CardKind::Section => {
                                edit::append_to_card(&self.doc.board, &self.doc.source, &id, &md)
                                    .map(|e| e.source)
                            }
                            _ => {
                                let area = self
                                    .doc
                                    .board
                                    .parent_area(&id)
                                    .map(|a| a.id.clone())
                                    .unwrap_or_default();
                                edit::insert_loose(&self.doc.board, &self.doc.source, &area, &md)
                                    .map(|e| e.source)
                            }
                        },
                    };
                    if let Some(src) = result {
                        self.doc.break_coalescing();
                        self.doc.apply(src, &self.selection, None);
                        self.images.forget_all();
                    }
                }
            }
        }
    }

    fn ai_menu_ui(&mut self, ui: &mut Ui, theme: &Theme) {
        let Some(at) = self.ai_menu else { return };
        let Some(id) = self
            .selection
            .first()
            .cloned()
            .filter(|_| self.selection.len() == 1)
        else {
            self.ai_menu = None;
            return;
        };
        let Some(card) = self.doc.board.card(&id) else {
            self.ai_menu = None;
            return;
        };
        let image_line = card.lines.iter().find_map(|l| match &l.kind {
            skiss_core::board::LineKind::Image { alt, .. } => Some((alt.clone(), l.orig.clone())),
            _ => None,
        });
        let text_prompt = {
            let mut t = card.title().unwrap_or("").to_string();
            for l in &card.lines {
                if !l.kind.is_atom() {
                    t.push_str(". ");
                    t.push_str(&l.text());
                }
            }
            t.trim_matches(['.', ' ']).to_string()
        };
        let pc = PromptColors::of(theme);
        let available = ai::available();
        let mut pick: Option<&str> = None;
        let mut targets: Vec<(String, Rect)> = Vec::new();
        let shown = egui::Area::new(egui::Id::unique("skiss-ai-menu"))
            .fixed_pos(at)
            .pivot(Align2::CENTER_TOP)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(pc.bg)
                    .stroke(Stroke::new(
                        if theme.dark { 0.0 } else { 1.5 },
                        theme.card_stroke,
                    ))
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::same(8))
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    })
                    .show(ui, |ui| {
                        ui.set_width(220.0);
                        let mut item = |ui: &mut Ui, label: &str, key: &'static str| {
                            let b = egui::Button::new(
                                egui::RichText::new(label).size(14.0).color(pc.text),
                            )
                            .frame(false)
                            .min_size(vec2(210.0, 24.0));
                            let r = ui.add(b);
                            targets.push((format!("ai-{key}"), r.rect));
                            if r.clicked() {
                                pick = Some(key);
                            }
                        };
                        if !available {
                            ui.label(
                                egui::RichText::new("AI is not set up for skiss yet.")
                                    .color(pc.text),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "Run `skiss ai config` (and `skiss ai enable`) in a terminal.",
                                )
                                .size(12.5)
                                .color(pc.muted),
                            );
                            return;
                        }
                        item(ui, "Fix spelling", "spell");
                        item(ui, "Make it clearer", "clear");
                        item(ui, "Shorten", "short");
                        ui.separator();
                        item(ui, "Translate to English", "en");
                        item(ui, "Translate to Swedish", "sv");
                        item(ui, "Translate to…", "other");
                        ui.separator();
                        item(ui, "Picture from this card", "pic");
                        if image_line.is_some() {
                            item(ui, "Redraw the picture", "redraw");
                        }
                    });
            });
        self.dock_buttons.retain(|(k, _)| !k.starts_with("ai-"));
        self.dock_buttons.extend(targets);
        let outside = ui.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !shown.response.rect.expand(4.0).contains(p))
        });
        let Some(key) = pick else {
            if outside {
                self.ai_menu = None;
            }
            return;
        };
        self.ai_menu = None;
        let ctx = ui.ctx().clone();
        let anchor = at + vec2(-130.0, 10.0);
        match key {
            "spell" => self.start_rewrite(&ctx, &id, ai::Rewrite::Spelling),
            "clear" => self.start_rewrite(&ctx, &id, ai::Rewrite::Clearer),
            "short" => self.start_rewrite(&ctx, &id, ai::Rewrite::Shorter),
            "en" => self.start_rewrite(&ctx, &id, ai::Rewrite::Translate("English".into())),
            "sv" => self.start_rewrite(&ctx, &id, ai::Rewrite::Translate("Swedish".into())),
            "other" => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::TranslateTo { card: id },
                    text: String::new(),
                    anchor,
                    focus: true,
                })
            }
            "pic" => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::Picture {
                        card: id,
                        replace: None,
                    },
                    text: text_prompt,
                    anchor,
                    focus: true,
                })
            }
            "redraw" => {
                if let Some((alt, range)) = image_line {
                    self.prompt = Some(Prompt {
                        kind: PromptKind::Picture {
                            card: id,
                            replace: Some(range),
                        },
                        text: alt,
                        anchor,
                        focus: true,
                    })
                }
            }
            _ => {}
        }
    }

    /// The dock at the bottom: the main things you can make, always in reach.
    fn dock(&mut self, ui: &mut Ui, screen: Rect, theme: &Theme) -> Option<UiRequest> {
        let mut act: Option<&str> = None;
        let mut rects: Vec<(String, Rect)> = Vec::new();
        let can_group = !self.selection.is_empty();
        let shown = egui::Area::new(egui::Id::unique("skiss-dock"))
            .fixed_pos(screen.center_bottom() - vec2(0.0, 14.0))
            .pivot(Align2::CENTER_BOTTOM)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme.ui_bg.gamma_multiply(0.94))
                    .corner_radius(12.0)
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .shadow(egui::Shadow {
                        offset: [0, 3],
                        blur: 14,
                        spread: 0,
                        color: Color32::from_black_alpha(55),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let mut b =
                                |ui: &mut Ui,
                                 label: &str,
                                 tip: &str,
                                 on: bool,
                                 key: &'static str| {
                                    let text = egui::RichText::new(label).size(14.0).color(if on {
                                        theme.ui_text
                                    } else {
                                        theme.ui_text.gamma_multiply(0.35)
                                    });
                                    let r = ui.add_enabled(
                                        on,
                                        egui::Button::new(text)
                                            .frame(false)
                                            .min_size(vec2(30.0, 26.0)),
                                    );
                                    rects.push((key.to_string(), r.rect));
                                    if r.on_hover_text(keys(tip)).clicked() {
                                        act = Some(key);
                                    }
                                };
                            b(
                                ui,
                                "+ Card",
                                "New card (double-click empty space, or Tab)",
                                true,
                                "card",
                            );
                            b(
                                ui,
                                "+ Area",
                                "New area (or select cards and press ⌘G)",
                                true,
                                "area",
                            );
                            b(
                                ui,
                                "Group",
                                "Put the selected cards in a new area (⌘G)",
                                can_group,
                                "group",
                            );
                            ui.separator();
                            b(
                                ui,
                                "Tidy up",
                                "Lay everything out again (⌘⇧L)",
                                true,
                                "tidy",
                            );
                            ui.separator();
                            let name = if theme.dark { "Ember" } else { "Sketchbook" };
                            b(ui, name, "Switch theme (⌘T)", true, "theme");
                            b(ui, "?", "Shortcuts", true, "help");
                        });
                    });
            });
        self.dock_buttons.retain(|(k, _)| k.starts_with("ai-"));
        self.dock_buttons.extend(rects);
        let now = ui.input(|i| i.time);
        let anchor = shown.response.rect.center_top() + vec2(-130.0, -96.0);
        match act {
            Some("card") => {
                let area = self.target_area();
                self.new_card_at(screen, &area, None, screen.center(), now);
            }
            Some("area") => {
                let parent = self.target_area();
                self.prompt = Some(Prompt {
                    kind: PromptKind::NewArea { parent },
                    text: String::new(),
                    anchor,
                    focus: true,
                });
            }
            Some("group") => self.group_selected(screen),
            Some("tidy") => self.tidy_up(),
            Some("theme") => return Some(UiRequest::ToggleTheme),
            Some("help") => self.help = !self.help,
            _ => {}
        }
        None
    }

    fn set_color(&mut self, c: Color) {
        self.commit_editor(true);
        let keys: Vec<String> = self
            .selection
            .iter()
            .filter_map(|id| self.doc.board.node(id).map(|n| n.key().to_string()))
            .collect();
        let all_same = keys
            .iter()
            .all(|k| self.doc.board.appendix.get(k).and_then(|e| e.color) == Some(c));
        let src = edit::with_appendix(&self.doc.board, &self.doc.source, |a| {
            for k in &keys {
                a.entry_mut(k).color = if all_same { None } else { Some(c) };
            }
        });
        self.doc.apply(src, &self.selection, None);
    }

    fn prompt_ui(&mut self, ui: &mut Ui, theme: &Theme) {
        let Some(prompt) = &mut self.prompt else {
            return;
        };
        let mut done: Option<bool> = None;
        let suggestions: Vec<String> =
            if let PromptKind::Label { .. } | PromptKind::Relabel { .. } = prompt.kind {
                let mut s: Vec<String> = Vec::new();
                for (_, _, c) in self.doc.board.connections() {
                    let l = lowercase_first(&c.label);
                    if !l.is_empty() && !s.contains(&l) {
                        s.push(l);
                    }
                }
                for d in [
                    "manages",
                    "reports to",
                    "depends on",
                    "told me to contact",
                    "works with",
                ] {
                    if !s.iter().any(|x| x == d) {
                        s.push(d.to_string());
                    }
                }
                s
            } else {
                Vec::new()
            };
        let title = match &prompt.kind {
            PromptKind::Label { .. } => "How are they connected?",
            PromptKind::AreaTitle { .. } => "Area title",
            PromptKind::ColorName { .. } => "What does this color mean?",
            PromptKind::Relabel { .. } => "How are they connected?",
            PromptKind::BoardTitle => "Board title",
            PromptKind::NewArea { .. } => "Name the new area",
            PromptKind::TranslateTo { .. } => "Translate into which language?",
            PromptKind::Picture { .. } => "Describe the picture",
        };
        let mut picked: Option<String> = None;
        // Prompts look like part of the theme: a paper card in Sketchbook, dark in Ember.
        let pc = PromptColors::of(theme);
        let first_frame = prompt.focus;
        let shown = egui::Area::new(egui::Id::unique("skiss-prompt"))
            .fixed_pos(prompt.anchor)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(pc.bg)
                    .stroke(Stroke::new(
                        if theme.dark { 0.0 } else { 1.5 },
                        theme.card_stroke,
                    ))
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::same(10))
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(70),
                    })
                    .show(ui, |ui| {
                        ui.set_max_width(260.0);
                        ui.label(egui::RichText::new(title).color(pc.muted).size(12.0));
                        let edit = egui::TextEdit::singleline(&mut prompt.text)
                            .desired_width(240.0)
                            .text_color(pc.text)
                            .background_color(pc.field)
                            .hint_text(match prompt.kind {
                                PromptKind::Label { .. } | PromptKind::Relabel { .. } => {
                                    "reports to"
                                }
                                PromptKind::AreaTitle { .. } | PromptKind::BoardTitle => "Untitled",
                                PromptKind::NewArea { .. } => "People",
                                PromptKind::TranslateTo { .. } => "German",
                                PromptKind::Picture { .. } => "A pencil sketch of...",
                                PromptKind::ColorName { .. } => "Blocked",
                            })
                            .font(FontId::proportional(16.0));
                        let out = edit.show(ui);
                        let r = out.response;
                        if prompt.focus {
                            // Start with the current text selected: typing replaces it.
                            r.request_focus();
                            let mut state = out.state;
                            let n = prompt.text.chars().count();
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(0),
                                    egui::text::CCursor::new(n),
                                )));
                            state.store(ui.ctx(), r.id);
                            prompt.focus = false;
                        }
                        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            done = Some(true);
                        }
                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                            done = Some(false);
                        }
                        if !suggestions.is_empty() {
                            ui.add_space(4.0);
                            let typed = prompt.text.to_lowercase();
                            ui.horizontal_wrapped(|ui| {
                                for s in suggestions
                                    .iter()
                                    .filter(|s| typed.is_empty() || s.contains(&typed))
                                    .take(6)
                                {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(s).color(pc.text).size(13.0),
                                            )
                                            .fill(pc.chip)
                                            .stroke(Stroke::NONE)
                                            .corner_radius(8.0),
                                        )
                                        .clicked()
                                    {
                                        picked = Some(s.clone());
                                    }
                                }
                            });
                        }
                    });
            });
        if let Some(p) = picked {
            prompt.text = p;
            done = Some(true);
        }
        // Clicking anywhere else confirms, like leaving any text field.
        let outside = ui.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !shown.response.rect.expand(4.0).contains(p))
        });
        if done.is_none() && outside && !first_frame {
            done = Some(true);
        }
        let Some(ok) = done else { return };
        let prompt = self.prompt.take().expect("prompt");
        if !ok {
            return;
        }
        match prompt.kind {
            PromptKind::Label { from, to } => {
                if let Some(src) = edit::add_connection(
                    &self.doc.board,
                    &self.doc.source,
                    &from,
                    &to,
                    &prompt.text,
                ) {
                    self.doc.apply(src, &self.selection, None);
                    self.selection = vec![from];
                } else {
                    self.toast = Some((
                        "They are already connected".into(),
                        self.last_time.unwrap_or(0.0),
                    ));
                }
            }
            PromptKind::AreaTitle { start, .. } => {
                if let Some(n) = edit::node_at(&self.doc.board, start) {
                    let id = n.id().to_string();
                    if let Some(Edited { source, at }) =
                        edit::rename_heading(&self.doc.board, &self.doc.source, &id, &prompt.text)
                    {
                        self.doc.apply(source, &self.selection, None);
                        if let Some(n) = edit::node_at(&self.doc.board, at) {
                            let nid = n.id().to_string();
                            self.motion.rename(&id, &nid);
                            self.selection = vec![nid];
                        }
                    }
                }
            }
            PromptKind::Relabel { from, to } => {
                if let Some(src) = edit::relabel_connection(
                    &self.doc.board,
                    &self.doc.source,
                    &from,
                    &to,
                    &prompt.text,
                ) {
                    self.doc.apply(src, &self.selection, None);
                }
                self.selected_conn = Some((from, to));
            }
            PromptKind::TranslateTo { card } => {
                let lang = prompt.text.trim().to_string();
                if !lang.is_empty() {
                    self.start_rewrite(ui.ctx(), &card, ai::Rewrite::Translate(lang));
                }
            }
            PromptKind::Picture { card, replace } => {
                let p = prompt.text.trim().to_string();
                if !p.is_empty() {
                    self.start_picture(ui.ctx(), &card, p, replace);
                }
            }
            PromptKind::NewArea { parent } => {
                let name = if prompt.text.trim().is_empty() {
                    "New area"
                } else {
                    prompt.text.trim()
                };
                if let Some(Edited { source, at }) =
                    edit::insert_area(&self.doc.board, &self.doc.source, &parent, name, "New card")
                {
                    self.doc.break_coalescing();
                    self.doc
                        .apply(source, &self.selection, Some(&format!("edit:{at}")));
                    self.motion.spawn_from = Some(self.camera.center);
                    if let Some(n) = edit::node_at(&self.doc.board, at) {
                        let id = n.id().to_string();
                        self.selection = vec![id.clone()];
                        self.start_editing(&id, None, true, self.last_time.unwrap_or(0.0));
                        // "New card" is selected: typing replaces it.
                        if let Some(ed) = &mut self.editor {
                            let len = ed.lines[0].chars.len();
                            ed.set_cursor(Pos { line: 0, col: 0 }, false);
                            ed.set_cursor(Pos { line: 0, col: len }, true);
                        }
                    }
                }
            }
            PromptKind::BoardTitle => {
                if let Some(src) =
                    edit::rename_title(&self.doc.board, &self.doc.source, &prompt.text)
                {
                    self.doc.apply(src, &self.selection, None);
                }
            }
            PromptKind::ColorName { color } => {
                let src = edit::with_appendix(&self.doc.board, &self.doc.source, |a| {
                    a.set_color_name(color, &prompt.text)
                });
                self.doc.apply(src, &self.selection, None);
            }
        }
    }

    fn status(&mut self, ui: &mut Ui, screen: Rect, theme: &Theme, now: f64) {
        let p = ui.painter();
        let font = FontId::proportional(12.5);
        let c = theme.muted;
        let saved = if self.doc.error.is_some() {
            "not saved"
        } else {
            "saved"
        };
        let left = format!("{}  ·  {saved}", self.doc.file_name());
        p.text(
            screen.left_bottom() + vec2(14.0, -12.0),
            Align2::LEFT_BOTTOM,
            left,
            font.clone(),
            c,
        );
        let right = format!("{:.0}%   ·   ? for shortcuts", self.camera.zoom * 100.0);
        p.text(
            screen.right_bottom() + vec2(-14.0, -12.0),
            Align2::RIGHT_BOTTOM,
            right,
            font,
            c,
        );
        if let Some(err) = &self.doc.error {
            p.text(
                screen.center_bottom() + vec2(0.0, -34.0),
                Align2::CENTER_BOTTOM,
                err,
                FontId::proportional(13.0),
                theme.accent,
            );
        }
        if let Some((msg, t)) = &self.toast {
            let a = (1.0 - ((now - t) as f32 - 1.6).max(0.0) / 0.6).clamp(0.0, 1.0);
            let g = p.layout_no_wrap(
                msg.clone(),
                FontId::proportional(14.0),
                theme.ui_text.gamma_multiply(a),
            );
            let r = Rect::from_center_size(
                screen.center_top() + vec2(0.0, 34.0),
                g.size() + vec2(24.0, 12.0),
            );
            p.rect_filled(r, r.height() / 2.0, theme.ui_bg.gamma_multiply(0.92 * a));
            p.galley(r.min + vec2(12.0, 6.0), g, theme.ui_text);
        }
        if self.doc.board.walk().len() <= 1
            && self.doc.board.title.is_some()
            && self.editor.is_none()
        {
            p.text(
                screen.center() + vec2(0.0, 150.0),
                Align2::CENTER_CENTER,
                "Double-click anywhere to add your first card",
                FontId::new(theme.body_size * 1.1, theme.body.clone()),
                theme.muted,
            );
        }
    }

    fn help_overlay(&mut self, ui: &mut Ui, screen: Rect, theme: &Theme) {
        let rows = [
            ("Double-click", "new card / edit a card"),
            ("Enter", "edit the selected card"),
            ("Tab", "add the next card"),
            ("Esc", "stop editing, clear selection"),
            ("Esc Esc", "close skiss"),
            ("Drag a card", "move it (into another area too)"),
            ("Drag a dot on the edge", "connect two cards"),
            ("Shift-drag", "select several"),
            ("⌘F", "search (Enter for the next hit)"),
            ("R", "everything about the selected card"),
            ("A", "AI: spelling, rewriting, translating, pictures"),
            ("⌘G", "group into a new area"),
            (
                "Drag the corner",
                "resize (double-click it: automatic size)",
            ),
            ("⌘⇧L", "tidy up the layout"),
            ("⌘B  ⌘I  ⌘E", "bold, italic, code"),
            ("⇧Enter", "line break inside a paragraph"),
            ("⌘L or [ ]", "turn a line into a task"),
            ("⌘Z  ⌘Y", "undo, redo"),
            ("Scroll / pinch", "pan and zoom"),
            ("F  or  ⌘0", "fit everything"),
            ("⌘⇧+  ⌘⇧−", "bigger text for sharing"),
            ("1 – 7, 0", "paper color, none"),
            ("⌘T", "switch theme"),
            ("⌘V", "paste text or an image"),
        ];
        egui::Area::new(egui::Id::unique("skiss-help"))
            .fixed_pos(screen.center())
            .pivot(Align2::CENTER_CENTER)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme.ui_bg)
                    .corner_radius(14.0)
                    .inner_margin(egui::Margin::same(22))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("skiss shortcuts")
                                .size(20.0)
                                .color(theme.ui_text),
                        );
                        ui.add_space(8.0);
                        egui::Grid::new("help-grid")
                            .spacing(vec2(24.0, 6.0))
                            .show(ui, |ui| {
                                for (k, v) in rows {
                                    ui.label(
                                        egui::RichText::new(keys(k))
                                            .monospace()
                                            .color(theme.accent),
                                    );
                                    ui.label(egui::RichText::new(v).color(theme.ui_text));
                                    ui.end_row();
                                }
                            });
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Esc to close")
                                .size(12.0)
                                .color(theme.ui_text.gamma_multiply(0.6)),
                        );
                    });
            });
    }
}

enum ToolbarAction {
    Bold,
    Italic,
    Strike,
    Task,
    Color(Color),
    NameColor(Color, Pos2),
    Edit,
    Delete,
    Related,
    Ai,
}

pub enum UiRequest {
    ToggleTheme,
}

/// Font size steps (about 4% apart) so zooming does not rasterize glyphs at every size.
fn quantize(zoom: f32) -> f32 {
    let steps = 24.0;
    (2f32).powf((zoom.log2() * steps).round() / steps)
}

fn title_spans(spans: &[Span]) -> Vec<Span> {
    spans.to_vec()
}

fn lowercase_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => {
            // Keep acronyms ("API owner") as they are.
            if c.clone().next().is_some_and(|n| n.is_uppercase()) {
                s.to_string()
            } else {
                f.to_lowercase().collect::<String>() + c.as_str()
            }
        }
        None => String::new(),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Shortcut labels as this platform writes them (⌘ on macOS, Ctrl elsewhere).
fn keys(s: &str) -> String {
    if cfg!(target_os = "macos") {
        s.to_string()
    } else {
        s.replace('⌘', "Ctrl+").replace('⇧', "Shift+")
    }
}

/// Distance from a point to a polyline.
fn distance_to(pts: &[Pos2], p: Pos2) -> f32 {
    pts.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
            (a + ab * t - p).length()
        })
        .fold(f32::INFINITY, f32::min)
}

/// Colors of the small prompts (connection labels, titles, color names).
struct PromptColors {
    bg: Color32,
    text: Color32,
    muted: Color32,
    field: Color32,
    chip: Color32,
}

impl PromptColors {
    fn of(theme: &Theme) -> Self {
        if theme.dark {
            PromptColors {
                bg: theme.ui_bg,
                text: theme.ui_text,
                muted: theme.ui_text.gamma_multiply(0.65),
                field: Color32::from_white_alpha(16),
                chip: Color32::from_white_alpha(22),
            }
        } else {
            PromptColors {
                bg: theme.card_fill,
                text: theme.text,
                muted: theme.muted,
                field: Color32::WHITE,
                chip: Color32::from_rgba_unmultiplied(60, 40, 20, 20),
            }
        }
    }
}

/// Picks which sides of two cards an arrow leaves and enters: the pair whose curve crosses the
/// fewest other cards, then the shortest.
fn best_route(
    a: Rect,
    b: Rect,
    obstacles: &[(String, Rect)],
    frame: Rect,
    from: &str,
    to: &str,
) -> ((u8, u8), f32) {
    let natural = (draw::side_of(a, b.center()), draw::side_of(b, a.center()));
    let others: Vec<Rect> = obstacles
        .iter()
        .filter(|(id, _)| id != from && id != to)
        .map(|(_, r)| *r)
        .filter(|r| r.intersects(a.union(b).expand(220.0)))
        .collect();
    let unit = (a.width() + b.width()) * 0.5;
    // Swinging out past the edge of the board looks lost, and is cut off when it all fits.
    let frame = frame.expand(unit * 0.3);
    let score = |sa: u8, sb: u8, bend: f32| -> f32 {
        let (p0, n0) = draw::port(a, sa, 0.5);
        let (p3, n3) = draw::port(b, sb, 0.5);
        let (pts, _, _) = draw::curve_with(p0, n0, p3, n3, bend);
        // Running back through its own cards counts as crossing too.
        let own = [a.shrink(6.0), b.shrink(6.0)];
        // Leaving a card on the side that faces away from the other end makes a loop.
        let away = |p: Pos2, n: Vec2, target: Pos2| (target - p).dot(n) < -unit * 0.25;
        let loops = away(p0, n0, p3) as u8 + away(p3, n3, p0) as u8;
        draw::route_score(&pts, &others, unit)
            + draw::crossings(&pts, &own) as f32 * unit * 0.3
            + loops as f32 * unit * 2.5
            + pts.iter().filter(|p| !frame.contains(**p)).count() as f32 * unit * 0.1
    };
    let mut best = ((natural, 1.0), score(natural.0, natural.1, 1.0));
    // Every pair of sides, each with a plain and two wider bends: the line that passes under
    // no card, without a long detour, wins.
    for sa in 0..4u8 {
        for sb in 0..4u8 {
            for bend in [1.0, 1.6] {
                let sc = score(sa, sb, bend);
                if sc < best.1 - 0.5 {
                    best = (((sa, sb), bend), sc);
                }
            }
        }
    }
    best.0
}

/// Whether the map labels have taken over from the area titles (1 far out, 0 near). It is a
/// clean switch, so an area never shows its name twice.
fn map_fade(zoom: f32) -> f32 {
    if zoom < 0.45 { 1.0 } else { 0.0 }
}
