//! Bundled fonts: Caveat for the Sketchbook theme, Hanken Grotesk and Spectral for Ember.
//! All are under the SIL Open Font License (see `fonts/OFL-*.txt`).

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

pub const HAND: &str = "hand";
pub const HAND_BOLD: &str = "hand-bold";
pub const SANS: &str = "sans";
pub const SANS_MEDIUM: &str = "sans-medium";
pub const SANS_ITALIC: &str = "sans-italic";
pub const SERIF: &str = "serif";
pub const SERIF_ITALIC: &str = "serif-italic";

pub fn family(name: &str) -> FontFamily {
    FontFamily::Name(name.into())
}

pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let fallback: Vec<String> = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let fonts: [(&str, &'static [u8]); 10] = [
        (
            "Caveat-Regular",
            include_bytes!("../../fonts/Caveat-Regular.ttf"),
        ),
        (
            "Caveat-Regular-Ext",
            include_bytes!("../../fonts/Caveat-Regular-LatinExt.ttf"),
        ),
        ("Caveat-Bold", include_bytes!("../../fonts/Caveat-Bold.ttf")),
        (
            "Caveat-Bold-Ext",
            include_bytes!("../../fonts/Caveat-Bold-LatinExt.ttf"),
        ),
        (
            "Hanken-Regular",
            include_bytes!("../../fonts/HankenGrotesk-Regular.ttf"),
        ),
        (
            "Hanken-Medium",
            include_bytes!("../../fonts/HankenGrotesk-Medium.ttf"),
        ),
        (
            "Hanken-Italic",
            include_bytes!("../../fonts/HankenGrotesk-Italic.ttf"),
        ),
        (
            "Spectral-Light",
            include_bytes!("../../fonts/Spectral-Light.ttf"),
        ),
        (
            "Spectral-LightItalic",
            include_bytes!("../../fonts/Spectral-LightItalic.ttf"),
        ),
        (
            "JetBrainsMono",
            include_bytes!("../../fonts/JetBrainsMono-Regular.ttf"),
        ),
    ];
    for (name, bytes) in fonts {
        defs.font_data
            .insert(name.to_string(), Arc::new(FontData::from_static(bytes)));
    }

    let mut set = |name: &str, primary: &[&str]| {
        let mut list: Vec<String> = primary.iter().map(|s| s.to_string()).collect();
        list.extend(fallback.iter().cloned());
        defs.families.insert(family(name), list);
    };
    set(HAND, &["Caveat-Regular", "Caveat-Regular-Ext"]);
    set(HAND_BOLD, &["Caveat-Bold", "Caveat-Bold-Ext"]);
    set(SANS, &["Hanken-Regular"]);
    set(SANS_MEDIUM, &["Hanken-Medium"]);
    set(SANS_ITALIC, &["Hanken-Italic"]);
    set(SERIF, &["Spectral-Light"]);
    set(SERIF_ITALIC, &["Spectral-LightItalic"]);

    // UI widgets (start screen, prompts) use Hanken Grotesk too.
    if let Some(p) = defs.families.get_mut(&FontFamily::Proportional) {
        p.insert(0, "Hanken-Regular".to_string());
    }
    if let Some(m) = defs.families.get_mut(&FontFamily::Monospace) {
        m.insert(0, "JetBrainsMono".to_string());
    }
    ctx.set_fonts(defs);
}
