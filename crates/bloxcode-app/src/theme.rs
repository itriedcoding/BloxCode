//! Monochrome design system. Every colour in the app comes from here: pure
//! greys, hairline borders, no gradients, no shadows, no rounded corners beyond
//! a 2px optical softening.

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Vec2,
};

pub const BG: Color32 = Color32::from_rgb(8, 8, 8);
pub const PANEL: Color32 = Color32::from_rgb(12, 12, 12);
pub const RAISED: Color32 = Color32::from_rgb(17, 17, 17);
pub const HOVER: Color32 = Color32::from_rgb(26, 26, 26);
pub const ACTIVE: Color32 = Color32::from_rgb(33, 33, 33);
pub const LINE: Color32 = Color32::from_rgb(30, 30, 30);
pub const LINE_STRONG: Color32 = Color32::from_rgb(58, 58, 58);
pub const TEXT: Color32 = Color32::from_rgb(240, 240, 240);
pub const MUTED: Color32 = Color32::from_rgb(150, 150, 150);
pub const DIM: Color32 = Color32::from_rgb(96, 96, 96);
pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
pub const BLACK: Color32 = Color32::from_rgb(0, 0, 0);

// Code colours: greyscale only.
pub const CODE_TEXT: Color32 = Color32::from_rgb(206, 206, 206);
pub const CODE_KEYWORD: Color32 = Color32::from_rgb(244, 244, 244);
pub const CODE_STRING: Color32 = Color32::from_rgb(160, 160, 160);
pub const CODE_COMMENT: Color32 = Color32::from_rgb(94, 94, 94);
pub const CODE_NUMBER: Color32 = Color32::from_rgb(210, 210, 210);
pub const CODE_FUNCTION: Color32 = Color32::from_rgb(255, 255, 255);
pub const CODE_PUNCT: Color32 = Color32::from_rgb(134, 134, 134);

const SANS: &str = "plex-sans";
const SANS_MEDIUM: &str = "plex-sans-medium";
const SANS_SEMIBOLD: &str = "plex-sans-semibold";
const MONO: &str = "plex-mono";

pub fn medium_family() -> FontFamily {
    FontFamily::Name("plex-medium".into())
}

pub fn semibold_family() -> FontFamily {
    FontFamily::Name("plex-semibold".into())
}

pub fn sans(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, medium_family())
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, semibold_family())
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        SANS.to_owned(),
        FontData::from_static(include_bytes!("../assets/fonts/PlexSans-Regular.ttf")),
    );
    fonts.font_data.insert(
        SANS_MEDIUM.to_owned(),
        FontData::from_static(include_bytes!("../assets/fonts/PlexSans-Medium.ttf")),
    );
    fonts.font_data.insert(
        SANS_SEMIBOLD.to_owned(),
        FontData::from_static(include_bytes!("../assets/fonts/PlexSans-SemiBold.ttf")),
    );
    fonts.font_data.insert(
        MONO.to_owned(),
        FontData::from_static(include_bytes!("../assets/fonts/PlexMono-Regular.ttf")),
    );

    fonts
        .families
        .insert(FontFamily::Proportional, vec![SANS.to_owned()]);
    fonts
        .families
        .insert(medium_family(), vec![SANS_MEDIUM.to_owned()]);
    fonts
        .families
        .insert(semibold_family(), vec![SANS_SEMIBOLD.to_owned()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec![MONO.to_owned()]);
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Small, sans(11.0)),
        (TextStyle::Body, sans(13.0)),
        (TextStyle::Button, medium(12.0)),
        (TextStyle::Heading, semibold(22.0)),
        (TextStyle::Monospace, mono(12.0)),
    ]
    .into();
    style.visuals = visuals();
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = false;
    ctx.set_style(style);
}

fn visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = RAISED;
    visuals.code_bg_color = BG;
    visuals.selection.bg_fill = WHITE;
    visuals.selection.stroke = Stroke::new(1.0_f32, BLACK);
    visuals.hyperlink_color = TEXT;
    visuals.window_stroke = Stroke::new(1.0_f32, LINE_STRONG);
    visuals.window_rounding = egui::Rounding::same(2.0);
    visuals.menu_rounding = egui::Rounding::same(2.0);
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;

    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, MUTED);
    visuals.widgets.noninteractive.rounding = egui::Rounding::same(2.0);

    visuals.widgets.inactive.bg_fill = RAISED;
    visuals.widgets.inactive.weak_bg_fill = RAISED;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.inactive.rounding = egui::Rounding::same(2.0);

    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, LINE_STRONG);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, WHITE);
    visuals.widgets.hovered.rounding = egui::Rounding::same(2.0);

    visuals.widgets.active.bg_fill = ACTIVE;
    visuals.widgets.active.weak_bg_fill = ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, WHITE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, WHITE);
    visuals.widgets.active.rounding = egui::Rounding::same(2.0);

    visuals.widgets.open.bg_fill = RAISED;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, LINE_STRONG);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT);

    visuals
}

/// Uppercase micro label used above headings and section titles.
pub fn kicker_text(text: &str) -> egui::RichText {
    egui::RichText::new(text.to_uppercase())
        .font(medium(9.5))
        .color(DIM)
}
