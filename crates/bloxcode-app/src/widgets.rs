//! Hand-built controls. Everything is allocated and painted explicitly so the
//! interface stays square, monochrome and free of framework chrome.

use eframe::egui::{self, Align, Color32, Layout, Response, Rounding, Sense, Stroke, Ui, Vec2};

use crate::theme::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Ghost,
    Quiet,
}

pub fn button(ui: &mut Ui, label: &str, kind: ButtonKind) -> Response {
    button_sized(ui, label, kind, None)
}

pub fn button_sized(ui: &mut Ui, label: &str, kind: ButtonKind, width: Option<f32>) -> Response {
    let font = medium(10.5);
    let text = label.to_uppercase();
    let pad = Vec2::new(16.0, 9.0);
    let measured = ui
        .painter()
        .layout_no_wrap(text.clone(), font.clone(), TEXT)
        .size();
    let mut size = measured + pad * 2.0;
    size.y = size.y.max(32.0);
    if let Some(width) = width {
        size.x = width;
    }

    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let enabled = ui.is_enabled();
    let hovered = response.hovered() && enabled;
    let (fill, border, fg) = if !enabled {
        (Color32::TRANSPARENT, LINE, DIM)
    } else {
        match kind {
            ButtonKind::Primary => {
                if hovered {
                    (Color32::from_rgb(214, 214, 214), WHITE, BLACK)
                } else {
                    (WHITE, WHITE, BLACK)
                }
            }
            ButtonKind::Ghost => {
                if hovered {
                    (HOVER, LINE_STRONG, WHITE)
                } else {
                    (Color32::TRANSPARENT, LINE, TEXT)
                }
            }
            ButtonKind::Quiet => {
                if hovered {
                    (Color32::TRANSPARENT, Color32::TRANSPARENT, WHITE)
                } else {
                    (Color32::TRANSPARENT, Color32::TRANSPARENT, MUTED)
                }
            }
        }
    };

    let painter = ui.painter();
    if fill != Color32::TRANSPARENT {
        painter.rect_filled(rect, Rounding::same(2.0), fill);
    }
    if border != Color32::TRANSPARENT {
        painter.rect_stroke(rect, Rounding::same(2.0), Stroke::new(1.0_f32, border));
    }
    let galley = painter.layout_no_wrap(text, font, fg);
    painter.galley(rect.center() - galley.size() * 0.5, galley, fg);

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// Full-width navigation row for the left rail.
pub fn nav_item(ui: &mut Ui, label: &str, badge: Option<String>, active: bool) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hovered = response.hovered();
    if active {
        ui.painter().rect_filled(rect, Rounding::ZERO, RAISED);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.left_top(), Vec2::new(2.0, rect.height())),
            Rounding::ZERO,
            WHITE,
        );
    } else if hovered {
        ui.painter()
            .rect_filled(rect, Rounding::ZERO, Color32::from_rgb(15, 15, 15));
    }

    let color = if active || hovered { TEXT } else { MUTED };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_uppercase(), medium(10.5), color);
    ui.painter().galley(
        egui::pos2(rect.left() + 16.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );

    if let Some(badge) = badge {
        let galley = ui.painter().layout_no_wrap(badge, mono(10.0), DIM);
        let x = rect.right() - 14.0 - galley.size().x;
        ui.painter().galley(
            egui::pos2(x, rect.center().y - galley.size().y * 0.5),
            galley,
            DIM,
        );
    }

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

pub fn chip(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let font = medium(10.0);
    let text = label.to_uppercase();
    let pad = Vec2::new(11.0, 7.0);
    let measured = ui
        .painter()
        .layout_no_wrap(text.clone(), font.clone(), TEXT)
        .size();
    let (rect, response) = ui.allocate_exact_size(measured + pad * 2.0, Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hovered = response.hovered();
    let (fill, border, fg) = if selected {
        (WHITE, WHITE, BLACK)
    } else if hovered {
        (HOVER, LINE_STRONG, TEXT)
    } else {
        (Color32::TRANSPARENT, LINE, MUTED)
    };
    let painter = ui.painter();
    if fill != Color32::TRANSPARENT {
        painter.rect_filled(rect, Rounding::same(2.0), fill);
    }
    painter.rect_stroke(rect, Rounding::same(2.0), Stroke::new(1.0_f32, border));
    let galley = painter.layout_no_wrap(text, font, fg);
    painter.galley(rect.center() - galley.size() * 0.5, galley, fg);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

pub fn hairline(ui: &mut Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0_f32, LINE));
}

pub fn kicker(ui: &mut Ui, text: &str) {
    ui.label(kicker_text(text));
}

pub fn dot(ui: &mut Ui, filled: bool, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
    let center = rect.center();
    if filled {
        ui.painter().circle_filled(center, 2.8, color);
    } else {
        ui.painter()
            .circle_stroke(center, 2.8, Stroke::new(1.0_f32, color));
    }
}

pub fn progress_bar(ui: &mut Ui, fraction: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 3.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::ZERO, LINE);
    let clamped = fraction.clamp(0.0, 1.0);
    if clamped > 0.0 {
        let filled = egui::Rect::from_min_size(
            rect.left_top(),
            Vec2::new(rect.width() * clamped, rect.height()),
        );
        painter.rect_filled(filled, Rounding::ZERO, WHITE);
    }
}

/// Bordered card container with a hairline stroke and 18px padding.
pub fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(PANEL)
        .stroke(Stroke::new(1.0_f32, LINE))
        .rounding(Rounding::same(2.0))
        .inner_margin(egui::Margin::symmetric(18.0, 16.0))
        .show(ui, add);
}

pub fn label(ui: &mut Ui, text: &str, font: egui::FontId, color: Color32) {
    ui.label(egui::RichText::new(text).font(font).color(color));
}

pub fn wrapped(ui: &mut Ui, text: &str, font: egui::FontId, color: Color32) {
    ui.add(egui::Label::new(egui::RichText::new(text).font(font).color(color)).wrap());
}

pub fn text_field(ui: &mut Ui, value: &mut String, hint: &str, password: bool) -> Response {
    let width = ui.available_width();
    text_field_width(ui, value, hint, password, width)
}

pub fn text_field_width(
    ui: &mut Ui,
    value: &mut String,
    hint: &str,
    password: bool,
    width: f32,
) -> Response {
    let height = 34.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(2.0), RAISED);
    let inner = rect.shrink2(Vec2::new(10.0, 0.0));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let response = child.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .font(sans(12.5))
            .frame(false)
            .password(password)
            .desired_width(f32::INFINITY),
    );
    let border = if response.has_focus() {
        WHITE
    } else if response.hovered() {
        LINE_STRONG
    } else {
        LINE
    };
    ui.painter()
        .rect_stroke(rect, Rounding::same(2.0), Stroke::new(1.0_f32, border));
    response
}

pub fn prompt_field(ui: &mut Ui, value: &mut String, hint: &str, height: f32) -> Response {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(2.0), RAISED);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(14.0, 12.0)))
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_width(rect.width() - 28.0);
    let response = child.add(
        egui::TextEdit::multiline(value)
            .hint_text(hint)
            .font(sans(13.5))
            .frame(false)
            .desired_width(f32::INFINITY)
            .desired_rows(2),
    );
    let border = if response.has_focus() { WHITE } else { LINE };
    ui.painter()
        .rect_stroke(rect, Rounding::same(2.0), Stroke::new(1.0_f32, border));
    response
}

pub fn empty_state(ui: &mut Ui, title: &str, body: &str) {
    ui.add_space(56.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(title).font(semibold(19.0)).color(TEXT));
        ui.add_space(6.0);
        ui.label(egui::RichText::new(body).font(sans(12.5)).color(MUTED));
    });
}

pub fn status_line(ui: &mut Ui, ok: Option<bool>, text: &str) {
    ui.horizontal(|ui| {
        match ok {
            Some(true) => dot(ui, true, WHITE),
            Some(false) => dot(ui, false, MUTED),
            None => dot(ui, false, DIM),
        }
        label(ui, text, sans(11.5), MUTED);
    });
}

pub fn kv_row(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(112.0, 18.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                label(ui, &key.to_uppercase(), medium(9.0), DIM);
            },
        );
        wrapped(ui, value, sans(12.0), TEXT);
    });
    ui.add_space(4.0);
}

/// Text pushed to the right edge of the current row.
pub fn align_right(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.with_layout(Layout::right_to_left(Align::Center), add);
}

pub fn tag(ui: &mut Ui, text: &str) {
    let font = mono(9.5);
    let pad = Vec2::new(7.0, 3.0);
    let measured = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), MUTED)
        .size();
    let (rect, _) = ui.allocate_exact_size(measured + pad * 2.0, Sense::hover());
    ui.painter()
        .rect_stroke(rect, Rounding::same(1.0), Stroke::new(1.0_f32, LINE));
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, MUTED);
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, MUTED);
}

pub fn note(ui: &mut Ui, text: &str) {
    wrapped(ui, text, sans(11.5), DIM);
}

pub fn hover_row(ui: &mut Ui, height: f32) -> (egui::Rect, Response) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, Rounding::ZERO, Color32::from_rgb(15, 15, 15));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    (rect, response)
}

pub fn field_label(ui: &mut Ui, text: &str) {
    kicker(ui, text);
    ui.add_space(5.0);
}

pub fn rule_with_label(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        kicker(ui, text);
        ui.add_space(6.0);
        let remaining = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(remaining, 1.0), Sense::hover());
        ui.painter()
            .hline(rect.x_range(), rect.center().y, Stroke::new(1.0_f32, LINE));
    });
}

pub fn inline_note(ui: &mut Ui, ok: bool, text: &str) {
    ui.horizontal_top(|ui| {
        dot(ui, ok, if ok { WHITE } else { MUTED });
        wrapped(ui, text, sans(11.5), MUTED);
    });
}

pub fn big_title(ui: &mut Ui, text: &str) {
    label(ui, text, semibold(29.0), TEXT);
}

pub fn subheading(ui: &mut Ui, text: &str) {
    label(ui, text, medium(12.5), TEXT);
}

pub fn body(ui: &mut Ui, text: &str) {
    wrapped(ui, text, sans(13.0), MUTED);
}

pub fn body_tight(ui: &mut Ui, text: &str) {
    wrapped(ui, text, sans(12.0), MUTED);
}

pub fn quiet_text(ui: &mut Ui, text: &str) {
    label(ui, text, sans(11.5), DIM);
}

pub fn tiny(ui: &mut Ui, text: &str) {
    label(ui, text, mono(9.5), DIM);
}

pub fn error_card(ui: &mut Ui, message: &str, dismiss: &mut bool) {
    egui::Frame::none()
        .fill(BG)
        .stroke(Stroke::new(1.0_f32, LINE_STRONG))
        .rounding(Rounding::same(2.0))
        .inner_margin(egui::Margin::symmetric(16.0, 14.0))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    kicker(ui, "Error");
                    ui.add_space(4.0);
                    wrapped(ui, message, sans(12.0), TEXT);
                });
                align_right(ui, |ui| {
                    if button(ui, "Dismiss", ButtonKind::Quiet).clicked() {
                        *dismiss = true;
                    }
                });
            });
        });
}
