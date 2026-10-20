//! The window: the start screen (no file given) and the board.

use std::path::PathBuf;

use egui::{Align2, Color32, FontId, Key, RichText, Ui, vec2};
use skiss_core::config::Config;

use super::document::{self, Document};
use super::fonts;
use super::theme::{Theme, ThemeId};
use super::view::{BoardView, UiRequest};

enum Screen {
    Start {
        name: String,
        error: Option<String>,
        focus: bool,
    },
    Board(Box<BoardView>),
}

pub struct SkissApp {
    theme: Theme,
    screen: Screen,
    config: Config,
    /// Development aid: `SKISS_SCREENSHOT=out.png` saves the window after a moment and quits.
    shot: Option<(PathBuf, f64, bool)>,
    /// Development aid: `SKISS_SCRIPT=steps.txt` drives the window (see `script.rs`).
    script: Option<super::script::Script>,
}

impl SkissApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        path: Option<PathBuf>,
        theme: Option<String>,
    ) -> Self {
        fonts::install(&cc.egui_ctx);
        let config = Config::load().unwrap_or_default();
        let theme_id = theme
            .as_deref()
            .or(config.theme.as_deref())
            .and_then(ThemeId::parse)
            .unwrap_or(ThemeId::Sketchbook);
        let theme = Theme::get(theme_id);
        apply_visuals(&cc.egui_ctx, &theme);
        let mut app = SkissApp {
            theme,
            screen: Screen::Start {
                name: String::new(),
                error: None,
                focus: true,
            },
            config,
            script: super::script::Script::load(),
            shot: std::env::var("SKISS_SCREENSHOT").ok().map(|p| {
                let wait = std::env::var("SKISS_SCREENSHOT_AFTER")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(2.0);
                (PathBuf::from(p), wait, false)
            }),
        };
        if let Some(p) = path
            && let Err(e) = app.open(&cc.egui_ctx, p)
        {
            app.screen = Screen::Start {
                name: String::new(),
                error: Some(e.to_string()),
                focus: true,
            };
        }
        app
    }

    fn open(&mut self, ctx: &egui::Context, path: PathBuf) -> anyhow::Result<()> {
        document::ensure_exists(&path)?;
        let doc = Document::open(&path, ctx)?;
        let name = doc.file_name();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{name} · skiss")));
        self.screen = Screen::Board(Box::new(BoardView::new(doc)));
        Ok(())
    }

    fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.theme = Theme::get(self.theme.id.next());
        apply_visuals(ctx, &self.theme);
        if let Screen::Board(view) = &mut self.screen {
            view.theme_changed();
        }
        self.config.theme = Some(self.theme.id.name().to_string());
        let _ = self.config.save();
    }

    fn start_screen(&mut self, ui: &mut Ui) -> Option<PathBuf> {
        let theme = self.theme.clone();
        let Screen::Start { name, error, focus } = &mut self.screen else {
            return None;
        };
        let rect = ui.max_rect();
        let p = ui.painter();
        p.rect_filled(rect, 0.0, theme.bg);
        super::draw::grid(p, rect, egui::Pos2::ZERO, 1.0, &theme);
        p.text(
            rect.center() - vec2(0.0, 205.0),
            Align2::CENTER_CENTER,
            "skiss",
            FontId::new(theme.board_title_size * 1.4, theme.title.clone()),
            theme.text,
        );
        p.text(
            rect.center() - vec2(0.0, 142.0),
            Align2::CENTER_CENTER,
            "An infinite canvas for your notes, stored as Markdown",
            FontId::new(theme.body_size, theme.body.clone()),
            theme.muted,
        );
        let mut chosen = None;
        egui::Area::new(egui::Id::unique("start"))
            .fixed_pos(rect.center() + vec2(0.0, 10.0))
            .pivot(Align2::CENTER_CENTER)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme.card_fill)
                    .stroke(egui::Stroke::new(1.5, theme.card_stroke))
                    .corner_radius(12.0)
                    .inner_margin(egui::Margin::same(20))
                    .shadow(egui::Shadow {
                        offset: [2, 4],
                        blur: 18,
                        spread: 0,
                        color: Color32::from_black_alpha(40),
                    })
                    .show(ui, |ui| {
                        ui.set_width(380.0);
                        ui.label(
                            RichText::new("Name your skiss")
                                .size(20.0)
                                .color(theme.text),
                        );
                        ui.add_space(6.0);
                        let r = ui.add(
                            egui::TextEdit::singleline(name)
                                .hint_text("project-notes")
                                .font(FontId::proportional(18.0))
                                .desired_width(f32::INFINITY),
                        );
                        if *focus {
                            r.request_focus();
                            *focus = false;
                        }
                        ui.add_space(6.0);
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let file = file_name(name);
                        ui.label(
                            RichText::new(format!("{}", cwd.join(&file).display()))
                                .size(12.0)
                                .color(theme.muted),
                        );
                        if let Some(e) = error {
                            ui.label(RichText::new(e.as_str()).size(12.5).color(theme.accent));
                        }
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        ui.add_space(8.0);
                        let go = ui.add_enabled(
                            !name.trim().is_empty(),
                            egui::Button::new(RichText::new("Open").size(16.0)),
                        );
                        if (enter || go.clicked()) && !name.trim().is_empty() {
                            chosen = Some(cwd.join(file));
                        }
                    });
            });
        chosen
    }
}

impl SkissApp {
    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, wait, asked)) = &mut self.shot else {
            return;
        };
        let t = ctx.input(|i| i.time);
        ctx.request_repaint();
        if !*asked && t > *wait {
            *asked = true;
            let path = path.clone();
            let c = ctx.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                egui::ScreenshotCallback::new(move |img| {
                    let [w, h] = img.size;
                    let mut rgba = Vec::with_capacity(w * h * 4);
                    for px in &img.pixels {
                        rgba.extend_from_slice(&px.to_array());
                    }
                    if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) {
                        let _ = buf.save(&path);
                    }
                    c.send_viewport_cmd(egui::ViewportCommand::Close);
                    c.request_repaint();
                }),
            ));
        }
    }
}

fn file_name(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() {
        return "notes.md".into();
    }
    if n.contains('.') {
        n.to_string()
    } else {
        format!("{n}.md")
    }
}

fn apply_visuals(ctx: &egui::Context, theme: &Theme) {
    let mut v = if theme.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = theme.bg;
    v.window_fill = theme.ui_bg;
    v.extreme_bg_color = if theme.dark {
        Color32::from_rgb(0x12, 0x10, 0x0f)
    } else {
        Color32::WHITE
    };
    v.selection.bg_fill = theme.accent.gamma_multiply(0.5);
    v.selection.stroke.color = theme.accent;
    v.hyperlink_color = theme.link;
    ctx.set_visuals(v);
}

impl eframe::App for SkissApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.screenshot(&ctx);
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::T))
            && matches!(self.screen, Screen::Start { .. })
        {
            self.toggle_theme(&ctx);
        }
        let theme = self.theme.clone();
        let mut open: Option<PathBuf> = None;
        let mut toggle = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme.bg))
            .show(ui, |ui| match &mut self.screen {
                Screen::Start { .. } => open = self.start_screen(ui),
                Screen::Board(view) => {
                    if let Some(UiRequest::ToggleTheme) = view.ui(ui, &theme) {
                        toggle = true;
                    }
                }
            });
        if toggle {
            self.toggle_theme(&ctx);
        }
        if let Some(path) = open
            && let Err(e) = self.open(&ctx, path)
            && let Screen::Start { error, .. } = &mut self.screen
        {
            *error = Some(e.to_string());
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(script) = &mut self.script {
            let view = match &self.screen {
                Screen::Board(v) => Some(v),
                Screen::Start { .. } => None,
            };
            let resolve = |id: &str| view.and_then(|v| v.screen_rect(id));
            script.feed(ctx, raw, &resolve);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Screen::Board(view) = &mut self.screen {
            view.save_now();
        }
    }
}
