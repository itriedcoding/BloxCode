use std::collections::BTreeMap;

use eframe::egui::{self, Align, Align2, Color32, Layout, Margin, Sense, Stroke, Vec2};
use egui::{pos2, vec2};

use bloxcode_core::plan::FileRole;

use crate::app::{App, Screen};
use crate::editor;
use crate::theme::*;
use crate::widgets::{self, ButtonKind};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    if app.session.is_none() {
        widgets::empty_state(
            ui,
            "No project open",
            "Generate a game on Home or open one from the project list.",
        );
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            if widgets::button(ui, "Go to Home", ButtonKind::Ghost).clicked() {
                app.screen = Screen::Home;
            }
        });
        return;
    }

    toolbar(app, ui);
    ui.add_space(10.0);

    let height = ui.available_height();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(vec2(248.0, height), Layout::top_down(Align::Min), |ui| {
            tree(app, ui)
        });
        ui.add_space(14.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), height),
            Layout::top_down(Align::Min),
            |ui| editor_pane(app, ui),
        );
    });
}

fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    let (path, dirty, file_count) = {
        let Some(session) = app.session.as_ref() else {
            return;
        };
        (
            session.selected.clone().unwrap_or_default(),
            session
                .selected
                .as_ref()
                .map(|path| session.is_dirty(path))
                .unwrap_or(false),
            session.snapshot.file_count(),
        )
    };

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            widgets::label(
                ui,
                if path.is_empty() {
                    "No file selected"
                } else {
                    &path
                },
                mono(11.5),
                TEXT,
            );
            ui.add_space(2.0);
            widgets::tiny(ui, &format!("{file_count} FILES · CTRL+S SAVES"));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::button(ui, "Save all", ButtonKind::Ghost).clicked() {
                app.save_all();
            }
            ui.add_space(6.0);
            let clicked = ui
                .add_enabled_ui(dirty, |ui| widgets::button(ui, "Save", ButtonKind::Primary))
                .inner
                .clicked();
            if clicked {
                app.save_current();
            }
            if dirty {
                ui.add_space(8.0);
                widgets::tag(ui, "Unsaved");
            }
            if widgets::button(ui, "Ask AI", ButtonKind::Quiet).clicked() {
                app.screen = Screen::Build;
                app.scope_to_file = true;
            }
        });
    });
    ui.add_space(8.0);
    widgets::hairline(ui);
}

fn tree(app: &mut App, ui: &mut egui::Ui) {
    let (files, dirty_set, selected) = {
        let Some(session) = app.session.as_ref() else {
            return;
        };
        (
            session
                .snapshot
                .files
                .iter()
                .map(|file| {
                    (
                        file.path.clone(),
                        FileRole::from_path(&file.path),
                        file.bytes,
                    )
                })
                .collect::<Vec<_>>(),
            session.dirty.clone(),
            session.selected.clone(),
        )
    };

    let grouped: BTreeMap<u8, Vec<(String, String, usize)>> = {
        let mut map: BTreeMap<u8, Vec<(String, String, usize)>> = BTreeMap::new();
        for (path, role, bytes) in files {
            let role = role.unwrap_or(FileRole::Shared);
            let relative = path
                .strip_prefix(role.dir())
                .unwrap_or(path.as_str())
                .to_string();
            let index = match role {
                FileRole::Shared => 0,
                FileRole::Server => 1,
                FileRole::Client => 2,
            };
            map.entry(index).or_default().push((path, relative, bytes));
        }
        map
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let headings = ["Shared", "Server", "Client"];
            let mut clicked: Option<String> = None;

            for (index, heading) in headings.iter().enumerate() {
                let Some(entries) = grouped.get(&(index as u8)) else {
                    continue;
                };
                ui.add_space(6.0);
                widgets::kicker(ui, heading);
                ui.add_space(4.0);
                for (path, relative, bytes) in entries {
                    let is_selected = selected.as_deref() == Some(path.as_str());
                    let is_dirty = dirty_set.contains(path);
                    if tree_row(ui, relative, is_selected, is_dirty).clicked() {
                        clicked = Some(path.clone());
                    }
                    if is_selected {
                        ui.add_space(1.0);
                        widgets::tiny(ui, &bloxcode_core::util::human_bytes(*bytes));
                        ui.add_space(2.0);
                    }
                }
                ui.add_space(8.0);
            }

            if let Some(path) = clicked {
                let result = app
                    .session
                    .as_mut()
                    .map(|session| session.select(&path))
                    .unwrap_or(Ok(()));
                if let Err(error) = result {
                    app.error = Some(format!("{error:#}"));
                }
            }
        });
}

fn tree_row(ui: &mut egui::Ui, label: &str, selected: bool, dirty: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, egui::Rounding::ZERO, RAISED);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.left_top(), Vec2::new(2.0, rect.height())),
            egui::Rounding::ZERO,
            WHITE,
        );
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::Rounding::ZERO, Color32::from_rgb(15, 15, 15));
    }
    let color = if selected { TEXT } else { MUTED };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), mono(11.0), color);
    ui.painter().galley(
        pos2(rect.left() + 12.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
    if dirty {
        ui.painter()
            .circle_filled(pos2(rect.right() - 12.0, rect.center().y), 2.0, WHITE);
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn editor_pane(app: &mut App, ui: &mut egui::Ui) {
    let (path, class, target, snapshot_content) = {
        let Some(session) = app.session.as_ref() else {
            return;
        };
        let Some(path) = session.selected.clone() else {
            return;
        };
        let file = session.snapshot.files.iter().find(|file| file.path == path);
        (
            path.clone(),
            file.map(|file| file.class.clone()).unwrap_or_default(),
            file.map(|file| file.target.clone()).unwrap_or_default(),
            session.snapshot.source(&path).cloned().unwrap_or_default(),
        )
    };

    let mut text = app
        .session
        .as_ref()
        .and_then(|session| session.drafts.get(&path).cloned())
        .unwrap_or_default();
    let dirty = app
        .session
        .as_ref()
        .map(|session| session.is_dirty(&path))
        .unwrap_or(false);

    ui.horizontal(|ui| {
        widgets::tiny(ui, &format!("{class} · {target}"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            widgets::tiny(
                ui,
                &format!(
                    "{} LINES · {}",
                    text.lines().count(),
                    bloxcode_core::util::human_bytes(text.len())
                ),
            );
        });
    });
    ui.add_space(6.0);

    let row_height = editor::row_height(ui, &mono(12.5));
    let lines = text.lines().count().max(1);

    let frame = egui::Frame::none()
        .fill(BG)
        .stroke(Stroke::new(1.0_f32, LINE))
        .rounding(egui::Rounding::same(2.0));
    frame.show(ui, |ui| {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    gutter(ui, lines, row_height);
                    let font = mono(12.5);
                    let mut layouter = |ui: &egui::Ui, text: &str, _wrap_width: f32| {
                        let job = editor::highlight(text, font.clone());
                        ui.fonts(|fonts| fonts.layout_job(job))
                    };
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .font(font.clone())
                            .code_editor()
                            .frame(false)
                            .desired_width(1600.0)
                            .desired_rows(lines.max(22))
                            .margin(Margin::symmetric(12.0, 8.0))
                            .layouter(&mut layouter),
                    );
                    if response.changed() {
                        if let Some(session) = app.session.as_mut() {
                            session.drafts.insert(path.clone(), text.clone());
                            if text != snapshot_content {
                                session.dirty.insert(path.clone());
                            } else {
                                session.dirty.remove(&path);
                            }
                        }
                    }
                });
            });
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        widgets::quiet_text(ui, "Changes stay local until you press Save.");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::button(ui, "Open project folder", ButtonKind::Quiet).clicked() {
                if let Some(session) = &app.session {
                    crate::app::open_path(&session.loaded.dir);
                }
            }
            if dirty {
                widgets::label(ui, "Unsaved changes", sans(10.5), MUTED);
            }
        });
    });
}

fn gutter(ui: &mut egui::Ui, lines: usize, row_height: f32) {
    let width = editor::gutter_width(lines);
    let height = row_height * lines as f32 + 16.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::Rounding::ZERO, PANEL);
    for line in 1..=lines {
        let y = rect.top() + 8.0 + (line as f32 - 1.0) * row_height;
        painter.text(
            pos2(rect.right() - 10.0, y),
            Align2::RIGHT_TOP,
            line.to_string(),
            mono(12.5),
            DIM,
        );
    }
    painter.vline(rect.right(), rect.y_range(), Stroke::new(1.0_f32, LINE));
}
