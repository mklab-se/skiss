//! Images on cards: loaded from paths relative to the skiss-file, kept as textures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{ColorImage, TextureHandle, TextureOptions};

pub enum Slot {
    Ready(TextureHandle, f32),
    Failed,
}

#[derive(Default)]
pub struct Images {
    slots: HashMap<PathBuf, Slot>,
}

impl Images {
    fn resolve(dir: &Path, url: &str) -> Option<PathBuf> {
        if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("data:") {
            return None;
        }
        let decoded = url.replace("%20", " ");
        let p = Path::new(&decoded);
        Some(if p.is_absolute() {
            p.to_path_buf()
        } else {
            dir.join(p)
        })
    }

    /// Height / width of the image, loading it on first use.
    pub fn aspect(&mut self, ctx: &egui::Context, dir: &Path, url: &str) -> Option<f32> {
        match self.get(ctx, dir, url)? {
            Slot::Ready(_, a) => Some(*a),
            Slot::Failed => None,
        }
    }

    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        dir: &Path,
        url: &str,
    ) -> Option<&TextureHandle> {
        match self.get(ctx, dir, url)? {
            Slot::Ready(t, _) => Some(t),
            Slot::Failed => None,
        }
    }

    fn get(&mut self, ctx: &egui::Context, dir: &Path, url: &str) -> Option<&Slot> {
        let path = Self::resolve(dir, url)?;
        if !self.slots.contains_key(&path) {
            let slot = match image::open(&path) {
                Ok(img) => {
                    let img = img.thumbnail(1600, 1600).to_rgba8();
                    let (w, h) = img.dimensions();
                    let color =
                        ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
                    let tex =
                        ctx.load_texture(path.to_string_lossy(), color, TextureOptions::LINEAR);
                    Slot::Ready(tex, h as f32 / w.max(1) as f32)
                }
                Err(_) => Slot::Failed,
            };
            self.slots.insert(path.clone(), slot);
        }
        self.slots.get(&path)
    }

    /// Forget an image so it is reloaded (after it was replaced on disk).
    pub fn forget_all(&mut self) {
        self.slots.clear();
    }
}

/// Saves a pasted image next to the skiss-file and returns its relative path.
pub fn save_pasted(assets: &Path, doc_dir: &Path, image: &ColorImage) -> anyhow::Result<String> {
    std::fs::create_dir_all(assets)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let mut name = format!("pasted-{stamp}.png");
    let mut n = 2;
    while assets.join(&name).exists() {
        name = format!("pasted-{stamp}-{n}.png");
        n += 1;
    }
    let [w, h] = image.size;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for px in &image.pixels {
        let [r, g, b, a] = px.to_srgba_unmultiplied();
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, rgba)
        .ok_or_else(|| anyhow::anyhow!("invalid image data"))?;
    let path = assets.join(&name);
    buf.save(&path)?;
    let rel = path.strip_prefix(doc_dir).unwrap_or(&path);
    Ok(rel.to_string_lossy().replace('\\', "/").replace(' ', "%20"))
}
