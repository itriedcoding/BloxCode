use eframe::egui::{self, Align, Layout};

use bloxcode_core::ai::models::{self, ModelInfo};
use bloxcode_core::config::{DEFAULT_BASE_URL, DEFAULT_MODEL, DEFAULT_PORT};
use bloxcode_core::{paths, util};

use crate::app::{self, App};
use crate::theme::*;
use crate::widgets::{self, ButtonKind};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            widgets::kicker(ui, "Settings");
            ui.add_space(8.0);
            widgets::big_title(ui, "Configuration");
            ui.add_space(6.0);
            widgets::body(ui, "Stored locally in %APPDATA%\\BloxCode\\config.json.");
            ui.add_space(18.0);

            vly_card(app, ui);
            ui.add_space(14.0);
            bridge_card(app, ui);
            ui.add_space(14.0);
            paths_card(app, ui);
            ui.add_space(14.0);
            about_card(app, ui);

            ui.add_space(20.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, "Save settings", ButtonKind::Primary).clicked() {
                    app.save_settings();
                }
                if widgets::button(ui, "Reset to defaults", ButtonKind::Quiet).clicked() {
                    app.settings.base_url = DEFAULT_BASE_URL.to_string();
                    app.settings.model = DEFAULT_MODEL.to_string();
                    app.settings.temperature = 0.4;
                    app.port_text = DEFAULT_PORT.to_string();
                    app.settings.bridge_port = DEFAULT_PORT;
                }
            });
            ui.add_space(30.0);
        });
}

fn vly_card(app: &mut App, ui: &mut egui::Ui) {
    let token_source = if app.config.effective_token().is_empty() {
        "missing"
    } else if std::env::var("VLY_INTEGRATION_KEY").is_ok() || std::env::var("VLY_API_KEY").is_ok() {
        "environment override"
    } else {
        "stored locally"
    };

    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::kicker(ui, "Vly AI");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::tiny(ui, token_source.to_uppercase().as_str());
            });
        });
        ui.add_space(10.0);
        widgets::note(
            ui,
            "Vly exposes an OpenAI-compatible gateway at integrations.vly.ai. Create a deployment token in the Vly dashboard.",
        );
        ui.add_space(12.0);

        widgets::field_label(ui, "Deployment token");
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 110.0).max(220.0);
            widgets::text_field_width(
                ui,
                &mut app.settings.vly_token,
                "vly_live_…",
                !app.token_visible,
                width,
            );
            if widgets::chip(ui, if app.token_visible { "Hide" } else { "Show" }, false).clicked() {
                app.token_visible = !app.token_visible;
            }
        });
        ui.add_space(10.0);

        model_picker(app, ui);
        ui.add_space(10.0);

        widgets::field_label(ui, "Gateway base URL");
        widgets::text_field(ui, &mut app.settings.base_url, DEFAULT_BASE_URL, false);
        ui.add_space(10.0);

        widgets::field_label(ui, "Reasoning temperature");
        ui.horizontal(|ui| {
            for (label, value) in [("Precise", 0.2f32), ("Balanced", 0.4), ("Creative", 0.7)] {
                let selected = (app.settings.temperature - value).abs() < 0.01;
                if widgets::chip(ui, label, selected).clicked() {
                    app.settings.temperature = value;
                }
            }
        });

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            let testing = app.connect_test.is_some();
            let clicked = ui
                .add_enabled_ui(!testing, |ui| {
                    widgets::button(ui, "Test connection", ButtonKind::Ghost)
                })
                .inner
                .clicked();
            if clicked {
                app.test_connection();
            }
            if testing {
                widgets::tiny(ui, "TESTING");
            }
        });
        if let Some(result) = &app.connect_result {
            ui.add_space(8.0);
            match result {
                Ok(message) => {
                    widgets::inline_note(ui, true, &format!("Gateway responded: {message}"))
                }
                Err(error) => widgets::inline_note(ui, false, error),
            }
        }
    });
}

/// Editable model id plus a searchable browser over every text/coding model.
fn model_picker(app: &mut App, ui: &mut egui::Ui) {
    widgets::field_label(ui, "Model");
    let loading = app.models_job.is_some();
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 168.0).max(220.0);
        widgets::text_field_width(ui, &mut app.settings.model, DEFAULT_MODEL, false, width);
        let refresh = ui
            .add_enabled_ui(!loading, |ui| {
                widgets::button(ui, "Refresh models", ButtonKind::Ghost)
            })
            .inner
            .clicked();
        if refresh {
            app.fetch_models();
        }
        if loading {
            widgets::tiny(ui, "LOADING");
        }
    });
    ui.add_space(6.0);
    if let Some(note) = &app.models_note {
        match note {
            Ok(text) => widgets::inline_note(ui, true, text),
            Err(error) => widgets::inline_note(ui, false, error),
        }
        ui.add_space(6.0);
    }

    widgets::field_label(ui, "Browse models");
    widgets::text_field(
        ui,
        &mut app.model_search,
        "Search all text and coding models…",
        false,
    );
    ui.add_space(6.0);

    let query = app.model_search.trim().to_ascii_lowercase();
    let shown: Vec<ModelInfo> = if query.is_empty() {
        let recommended: Vec<ModelInfo> = app
            .models
            .iter()
            .filter(|model| models::is_recommended(&model.id))
            .cloned()
            .collect();
        if recommended.is_empty() {
            app.models.iter().take(24).cloned().collect()
        } else {
            recommended
        }
    } else {
        app.models
            .iter()
            .filter(|model| model.matches(&query))
            .take(80)
            .cloned()
            .collect()
    };
    let total = app.models.len();
    ui.horizontal(|ui| {
        widgets::tiny(ui, &format!("{total} MODELS"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if query.is_empty() {
                widgets::tiny(ui, "RECOMMENDED · TYPE TO SEARCH");
            } else {
                widgets::tiny(ui, &format!("{} MATCHES", shown.len()));
            }
        });
    });
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .max_height(208.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if shown.is_empty() {
                widgets::quiet_text(ui, "No models match this search.");
            }
            for model in &shown {
                let selected = app.settings.model == model.id;
                let (rect, response) = widgets::hover_row(ui, 30.0);
                if selected {
                    ui.painter().rect_filled(rect, egui::Rounding::ZERO, RAISED);
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height())),
                        egui::Rounding::ZERO,
                        WHITE,
                    );
                }
                let color = if selected { TEXT } else { MUTED };
                let name = ui
                    .painter()
                    .layout_no_wrap(model.name.clone(), medium(11.5), color);
                ui.painter().galley(
                    egui::pos2(rect.left() + 12.0, rect.center().y - name.size().y * 0.5),
                    name,
                    color,
                );
                let id = ui
                    .painter()
                    .layout_no_wrap(model.id.clone(), mono(9.5), DIM);
                ui.painter().galley(
                    egui::pos2(
                        rect.right() - 12.0 - id.size().x,
                        rect.center().y - id.size().y * 0.5,
                    ),
                    id,
                    DIM,
                );
                if response.clicked() {
                    app.settings.model = model.id.clone();
                }
            }
        });
    ui.add_space(6.0);
    widgets::note(
        ui,
        "Pick any model the gateway exposes, or type an exact id. Reasoning models retry without a custom temperature when the provider rejects it.",
    );
}

fn bridge_card(app: &mut App, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        widgets::kicker(ui, "Studio bridge");
        ui.add_space(10.0);
        widgets::note(
            ui,
            "Loopback only. The token below is embedded into the Studio plugin you install.",
        );
        ui.add_space(12.0);

        widgets::field_label(ui, "Port");
        ui.horizontal(|ui| {
            widgets::text_field_width(ui, &mut app.port_text, "42811", false, 120.0);
            widgets::tiny(ui, "1024–65535 · restarts on save");
        });
        ui.add_space(10.0);

        widgets::field_label(ui, "Bridge token");
        ui.horizontal(|ui| {
            let masked = format!(
                "{}…{}",
                &app.settings.bridge_token[..6.min(app.settings.bridge_token.len())],
                &app.settings.bridge_token[app.settings.bridge_token.len().saturating_sub(4)..]
            );
            widgets::label(ui, &masked, mono(11.5), MUTED);
            if widgets::button(ui, "Rotate", ButtonKind::Quiet).clicked() {
                app.settings.bridge_token = util::random_hex(24);
                app.status = "New bridge token generated. Save to apply.".into();
            }
        });
        ui.add_space(10.0);
        widgets::inline_note(
            ui,
            true,
            "Rotating the token requires reinstalling the Studio plugin (same Save applies it automatically).",
        );
    });
}

fn paths_card(app: &mut App, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        widgets::kicker(ui, "Paths");
        ui.add_space(10.0);

        widgets::field_label(ui, "Projects folder");
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 130.0).max(220.0);
            widgets::text_field_width(ui, &mut app.settings.projects_root, "", false, width);
            if widgets::button(ui, "Open", ButtonKind::Ghost).clicked() {
                let root = std::path::PathBuf::from(app.settings.projects_root.trim());
                let _ = std::fs::create_dir_all(&root);
                app::open_path(&root);
            }
        });
        ui.add_space(10.0);

        widgets::field_label(ui, "Rojo CLI (optional)");
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 130.0).max(220.0);
            widgets::text_field_width(
                ui,
                &mut app.settings.rojo_path,
                "auto-detected on PATH",
                false,
                width,
            );
            if widgets::button(ui, "Detect", ButtonKind::Ghost).clicked() {
                app.refresh_tools();
            }
        });
        ui.add_space(10.0);

        widgets::kv_row(
            ui,
            "Config file",
            &paths::config_file().display().to_string(),
        );
        widgets::kv_row(
            ui,
            "Plugins",
            &bloxcode_core::studio::plugins_dir().display().to_string(),
        );
    });
}

fn about_card(app: &mut App, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        widgets::kicker(ui, "About");
        ui.add_space(10.0);
        widgets::kv_row(ui, "Version", env!("CARGO_PKG_VERSION"));
        widgets::kv_row(ui, "Model", &app.config.model);
        widgets::kv_row(ui, "Renderer", "egui / glow");
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if widgets::button(ui, "Source on GitHub", ButtonKind::Ghost).clicked() {
                app::open_url("https://github.com/itriedcoding/BloxCode");
            }
            if widgets::button(ui, "Open config folder", ButtonKind::Quiet).clicked() {
                let dir = paths::app_dir();
                let _ = std::fs::create_dir_all(&dir);
                app::open_path(&dir);
            }
        });
    });
}
