//! The two looks of skiss 0.1: Sketchbook (default) and Ember.

use egui::{Color32, FontFamily};
use skiss_core::board::Color;

use super::fonts;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeId {
    Sketchbook,
    Ember,
}

impl ThemeId {
    pub fn next(self) -> ThemeId {
        match self {
            ThemeId::Sketchbook => ThemeId::Ember,
            ThemeId::Ember => ThemeId::Sketchbook,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Sketchbook => "sketchbook",
            ThemeId::Ember => "ember",
        }
    }

    pub fn parse(s: &str) -> Option<ThemeId> {
        match s.trim() {
            "sketchbook" => Some(ThemeId::Sketchbook),
            "ember" => Some(ThemeId::Ember),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub id: ThemeId,
    pub dark: bool,
    pub bg: Color32,
    pub grid: Color32,
    pub area_fill: Color32,
    pub area_stroke: Color32,
    pub area_title: Color32,
    pub card_fill: Color32,
    pub card_stroke: Color32,
    pub card_shadow: Color32,
    pub card_radius: f32,
    pub text: Color32,
    pub muted: Color32,
    pub link: Color32,
    /// Selection and highlighted connections.
    pub accent: Color32,
    pub accent_soft: Color32,
    pub arrow: Color32,
    pub label_bg: Color32,
    pub ui_bg: Color32,
    pub ui_text: Color32,
    /// Maximum card tilt in radians (hand-placed paper).
    pub tilt: f32,
    /// Pencil wobble of lines, in world units.
    pub wobble: f32,
    /// Dashed area frames.
    pub dashed: bool,
    pub glow: bool,
    pub body: FontFamily,
    pub bold: FontFamily,
    pub italic: Option<FontFamily>,
    pub title: FontFamily,
    pub area_font: FontFamily,
    pub body_size: f32,
    pub title_size: f32,
    pub area_size: f32,
    pub board_title_size: f32,
}

impl Theme {
    pub fn get(id: ThemeId) -> Theme {
        match id {
            ThemeId::Sketchbook => Theme {
                id,
                dark: false,
                bg: Color32::from_rgb(0xf5, 0xef, 0xe2),
                grid: Color32::from_rgba_unmultiplied(80, 60, 30, 34),
                area_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 40),
                area_stroke: Color32::from_rgb(0x8c, 0x81, 0x73),
                area_title: Color32::from_rgb(0x5d, 0x54, 0x48),
                card_fill: Color32::from_rgb(0xff, 0xfd, 0xf7),
                card_stroke: Color32::from_rgb(0x3a, 0x34, 0x2c),
                card_shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 22),
                card_radius: 7.0,
                text: Color32::from_rgb(0x2b, 0x27, 0x22),
                muted: Color32::from_rgb(0x8c, 0x81, 0x73),
                link: Color32::from_rgb(0x1f, 0x5f, 0x9e),
                accent: Color32::from_rgb(0xc0, 0x39, 0x2b),
                accent_soft: Color32::from_rgba_unmultiplied(255, 200, 60, 150),
                arrow: Color32::from_rgb(0x4a, 0x43, 0x3a),
                label_bg: Color32::from_rgb(0xf5, 0xef, 0xe2),
                ui_bg: Color32::from_rgb(0x2b, 0x27, 0x22),
                ui_text: Color32::from_rgb(0xf5, 0xef, 0xe2),
                tilt: 0.018,
                wobble: 1.2,
                dashed: true,
                glow: false,
                body: fonts::family(fonts::HAND),
                bold: fonts::family(fonts::HAND_BOLD),
                italic: None,
                title: fonts::family(fonts::HAND_BOLD),
                area_font: fonts::family(fonts::HAND_BOLD),
                body_size: 21.0,
                title_size: 26.0,
                area_size: 28.0,
                board_title_size: 54.0,
            },
            ThemeId::Ember => Theme {
                id,
                dark: true,
                bg: Color32::from_rgb(0x0c, 0x0b, 0x0a),
                grid: Color32::from_rgba_unmultiplied(255, 255, 255, 16),
                area_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 4),
                area_stroke: Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                area_title: Color32::from_rgb(0xa3, 0x9a, 0x92),
                card_fill: Color32::from_rgb(0x17, 0x15, 0x13),
                card_stroke: Color32::from_rgb(0x2e, 0x2a, 0x26),
                card_shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
                card_radius: 9.0,
                text: Color32::from_rgb(0xec, 0xe6, 0xdf),
                muted: Color32::from_rgb(0x8a, 0x81, 0x7a),
                link: Color32::from_rgb(0xff, 0x8a, 0x5c),
                accent: Color32::from_rgb(0xff, 0x4d, 0x1c),
                accent_soft: Color32::from_rgba_unmultiplied(255, 77, 28, 70),
                arrow: Color32::from_rgb(0x6d, 0x65, 0x5e),
                label_bg: Color32::from_rgb(0x0c, 0x0b, 0x0a),
                ui_bg: Color32::from_rgb(0x1f, 0x1c, 0x19),
                ui_text: Color32::from_rgb(0xec, 0xe6, 0xdf),
                tilt: 0.0,
                wobble: 0.0,
                dashed: false,
                glow: true,
                body: fonts::family(fonts::SANS),
                bold: fonts::family(fonts::SANS_MEDIUM),
                italic: Some(fonts::family(fonts::SANS_ITALIC)),
                title: fonts::family(fonts::SERIF),
                area_font: fonts::family(fonts::SANS_MEDIUM),
                body_size: 15.0,
                title_size: 21.0,
                area_size: 15.0,
                board_title_size: 44.0,
            },
        }
    }

    /// The paper color of a colored card.
    pub fn paper(&self, color: Color) -> Color32 {
        if self.dark {
            match color {
                Color::Red => Color32::from_rgb(0x3a, 0x17, 0x12),
                Color::Orange => Color32::from_rgb(0x3a, 0x22, 0x10),
                Color::Yellow => Color32::from_rgb(0x36, 0x2e, 0x12),
                Color::Green => Color32::from_rgb(0x16, 0x2c, 0x1b),
                Color::Blue => Color32::from_rgb(0x13, 0x22, 0x36),
                Color::Purple => Color32::from_rgb(0x2a, 0x17, 0x33),
                Color::Gray => Color32::from_rgb(0x26, 0x24, 0x22),
            }
        } else {
            match color {
                Color::Red => Color32::from_rgb(0xf9, 0xd9, 0xd2),
                Color::Orange => Color32::from_rgb(0xfb, 0xe0, 0xc4),
                Color::Yellow => Color32::from_rgb(0xfb, 0xec, 0xa8),
                Color::Green => Color32::from_rgb(0xd6, 0xec, 0xcf),
                Color::Blue => Color32::from_rgb(0xd2, 0xe3, 0xf5),
                Color::Purple => Color32::from_rgb(0xe6, 0xd8, 0xf2),
                Color::Gray => Color32::from_rgb(0xe4, 0xe0, 0xd8),
            }
        }
    }

    /// The ink (stroke) of a colored card.
    pub fn ink(&self, color: Color) -> Color32 {
        match color {
            Color::Red => Color32::from_rgb(0xc0, 0x39, 0x2b),
            Color::Orange => Color32::from_rgb(0xd3, 0x6b, 0x16),
            Color::Yellow => Color32::from_rgb(0xb8, 0x8a, 0x00),
            Color::Green => Color32::from_rgb(0x3d, 0x8b, 0x46),
            Color::Blue => Color32::from_rgb(0x2f, 0x6f, 0xb5),
            Color::Purple => Color32::from_rgb(0x84, 0x4a, 0xb0),
            Color::Gray => Color32::from_rgb(0x8a, 0x84, 0x7a),
        }
    }
}
