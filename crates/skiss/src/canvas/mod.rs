//! The canvas window.

mod ai;
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

/// Checks what can be checked before the window opens, so mistakes are reported in the terminal.
/// Creates the file if it does not exist yet.
pub fn check(path: Option<&std::path::Path>, theme: Option<&str>) -> anyhow::Result<()> {
    if let Some(t) = theme
        && theme::ThemeId::parse(t).is_none()
    {
        anyhow::bail!("unknown theme '{t}' (use sketchbook or ember)");
    }
    if let Some(p) = path {
        if p.is_dir() {
            anyhow::bail!("{} is a folder; give a Markdown file name", p.display());
        }
        document::ensure_exists(p)?;
        std::fs::read_to_string(p)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", p.display()))?;
    }
    Ok(())
}

/// Starts the window as its own process and returns, so the terminal gets its prompt back.
pub fn spawn_detached() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(std::env::args_os().skip(1))
        .env("SKISS_DETACHED", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group: Ctrl+C in the terminal does not close the window.
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn()
        .map_err(|e| anyhow::anyhow!("could not start the skiss window: {e}"))?;
    Ok(())
}

/// Opens the canvas for `path` (or the start screen) and blocks until the window closes.
pub fn run(path: Option<PathBuf>, theme: Option<String>) -> anyhow::Result<()> {
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
