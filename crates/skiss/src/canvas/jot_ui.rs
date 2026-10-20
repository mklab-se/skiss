//! The jot on screen: a large editor over the board, and putting the note in its place.
//!
//! The jot is the ordinary card editor working on a draft card that is not in the file yet, so
//! everything that works in a card works here too (formatting, links, lists, tasks, pasted
//! pictures). It is drawn as a big card in the middle of the screen, over a dimmed board.

use super::*;
use crate::canvas::jot::{self, JOT_ID, Outcome, Request, Target};
use crate::canvas::motion::Anim;
use skiss_core::board::inline;
use skiss_core::board::place;

/// The draft and what is happening to it.
pub(super) struct JotState {
    pub card: Card,
    pub job: Option<jot::Job>,
    /// Places to choose from when the judge was unsure.
    pub choices: Vec<jot::Choice>,
    /// The note being placed (Markdown), kept while choosing.
    pub note: String,
    /// Esc was pressed once on a note with text: the next Esc throws it away.
    pub warned: bool,
    /// How far the text is scrolled (world units), and how tall it is.
    pub scroll: f32,
    pub content_h: f32,
    /// The `.` that opened the jot must not land in it.
    pub swallow_dot: bool,
    pub panel: Rect,
}

/// Text in the jot is this much bigger than on a card at 100%.
const JOT_ZOOM: f32 = 1.3;

impl BoardView {
    /// Opens an empty jot.
    pub(super) fn open_jot(&mut self, now: f64) {
        if self.jot.is_some() {
            return;
        }
        if self.editor.is_some() {
            self.finish_editing();
        }
        self.search = None;
        self.ai_menu = None;
        self.prompt = None;
        if let Some(card) = draft_card("###") {
            self.start_jot_editor(card, now);
        }
        if let Some(j) = &mut self.jot {
            j.swallow_dot = true;
        }
    }

    fn start_jot_editor(&mut self, card: Card, now: f64) {
        let mut ed = Editor::new(&card);
        ed.card_id = JOT_ID.to_string();
        ed.card_start = usize::MAX;
        let end = ed.end();
        ed.set_cursor(end, false);
        self.editor = Some(ed);
        self.edit_text = None;
        self.blink_start = now;
        self.selection.clear();
        match &mut self.jot {
            Some(j) => {
                j.card = card;
                j.scroll = 0.0;
            }
            None => {
                self.jot = Some(JotState {
                    card,
                    job: None,
                    choices: Vec::new(),
                    note: String::new(),
                    warned: false,
                    scroll: 0.0,
                    content_h: 0.0,
                    swallow_dot: false,
                    panel: Rect::NOTHING,
                })
            }
        }
    }

    pub(super) fn close_jot(&mut self) {
        self.jot = None;
        if self.editor.as_ref().is_some_and(|e| e.card_id == JOT_ID) {
            self.editor = None;
        }
        self.edit_text = None;
    }

    /// Esc: back from choosing or waiting; on a written note, a second Esc throws it away.
    pub(super) fn jot_escape(&mut self, now: f64) {
        let Some(j) = &mut self.jot else { return };
        if !j.choices.is_empty() || j.job.is_some() {
            j.choices.clear();
            j.job = None;
            return;
        }
        let empty = self.editor.as_ref().is_none_or(|e| e.is_empty());
        if empty || j.warned {
            self.close_jot();
        } else {
            j.warned = true;
            self.toast = Some(("Press Esc again to throw this note away".into(), now));
        }
    }

    /// The note as Markdown: a card with a title.
    fn jot_markdown(&self) -> Option<String> {
        let ed = self.editor.as_ref()?;
        let title = ed
            .lines
            .first()
            .filter(|l| l.title)
            .map(|l| inline::to_markdown(&l.spans()))
            .unwrap_or_default();
        // The draft is new text: nothing in it is copied from a file.
        let mut lines = ed.body_lines();
        for l in &mut lines {
            l.orig = 0..0;
        }
        let body = edit::lines_to_markdown("", &lines, &[]);
        let title = title.trim();
        if title.is_empty() && body.trim().is_empty() {
            return None;
        }
        let md = if title.chars().count() > 70 || title.is_empty() {
            place::note_card(&format!("{title}\n{body}"))
        } else {
            format!("# {title}\n\n{}", body.trim())
        };
        Some(jot::tidy_tables(md.trim()))
    }

    fn jot_board(&self) -> jot::BoardView {
        let board = &self.doc.board;
        let src = &self.doc.source;
        let spots = place::spots(board);
        let mut examples = std::collections::HashMap::new();
        let mut checklists = std::collections::HashMap::new();
        for s in &spots {
            examples.insert(s.id.clone(), place::examples(board, src, &s.id, 3));
            let lists: Vec<(String, String)> = place::checklists(board, &s.id)
                .into_iter()
                .map(|c| {
                    let t = c
                        .heading
                        .as_ref()
                        .map(|h| h.text.clone())
                        .unwrap_or_else(|| "Checklist".into());
                    (c.id.clone(), t)
                })
                .collect();
            checklists.insert(s.id.clone(), lists);
        }
        jot::BoardView {
            title: board
                .title
                .as_ref()
                .map(|t| t.text.clone())
                .unwrap_or_default(),
            spots,
            examples,
            checklists,
            today: chrono::Local::now().format("%Y-%m-%d").to_string(),
            document: src[..board.content_end().min(src.len())].to_string(),
        }
    }

    /// Enter: ask AI (a `/` request), or put the note in its place.
    pub(super) fn submit_jot(&mut self, ctx: &egui::Context, now: f64) {
        if self.jot.as_ref().is_none_or(|j| j.job.is_some()) {
            return;
        }
        let first = self
            .editor
            .as_ref()
            .and_then(|e| e.lines.first())
            .map(|l| l.text())
            .unwrap_or_default();
        if let Some(req) = jot::request(&first) {
            if !ai::available() {
                self.toast = Some((
                    "AI is not set up: run `skiss ai config` in a terminal".into(),
                    now,
                ));
                return;
            }
            let job = match req {
                Request::Write(r) => jot::draft(ctx, r, self.jot_board()),
                Request::Picture(p) => jot::picture(ctx, p),
            };
            if let Some(j) = &mut self.jot {
                j.job = Some(job);
            }
            return;
        }
        let Some(md) = self.jot_markdown() else {
            self.close_jot();
            return;
        };
        if !ai::available() {
            self.place_jot(Target::Area(String::new()), &md, now);
            return;
        }
        let board = self.jot_board();
        if let Some(j) = &mut self.jot {
            j.note = md.clone();
            j.warned = false;
            j.job = Some(jot::place(ctx, md, board, None));
        }
    }

    /// Picks one of the offered places (1, 2, 3, or a click).
    pub(super) fn choose_jot(&mut self, ctx: &egui::Context, i: usize) {
        let board = self.jot_board();
        let Some(j) = &mut self.jot else { return };
        let Some(choice) = j.choices.get(i).cloned() else {
            return;
        };
        j.choices.clear();
        j.job = Some(jot::place(ctx, j.note.clone(), board, Some(choice.target)));
    }

    /// Writes the note into the file. `Target::Area("")` without AI means the Inbox.
    fn place_jot(&mut self, target: Target, md: &str, now: f64) {
        let board = &self.doc.board;
        let src = &self.doc.source;
        let (edited, label) = match &target {
            Target::Area(id) if id.is_empty() && !ai::available() => {
                let inbox = place::spots(board)
                    .into_iter()
                    .find(|s| s.path == place::INBOX)
                    .map(|s| s.id);
                match inbox {
                    Some(id) => (place::place_card(board, src, &id, md), place::INBOX.into()),
                    None => (
                        place::place_in_new_area(board, src, place::INBOX, md),
                        place::INBOX.into(),
                    ),
                }
            }
            Target::Area(id) => {
                let label = place::spots(board)
                    .into_iter()
                    .find(|s| &s.id == id)
                    .map_or_else(|| "the board".into(), |s| s.path);
                (place::place_card(board, src, id, md), label)
            }
            Target::Checklist(card) => {
                let label = board
                    .card(card)
                    .and_then(|c| c.heading.as_ref())
                    .map_or_else(|| "a checklist".into(), |h| h.text.clone());
                (place::add_task(board, src, card, md), label)
            }
            Target::NewArea(title) => (
                place::place_in_new_area(board, src, title, md),
                format!("a new area, {title}"),
            ),
        };
        let Some(Edited { source, at }) = edited else {
            self.toast = Some(("Could not place the note".into(), now));
            if let Some(j) = &mut self.jot {
                j.job = None;
            }
            return;
        };
        self.close_jot();
        self.doc.break_coalescing();
        self.doc.apply(source, &self.selection, None);
        self.doc.break_coalescing();
        if let Some(n) = edit::node_at(&self.doc.board, at) {
            let id = n.id().to_string();
            self.selection = vec![id.clone()];
            self.reveal_card = Some(id);
        }
        self.toast = Some((format!("Placed in {label} · ⌘Z undoes it"), now));
    }

    /// Takes in what AI sent back.
    pub(super) fn poll_jot(&mut self, now: f64) {
        let Some(out) = self
            .jot
            .as_ref()
            .and_then(|j| j.job.as_ref())
            .and_then(|job| job.poll())
        else {
            return;
        };
        let placing = self.jot.as_ref().is_some_and(|j| !j.note.is_empty());
        if let Some(j) = &mut self.jot {
            j.job = None;
        }
        match out {
            Err(e) if placing => {
                // The note is never lost: without an answer it goes to the Inbox.
                let note = self
                    .jot
                    .as_ref()
                    .map(|j| j.note.clone())
                    .unwrap_or_default();
                self.toast = Some((format!("AI: {e}"), now));
                let inbox = place::spots(&self.doc.board)
                    .into_iter()
                    .find(|s| s.path == place::INBOX)
                    .map(|s| s.id);
                match inbox {
                    Some(id) => self.place_jot(Target::Area(id), &note, now),
                    None => self.place_jot(Target::NewArea(place::INBOX.into()), &note, now),
                }
            }
            Err(e) => self.toast = Some((format!("AI: {e}"), now)),
            Ok(Outcome::Place { target, markdown }) => self.place_jot(target, &markdown, now),
            Ok(Outcome::Choose(choices)) => {
                if let Some(j) = &mut self.jot {
                    j.choices = choices;
                }
            }
            Ok(Outcome::Draft(md)) => self.load_jot_draft(&jot::tidy_tables(&md), now),
            Ok(Outcome::Picture { bytes, ext, prompt }) => {
                match ai::save_picture(&self.doc.assets_dir(), &self.doc.dir(), &bytes, ext) {
                    Ok(rel) => {
                        let alt = prompt.replace(['[', ']', '\n'], " ");
                        let title = capitalize_first(alt.trim());
                        self.load_jot_draft(&format!("# {title}\n\n![{}]({rel})", alt.trim()), now);
                        self.images.forget_all();
                    }
                    Err(e) => {
                        self.toast = Some((format!("Could not save the picture: {e}"), now));
                    }
                }
            }
        }
        if let Some(j) = &mut self.jot {
            j.note.clear();
        }
    }

    /// Puts AI's draft in the jot, ready to be read, changed and placed.
    fn load_jot_draft(&mut self, md: &str, now: f64) {
        // One card: the first heading is its title, later headings become bold lines.
        let mut out = String::new();
        let mut titled = false;
        for line in md.trim().lines() {
            let t = line.trim_start();
            if t.starts_with('#') {
                let text = t.trim_start_matches('#').trim();
                if titled {
                    out.push_str(&format!("**{text}**\n"));
                } else {
                    out.push_str(&format!("### {text}\n"));
                    titled = true;
                }
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        if !titled {
            out = format!("### Note\n\n{out}");
        }
        match draft_card(&out) {
            Some(card) => self.start_jot_editor(card, now),
            None => self.toast = Some(("AI sent back nothing usable".into(), now)),
        }
    }

    /// Mouse in the jot: place the caret, select by dragging, double-click a word.
    pub(super) fn jot_pointer(&mut self, ui: &Ui) {
        let Some(j) = &self.jot else { return };
        let panel = j.panel;
        let (pressed, down, double, pos, shift) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.button_double_clicked(PointerButton::Primary),
                i.pointer.interact_pos(),
                i.modifiers.shift,
            )
        });
        let Some(at) = pos else { return };
        if panel.contains(at) {
            ui.ctx().set_cursor_icon(CursorIcon::Text);
        }
        if !(pressed || down) || !panel.contains(at) && pressed {
            return;
        }
        let Some(p) = self.editor_pos_at(at) else {
            return;
        };
        if let Some(ed) = &mut self.editor {
            if double {
                ed.select_word(p);
            } else if pressed {
                ed.set_cursor(p, shift);
            } else {
                ed.set_cursor(p, true);
            }
            self.blink_start = ui.input(|i| i.time);
        }
    }

    /// Scrolling over the jot scrolls its text.
    pub(super) fn jot_scroll(&mut self, ui: &Ui) {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if let Some(j) = &mut self.jot
            && dy != 0.0
        {
            let visible = j.panel.height() / JOT_ZOOM;
            let max = (j.content_h - visible).max(0.0);
            j.scroll = (j.scroll - dy / JOT_ZOOM).clamp(0.0, max);
        }
    }

    /// Paints the dimmed board and the jot over it.
    pub(super) fn paint_jot(
        &mut self,
        ctx: &egui::Context,
        p: &egui::Painter,
        screen: Rect,
        theme: &Theme,
        now: f64,
    ) {
        if self.jot.is_none() {
            return;
        }
        // Pictures in the draft are loaded before it is laid out, so they have their real
        // size from the first frame (otherwise they jump when they arrive).
        let dir = self.doc.dir();
        let urls: Vec<String> = self
            .editor
            .iter()
            .flat_map(|e| e.body_lines())
            .filter_map(|l| match l.kind {
                LineKind::Image { url, .. } => Some(url),
                _ => None,
            })
            .collect();
        for url in &urls {
            self.images.aspect(ctx, &dir, url);
        }
        let Some(j) = &self.jot else { return };
        // The board stays faintly visible underneath.
        p.rect_filled(screen, 0.0, theme.bg.gamma_multiply(0.8));

        let w = (screen.width() * 0.66)
            .clamp((screen.width() - 32.0).min(520.0), 1100.0)
            .min(screen.width() - 32.0);
        let top = screen.min.y + (screen.height() * 0.07).max(24.0);
        let bottom = screen.max.y - 120.0;
        let panel = Rect::from_min_max(
            pos2(screen.center().x - w / 2.0, top),
            pos2(screen.center().x + w / 2.0, bottom.max(top + 160.0)),
        );
        let visible = panel.height() / JOT_ZOOM;
        let world_h = j.content_h.max(visible);
        let world = Rect::from_min_size(pos2(0.0, 0.0), vec2(panel.width() / JOT_ZOOM, world_h));
        let scroll = j.scroll.min((world_h - visible).max(0.0));
        let center = world.min + (screen.center() - panel.min) / JOT_ZOOM + vec2(0.0, scroll);

        // Draw the draft as a card, through a camera that puts it in the panel.
        // The panel: a sheet of paper with a soft shadow; the draft is drawn inside it.
        let radius = 14.0;
        p.add(
            egui::Shadow {
                offset: [0, 12],
                blur: 40,
                spread: 0,
                color: Color32::from_black_alpha(if theme.dark { 140 } else { 45 }),
            }
            .as_shape(panel, radius),
        );
        p.rect_filled(panel, radius, theme.card_fill);
        let saved = self.camera;
        self.camera = Camera::new(center, JOT_ZOOM);
        let clip = p.with_clip_rect(panel.shrink(2.0).intersect(screen));
        self.jot_anim = Some(Anim::settled(world));
        self.paint_card(ctx, &clip, screen, theme, JOT_ID, now);
        self.camera = saved;
        p.rect_stroke(
            panel,
            radius,
            Stroke::new(1.5, theme.card_stroke),
            StrokeKind::Inside,
        );
        if let Some(j) = &mut self.jot {
            j.panel = panel;
        }

        // How tall the text is, and keeping the caret in view.
        if let (Some((_, text, k)), Some(ed)) = (&self.edit_text, &self.editor) {
            let content = text.size.y * k / JOT_ZOOM + CARD_PAD * 2.0 + 40.0;
            let has_title = ed.has_title();
            let caret = text
                .rows
                .iter()
                .find(|r| match r.line {
                    None => ed.cursor.line == 0,
                    Some(i) => i + usize::from(has_title) == ed.cursor.line,
                })
                .map(|r| {
                    let top = r.offset.y * k / JOT_ZOOM + CARD_PAD;
                    (top, top + r.galley.size().y.max(r.size.y) * k / JOT_ZOOM)
                });
            if let Some(j) = &mut self.jot {
                j.content_h = content;
                if let Some((a, b)) = caret {
                    let margin = 30.0;
                    if b > j.scroll + visible - margin {
                        j.scroll = b - visible + margin;
                    } else if a < j.scroll + margin {
                        j.scroll = (a - margin).max(0.0);
                    }
                    j.scroll = j.scroll.clamp(0.0, (content - visible).max(0.0));
                }
            }
        }

        // What happens next, under the panel.
        let busy = self
            .jot
            .as_ref()
            .and_then(|j| j.job.as_ref())
            .map(|j| j.label.clone());
        let line = match busy {
            Some(label) => {
                let dots = ".".repeat(1 + ((now * 2.0) as usize % 3));
                ctx.request_repaint();
                format!("AI · {label}{dots}    Esc to keep writing")
            }
            None => {
                let ai_hint = if ai::available() {
                    "Start with / to ask AI (/picture draws)  ·  "
                } else {
                    ""
                };
                keys(&format!(
                    "Enter places it  ·  ⇧Enter new line  ·  ⌘B ⌘I ⌘E  ·  paste pictures  ·  {ai_hint}Esc"
                ))
            }
        };
        p.text(
            pos2(panel.center().x, panel.max.y + 14.0),
            Align2::CENTER_TOP,
            line,
            FontId::new(13.5, FontFamily::Proportional),
            theme.muted,
        );
    }

    /// The places to choose from, when the judge was unsure.
    pub(super) fn jot_choices_ui(&mut self, ui: &mut Ui, theme: &Theme) {
        let Some(j) = &self.jot else { return };
        if j.choices.is_empty() {
            return;
        }
        let choices = j.choices.clone();
        let anchor = pos2(j.panel.center().x, j.panel.max.y + 44.0);
        let mut picked = None;
        egui::Area::new(egui::Id::unique("skiss-jot-choices"))
            .fixed_pos(anchor)
            .pivot(Align2::CENTER_TOP)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                glass(ui, theme, 14.0, egui::Margin::symmetric(12, 8), |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(
                            egui::RichText::new("Where does it go?")
                                .size(13.5)
                                .color(Glass::of(theme).ink),
                        );
                        for (i, c) in choices.iter().enumerate() {
                            let text = egui::RichText::new(format!(
                                "{}  {}  {:.0}%",
                                i + 1,
                                c.label,
                                c.probability * 100.0
                            ))
                            .size(14.0);
                            if pill(ui, text, true, theme, vec2(40.0, 30.0)).clicked() {
                                picked = Some(i);
                            }
                        }
                    });
                });
            });
        let digit = ui.input(|i| {
            [Key::Num1, Key::Num2, Key::Num3]
                .iter()
                .position(|k| i.key_pressed(*k))
                .or_else(|| i.key_pressed(Key::Enter).then_some(0))
        });
        if let Some(i) = picked.or(digit) {
            let ctx = ui.ctx().clone();
            self.choose_jot(&ctx, i);
        }
    }
}

/// A card parsed from Markdown, to stand in for the draft (it is never part of the file).
fn draft_card(md: &str) -> Option<Card> {
    let board = Board::parse(md);
    board.walk().into_iter().find_map(|n| match n {
        NodeRef::Card(c) => Some(c.clone()),
        NodeRef::Area(_) => None,
    })
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
