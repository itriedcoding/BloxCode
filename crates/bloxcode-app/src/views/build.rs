use eframe::egui::{self, Align, Layout};

use crate::app::{self, App, Screen};
use crate::jobs::GenKind;
use crate::theme::*;
use crate::widgets::{self, ButtonKind};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if app.gen.is_some() {
                generation(app, ui);
                return;
            }
            if app.session.is_none() {
                widgets::empty_state(
                    ui,
                    "Nothing to build yet",
                    "Describe a game on Home, or open an existing project.",
                );
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    if widgets::button(ui, "Go to Home", ButtonKind::Ghost).clicked() {
                        app.screen = Screen::Home;
                    }
                });
                return;
            }
            project(app, ui);
        });
}

fn generation(app: &mut App, ui: &mut egui::Ui) {
    let (
        kind,
        fraction,
        stages,
        log,
        plan_title,
        plan_systems,
        files_written,
        elapsed,
        cancelling,
        total,
    ) = {
        let Some(gen) = app.gen.as_ref() else { return };
        (
            gen.kind,
            gen.fraction(),
            gen.stages
                .iter()
                .map(|stage| (stage.label.clone(), stage.done))
                .collect::<Vec<_>>(),
            gen.log.clone(),
            gen.plan.as_ref().map(|plan| plan.title.clone()),
            gen.plan
                .as_ref()
                .map(|plan| plan.systems.len())
                .unwrap_or(0),
            gen.files_written,
            gen.elapsed_secs(),
            gen.cancelling,
            gen.total,
        )
    };

    widgets::kicker(
        ui,
        if kind == GenKind::NewGame {
            "Generation"
        } else {
            "Revision"
        },
    );
    ui.add_space(8.0);
    widgets::big_title(ui, app::active_generation_title(kind));
    ui.add_space(6.0);
    widgets::label(
        ui,
        &format!(
            "{} · {} · {} of {} stages",
            app.config.model,
            app::elapsed_label(elapsed),
            stages.iter().filter(|(_, done)| *done).count(),
            total.max(stages.len())
        ),
        mono(10.5),
        MUTED,
    );
    ui.add_space(16.0);
    widgets::progress_bar(ui, fraction);
    ui.add_space(18.0);

    for (index, (label, done)) in stages.iter().enumerate() {
        let active = !done && index + 1 == stages.len();
        ui.horizontal(|ui| {
            widgets::dot(
                ui,
                *done,
                if *done {
                    WHITE
                } else if active {
                    LINE_STRONG
                } else {
                    LINE
                },
            );
            let color = if *done || active { TEXT } else { DIM };
            widgets::label(ui, label, sans(12.5), color);
            if active {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::tiny(ui, "IN PROGRESS");
                });
            }
        });
        ui.add_space(2.0);
    }

    if let Some(title) = plan_title {
        ui.add_space(18.0);
        widgets::card(ui, |ui| {
            widgets::kicker(ui, "Blueprint");
            ui.add_space(6.0);
            widgets::label(ui, &title, semibold(16.0), TEXT);
            ui.add_space(4.0);
            widgets::quiet_text(
                ui,
                &format!("{plan_systems} systems · {files_written} files written"),
            );
        });
    }

    if !log.is_empty() {
        ui.add_space(16.0);
        widgets::kicker(ui, "Activity");
        ui.add_space(6.0);
        let start = log.len().saturating_sub(9);
        for line in &log[start..] {
            widgets::label(ui, line, mono(10.5), DIM);
            ui.add_space(1.0);
        }
    }

    ui.add_space(22.0);
    ui.horizontal(|ui| {
        if cancelling {
            widgets::quiet_text(ui, "Cancellation requested.");
        } else if widgets::button(ui, "Cancel", ButtonKind::Ghost).clicked() {
            app.cancel_work();
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if kind == GenKind::Revision {
                widgets::quiet_text(ui, "Applying a targeted change.");
            } else {
                widgets::quiet_text(ui, "Stages stream in as the model writes.");
            }
        });
    });
}

fn project(app: &mut App, ui: &mut egui::Ui) {
    let (title, pitch, systems, remotes, file_count, chat) = {
        let Some(session) = app.session.as_ref() else {
            return;
        };
        (
            session.snapshot.title.clone(),
            session.loaded.project.pitch.clone(),
            session
                .loaded
                .project
                .plan
                .systems
                .iter()
                .map(|system| (system.name.clone(), system.detail.clone()))
                .collect::<Vec<_>>(),
            session
                .loaded
                .project
                .plan
                .remotes
                .iter()
                .map(|remote| (remote.name.clone(), remote.class().to_string()))
                .collect::<Vec<_>>(),
            session.snapshot.file_count(),
            session
                .chat
                .iter()
                .map(|message| (message.role, message.text.clone()))
                .collect::<Vec<_>>(),
        )
    };

    widgets::kicker(ui, "Build");
    ui.add_space(8.0);
    widgets::big_title(ui, &title);
    ui.add_space(6.0);
    widgets::body(ui, &pitch);
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        if widgets::button(ui, "Open files", ButtonKind::Ghost).clicked() {
            app.screen = Screen::Files;
        }
        if widgets::button(ui, "Studio sync", ButtonKind::Ghost).clicked() {
            app.screen = Screen::Studio;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            widgets::tiny(ui, &format!("{file_count} FILES"));
        });
    });

    ui.add_space(20.0);
    widgets::rule_with_label(ui, "Systems");
    ui.add_space(8.0);
    if systems.is_empty() {
        widgets::quiet_text(ui, "No systems recorded.");
    }
    for (name, detail) in &systems {
        ui.horizontal_top(|ui| {
            widgets::dot(ui, true, WHITE);
            ui.vertical(|ui| {
                widgets::subheading(ui, name);
                ui.add_space(1.0);
                widgets::body_tight(ui, detail);
            });
        });
        ui.add_space(6.0);
    }

    if !remotes.is_empty() {
        ui.add_space(12.0);
        widgets::rule_with_label(ui, "Remotes");
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for (name, class) in &remotes {
                widgets::tag(ui, &format!("{name} · {class}"));
            }
        });
    }

    ui.add_space(24.0);
    widgets::rule_with_label(ui, "Iterate with AI");
    ui.add_space(8.0);

    if chat.is_empty() {
        widgets::quiet_text(
            ui,
            "Ask for a change — new mechanics, balance, UI, bug fixes. Only the files that need edits are rewritten.",
        );
        ui.add_space(10.0);
    }

    for (role, text) in &chat {
        widgets::label(
            ui,
            app::chat_role_label(*role).to_uppercase().as_str(),
            medium(9.0),
            app::chat_role_color(*role),
        );
        ui.add_space(3.0);
        widgets::wrapped(ui, text, sans(12.5), TEXT);
        ui.add_space(10.0);
        widgets::hairline(ui);
        ui.add_space(10.0);
    }

    ui.add_space(6.0);
    widgets::card(ui, |ui| {
        widgets::kicker(ui, "Change request");
        ui.add_space(8.0);
        widgets::prompt_field(
            ui,
            &mut app.chat_input,
            "Add a shop that sells three drills, with prices in the shared config and a server-validated purchase.",
            86.0,
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let selected_label = app
                .session
                .as_ref()
                .and_then(|session| session.selected.clone())
                .and_then(|path| path.rsplit('/').next().map(|name| name.to_string()))
                .unwrap_or_else(|| "no file".into());
            if widgets::chip(ui, "This file", app.scope_to_file).clicked() {
                app.scope_to_file = true;
            }
            if widgets::chip(ui, "Whole project", !app.scope_to_file).clicked() {
                app.scope_to_file = false;
            }
            widgets::tiny(ui, &selected_label);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ready = app.chat_input.trim().len() >= 3 && app.gen.is_none();
                let clicked = ui
                    .add_enabled_ui(ready, |ui| {
                        widgets::button(ui, "Send request", ButtonKind::Primary)
                    })
                    .inner
                    .clicked();
                if clicked {
                    app.send_chat();
                    app.chat_input.clear();
                }
            });
        });
    });
    ui.add_space(24.0);
}
