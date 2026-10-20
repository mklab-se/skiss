//! The canvas window.

mod app;
mod content;
mod document;
mod draw;
mod editor;
mod fonts;
mod images;
mod layout;
mod motion;
mod script;
mod theme;
mod view;

use std::path::PathBuf;

/// Opens the canvas for `path` (or the start screen) and blocks until the window closes.
pub fn run(path: Option<PathBuf>, theme: Option<String>) -> anyhow::Result<()> {
    if let Some(t) = &theme
        && theme::ThemeId::parse(t).is_none()
    {
        anyhow::bail!("unknown theme '{t}' (use sketchbook or ember)");
    }
    let title = path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| format!("{} · skiss", n.to_string_lossy()))
        .unwrap_or_else(|| "skiss".to_string());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([520.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native(
        "skiss",
        options,
        Box::new(move |cc| Ok(Box::new(app::SkissApp::new(cc, path, theme)))),
    )
    .map_err(|e| anyhow::anyhow!("could not open the window: {e}"))
}
