// SPDX-License-Identifier: GPL-3.0-or-later
//! The house design system, ported from phosphor's canonical native
//! implementation (crates/phosphor-app/src/theme.rs) — same tokens, same
//! hard rules: sharp corners everywhere, hairline frames, mono for data,
//! depth reserved for the few carved controls. Blossom Dark is the default.

use egui::Color32;

#[derive(Clone, Copy)]
pub struct Palette {
    pub id: &'static str,
    pub label: &'static str,
    pub dark: bool,
    pub plane: Color32,
    pub surface: Color32,
    pub surface_2: Color32,
    pub ink: Color32,
    pub ink_2: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub stone: Color32,
    pub stone_hi: Color32,
    pub stone_lo: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(r, g, b, a)
}

/// Semantic status colors — stable across the whole estate (same meanings as
/// surveyor's exposure badges and the CLI lamps).
pub fn status_color(p: &Palette, state: crate::probe::State) -> Color32 {
    use crate::probe::State::*;
    match state {
        Ok => rgb(0x87, 0xd7, 0x87),
        Present => rgb(0x87, 0xc7, 0x9c),
        Unavailable => rgb(0xff, 0xaf, 0x00),
        Error | Missing => rgb(0xe0, 0x5a, 0x5a),
        Unprobed => p.muted,
    }
}

pub const PALETTES: [Palette; 4] = [
    // Blossom Dark — warm wine-plum ground, sakura-rose accent (THE default)
    Palette {
        id: "blossom_dark",
        label: "Blossom Dark",
        dark: true,
        plane: rgb(0x1c, 0x10, 0x16),
        surface: rgb(0x28, 0x18, 0x21),
        surface_2: rgb(0x33, 0x21, 0x2c),
        ink: rgb(0xf5, 0xea, 0xef),
        ink_2: rgb(0xc9, 0xb0, 0xbc),
        muted: rgb(0x91, 0x79, 0x86),
        line: rgba(244, 233, 238, 36),
        line_strong: rgba(244, 233, 238, 82),
        accent: rgb(0xec, 0x8f, 0xac),
        on_accent: rgb(0x1a, 0x0e, 0x14),
        stone: rgb(0x3b, 0x26, 0x31),
        stone_hi: rgb(0x55, 0x39, 0x48),
        stone_lo: rgb(0x1d, 0x11, 0x17),
    },
    // Blossom — warm sakura, rice-paper, ink
    Palette {
        id: "blossom",
        label: "Blossom",
        dark: false,
        plane: rgb(0xf3, 0xe6, 0xe6),
        surface: rgb(0xfc, 0xf4, 0xf3),
        surface_2: rgb(0xf6, 0xe9, 0xe9),
        ink: rgb(0x2b, 0x21, 0x28),
        ink_2: rgb(0x5f, 0x4f, 0x57),
        muted: rgb(0x9c, 0x88, 0x90),
        line: rgba(43, 33, 40, 41),
        line_strong: rgba(43, 33, 40, 77),
        accent: rgb(0xc8, 0x5a, 0x7c),
        on_accent: rgb(0xff, 0xf7, 0xf9),
        stone: rgb(0xe7, 0xda, 0xd9),
        stone_hi: rgb(0xff, 0xfa, 0xfa),
        stone_lo: rgb(0xc9, 0xb6, 0xb8),
    },
    // Light — cool neutral instrument
    Palette {
        id: "light",
        label: "Light",
        dark: false,
        plane: rgb(0xea, 0xee, 0xf2),
        surface: rgb(0xff, 0xff, 0xff),
        surface_2: rgb(0xf5, 0xf8, 0xfa),
        ink: rgb(0x0e, 0x16, 0x20),
        ink_2: rgb(0x43, 0x51, 0x5e),
        muted: rgb(0x7a, 0x88, 0x94),
        line: rgba(14, 22, 32, 31),
        line_strong: rgba(14, 22, 32, 71),
        accent: rgb(0x0c, 0x94, 0xa2),
        on_accent: rgb(0xff, 0xff, 0xff),
        stone: rgb(0xe4, 0xe9, 0xee),
        stone_hi: rgb(0xff, 0xff, 0xff),
        stone_lo: rgb(0xc2, 0xcc, 0xd4),
    },
    // Dark — deep instrument, plum-tinted near-black
    Palette {
        id: "dark",
        label: "Dark",
        dark: true,
        plane: rgb(0x0a, 0x08, 0x10),
        surface: rgb(0x14, 0x10, 0x19),
        surface_2: rgb(0x1b, 0x15, 0x22),
        ink: rgb(0xf0, 0xea, 0xf0),
        ink_2: rgb(0xb3, 0xa6, 0xb3),
        muted: rgb(0x7d, 0x6f, 0x7d),
        line: rgba(240, 234, 240, 31),
        line_strong: rgba(240, 234, 240, 66),
        accent: rgb(0xe7, 0x8a, 0xa6),
        on_accent: rgb(0x16, 0x08, 0x10),
        stone: rgb(0x24, 0x1d, 0x29),
        stone_hi: rgb(0x33, 0x28, 0x38),
        stone_lo: rgb(0x14, 0x0f, 0x18),
    },
];

pub fn palette(id: &str) -> Palette {
    PALETTES.iter().copied().find(|p| p.id == id).unwrap_or(PALETTES[0])
}

pub fn next_palette(id: &str) -> Palette {
    let i = PALETTES.iter().position(|p| p.id == id).unwrap_or(0);
    PALETTES[(i + 1) % PALETTES.len()]
}

impl Palette {
    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() };

        let sharp = egui::CornerRadius::ZERO;
        visuals.window_corner_radius = sharp;
        visuals.menu_corner_radius = sharp;

        visuals.panel_fill = self.surface;
        visuals.window_fill = self.surface;
        visuals.window_stroke = egui::Stroke::new(1.0, self.line_strong);
        visuals.extreme_bg_color = self.plane;
        visuals.faint_bg_color = self.surface_2;
        visuals.override_text_color = None;
        visuals.weak_text_alpha = 0.7;
        visuals.hyperlink_color = self.accent;
        visuals.selection.bg_fill = self.accent;
        visuals.selection.stroke = egui::Stroke::new(1.0, self.on_accent);

        let hairline = egui::Stroke::new(1.0, self.line);
        let hairline_strong = egui::Stroke::new(1.0, self.line_strong);
        let button_face = lerp(self.surface_2, self.stone, 0.35);
        let widgets = &mut visuals.widgets;
        for w in [
            &mut widgets.noninteractive,
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            w.corner_radius = sharp;
        }
        widgets.noninteractive.bg_fill = self.surface;
        widgets.noninteractive.weak_bg_fill = self.surface;
        widgets.noninteractive.bg_stroke = hairline;
        widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, self.ink);

        widgets.inactive.bg_fill = button_face;
        widgets.inactive.weak_bg_fill = self.surface_2;
        widgets.inactive.bg_stroke = hairline_strong;
        widgets.inactive.fg_stroke = egui::Stroke::new(1.0, self.ink);

        widgets.hovered.bg_fill = lerp(button_face, self.accent, 0.14);
        widgets.hovered.weak_bg_fill = lerp(self.surface_2, self.accent, 0.14);
        widgets.hovered.bg_stroke = egui::Stroke::new(1.0, lerp_rgba(self.line_strong, self.accent, 0.5));
        widgets.hovered.fg_stroke = egui::Stroke::new(1.0, self.ink);
        widgets.hovered.expansion = 1.0;

        widgets.active.bg_fill = lerp(self.surface_2, self.accent, 0.30);
        widgets.active.weak_bg_fill = lerp(self.surface_2, self.accent, 0.30);
        widgets.active.bg_stroke = egui::Stroke::new(1.0, self.accent);
        widgets.active.fg_stroke = egui::Stroke::new(1.0, self.ink);
        widgets.active.expansion = -0.5;

        widgets.open.bg_fill = self.surface_2;
        widgets.open.bg_stroke = hairline_strong;

        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.animation_time = 0.12; // restrained, purposeful motion
        style.spacing.button_padding = egui::vec2(9.0, 4.5);
        style.spacing.item_spacing = egui::vec2(7.0, 5.0);
        style.spacing.window_margin = egui::Margin::same(10);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.5));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.5));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(17.5));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.5));
        ctx.set_style(style);
    }

    /// The carved-stone treatment for the FEW important controls — bevel
    /// catch-light top-left, shadow bottom-right; pressed = it sinks;
    /// `face_mix` eases the accent-tinted "on" face in and out.
    pub fn carve_with_face(&self, painter: &egui::Painter, rect: egui::Rect, pressed: bool, face_mix: f32) {
        let t = face_mix.clamp(0.0, 1.0);
        let on_face = lerp(self.stone, self.accent, 0.34);
        let face = lerp(self.stone, on_face, t);
        painter.rect_filled(rect, 0.0, face);
        let hi = egui::Stroke::new(1.0, self.stone_hi);
        let lo = egui::Stroke::new(1.0, self.stone_lo);
        let (top_left, bottom_right) = if pressed { (lo, hi) } else { (hi, lo) };
        painter.line_segment([rect.left_top(), rect.right_top()], top_left);
        painter.line_segment([rect.left_top(), rect.left_bottom()], top_left);
        painter.line_segment([rect.left_bottom(), rect.right_bottom()], bottom_right);
        painter.line_segment([rect.right_top(), rect.right_bottom()], bottom_right);
        if t > 0.003 {
            let rim = self.accent.gamma_multiply(t);
            painter.rect_stroke(
                rect.shrink(1.0),
                0.0,
                egui::Stroke::new(1.0, rim),
                egui::StrokeKind::Inside,
            );
        }
    }
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

pub fn lerp_rgba(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_premultiplied(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()), mix(a.a(), b.a()))
}
