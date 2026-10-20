//! The open skiss-file: autosave, watching the file for outside changes, and session undo.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use skiss_core::board::Board;

/// How long after the last change the file is written.
const SAVE_DELAY: Duration = Duration::from_millis(250);
/// Typing pauses longer than this start a new undo step.
const COALESCE: Duration = Duration::from_millis(1200);

#[derive(Clone)]
pub struct Snapshot {
    pub source: String,
    pub selection: Vec<String>,
}

pub struct Document {
    pub path: PathBuf,
    pub source: String,
    pub board: Board,
    /// Bumped on every change, so caches can tell the board changed.
    pub revision: u64,
    /// What is on disk as far as we know (last read or written).
    on_disk: String,
    dirty_since: Option<Instant>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Groups rapid edits (typing) into one undo step.
    coalesce_key: Option<String>,
    last_edit: Option<Instant>,
    _watcher: Option<RecommendedWatcher>,
    events: Option<Receiver<()>>,
    pub error: Option<String>,
}

impl Document {
    pub fn open(path: &Path, ctx: &egui::Context) -> anyhow::Result<Document> {
        let source = std::fs::read_to_string(path)?;
        let board = Board::parse(&source);
        let (tx, rx) = channel();
        let repaint = ctx.clone();
        let file_name = path.file_name().map(|n| n.to_os_string());
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res
                && event
                    .paths
                    .iter()
                    .any(|p| p.file_name().map(|n| n.to_os_string()) == file_name)
            {
                let _ = tx.send(());
                repaint.request_repaint();
            }
        })
        .ok()
        .and_then(|mut w| {
            let dir = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            w.watch(dir, RecursiveMode::NonRecursive).ok().map(|_| w)
        });
        Ok(Document {
            path: path.to_path_buf(),
            on_disk: source.clone(),
            source,
            board,
            revision: 0,
            dirty_since: None,
            undo: Vec::new(),
            redo: Vec::new(),
            coalesce_key: None,
            last_edit: None,
            events: watcher.as_ref().map(|_| rx),
            _watcher: watcher,
            error: None,
        })
    }

    /// The folder for pasted images: `notes.assets/` next to `notes.md`.
    pub fn assets_dir(&self) -> PathBuf {
        let stem = self
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "notes".into());
        self.dir().join(format!("{stem}.assets"))
    }

    pub fn dir(&self) -> PathBuf {
        self.path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// Applies an edit. `coalesce` names a typing burst (for example the card being edited):
    /// consecutive edits with the same key within a short time share one undo step.
    pub fn apply(&mut self, source: String, selection: &[String], coalesce: Option<&str>) {
        if source == self.source {
            return;
        }
        let now = Instant::now();
        let same_burst = coalesce.is_some()
            && coalesce.map(str::to_string) == self.coalesce_key
            && self.last_edit.is_some_and(|t| now - t < COALESCE);
        if !same_burst {
            self.undo.push(Snapshot {
                source: self.source.clone(),
                selection: selection.to_vec(),
            });
            if self.undo.len() > 500 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.coalesce_key = coalesce.map(str::to_string);
        self.last_edit = Some(now);
        self.set_source(source);
    }

    /// Ends the current typing burst so the next edit gets its own undo step.
    pub fn break_coalescing(&mut self) {
        self.coalesce_key = None;
    }

    fn set_source(&mut self, source: String) {
        self.board = Board::parse(&source);
        self.source = source;
        self.revision += 1;
        self.dirty_since = Some(Instant::now());
    }

    pub fn undo(&mut self, selection: &[String]) -> Option<Vec<String>> {
        let snap = self.undo.pop()?;
        self.redo.push(Snapshot {
            source: self.source.clone(),
            selection: selection.to_vec(),
        });
        self.coalesce_key = None;
        self.set_source(snap.source);
        Some(snap.selection)
    }

    pub fn redo(&mut self, selection: &[String]) -> Option<Vec<String>> {
        let snap = self.redo.pop()?;
        self.undo.push(Snapshot {
            source: self.source.clone(),
            selection: selection.to_vec(),
        });
        self.coalesce_key = None;
        self.set_source(snap.source);
        Some(snap.selection)
    }

    /// Writes pending changes when they have settled. Returns how long until the next write is due.
    pub fn autosave(&mut self, force: bool) -> Option<Duration> {
        let since = self.dirty_since?;
        let elapsed = since.elapsed();
        if !force && elapsed < SAVE_DELAY {
            return Some(SAVE_DELAY - elapsed);
        }
        self.dirty_since = None;
        if self.source == self.on_disk {
            return None;
        }
        match std::fs::write(&self.path, &self.source) {
            Ok(()) => {
                self.on_disk = self.source.clone();
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!("Could not save {}: {e}", self.path.display()));
                self.dirty_since = Some(Instant::now());
            }
        }
        None
    }

    /// Picks up changes made to the file by someone else (Git, another editor, a sync tool).
    /// Returns true if the board changed.
    pub fn poll_disk(&mut self) -> bool {
        let Some(rx) = &self.events else {
            return false;
        };
        let mut changed = false;
        while rx.try_recv().is_ok() {
            changed = true;
        }
        if !changed {
            return false;
        }
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return false;
        };
        if text == self.on_disk || text == self.source {
            self.on_disk = text;
            return false;
        }
        // Outside change: it wins over anything not yet written, and is undoable.
        self.undo.push(Snapshot {
            source: self.source.clone(),
            selection: Vec::new(),
        });
        self.redo.clear();
        self.coalesce_key = None;
        self.on_disk = text.clone();
        self.board = Board::parse(&text);
        self.source = text;
        self.revision += 1;
        self.dirty_since = None;
        true
    }
}

/// A readable board title from a file name: `my-notes.md` -> "My notes".
pub fn title_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Notes".into());
    let words = stem.replace(['-', '_'], " ");
    let mut c = words.trim().chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Notes".into(),
    }
}

/// Creates the file with a title if it does not exist yet.
pub fn ensure_exists(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("# {}\n", title_from_path(path)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_from_file_names() {
        assert_eq!(title_from_path(Path::new("my-notes.md")), "My notes");
        assert_eq!(
            title_from_path(Path::new("dir/apollo_program.md")),
            "Apollo program"
        );
    }
}
