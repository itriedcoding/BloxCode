use eframe::egui::{self, Align, Layout};

use crate::app::{self, App};
use crate::theme::*;
use crate::widgets::{self, ButtonKind};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            widgets::kicker(ui, "Studio");
            ui.add_space(8.0);
            widgets::big_title(ui, "Roblox Studio integration");
            ui.add_space(6.0);
            widgets::body(
                ui,
                "BloxCode runs a loopback bridge. The Studio plugin pulls the open build straight into your place — no copying scripts by hand.",
            );
            ui.add_space(18.0);

            bridge_card(app, ui);
            ui.add_space(14.0);
            plugin_card(app, ui);
            ui.add_space(14.0);
            rojo_card(app, ui);
            ui.add_space(22.0);
            steps(ui);
            ui.add_space(24.0);
        });
}

fn bridge_card(app: &mut App, ui: &mut egui::Ui) {
    let (state, label) = app::connection_state(app);
    let endpoint = app::bridge_display(app);
    let requests = app
        .bridge_stats
        .as_ref()
        .map(|stats| stats.request_count())
        .unwrap_or(0);
    let last_seen = app
        .bridge_stats
        .as_ref()
        .and_then(|stats| stats.seconds_since_seen());
    let project_live = app.session.is_some();
    let error = app.bridge_error.clone();

    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::kicker(ui, "Local bridge");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::status_line(ui, state, &label);
            });
        });
        ui.add_space(10.0);
        widgets::kv_row(ui, "Endpoint", &endpoint);
        widgets::kv_row(
            ui,
            "Project",
            if project_live {
                "published to bridge"
            } else {
                "no project open"
            },
        );
        widgets::kv_row(ui, "Requests", &requests.to_string());
        widgets::kv_row(
            ui,
            "Last ack",
            &last_seen
                .map(|seconds| format!("{seconds}s ago"))
                .unwrap_or_else(|| "never".to_string()),
        );
        if let Some(error) = &error {
            ui.add_space(8.0);
            widgets::inline_note(ui, false, error);
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if widgets::button(ui, "Restart bridge", ButtonKind::Ghost).clicked() {
                app.start_bridge();
            }
            if widgets::button(ui, "Open settings", ButtonKind::Quiet).clicked() {
                app.screen = app::Screen::Settings;
            }
        });
    });
}

fn plugin_card(app: &mut App, ui: &mut egui::Ui) {
    let (installed, path) = app::plugin_state(app);
    let plugins_dir = bloxcode_core::studio::plugins_dir();

    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::kicker(ui, "Studio plugin");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::status_line(
                    ui,
                    Some(installed),
                    if installed {
                        "Installed"
                    } else {
                        "Not installed"
                    },
                );
            });
        });
        ui.add_space(10.0);
        widgets::kv_row(ui, "Plugin file", &path);
        widgets::kv_row(ui, "Folder", &plugins_dir.display().to_string());
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if widgets::button(
                ui,
                if installed {
                    "Reinstall plugin"
                } else {
                    "Install plugin"
                },
                ButtonKind::Primary,
            )
            .clicked()
            {
                app.install_plugin();
            }
            if widgets::button(ui, "Open plugins folder", ButtonKind::Ghost).clicked() {
                let _ = std::fs::create_dir_all(&plugins_dir);
                app::open_path(&plugins_dir);
            }
        });
        ui.add_space(10.0);
        widgets::note(
            ui,
            "The plugin embeds this install's bridge token. Reinstall it after rotating the token or changing the port.",
        );
    });
}

fn rojo_card(app: &mut App, ui: &mut egui::Ui) {
    let cli = app.rojo_cli.clone();
    let version = app.rojo_version.clone();
    let has_session = app.session.is_some();
    let build_result = app.build_result.clone();
    let building = app.build_job.is_some();

    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::kicker(ui, "Rojo");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::status_line(
                    ui,
                    Some(cli.is_some()),
                    if cli.is_some() {
                        "Available"
                    } else {
                        "Not found"
                    },
                );
                if let Some(version) = &version {
                    widgets::tiny(ui, version);
                }
            });
        });
        ui.add_space(10.0);
        widgets::kv_row(
            ui,
            "CLI",
            &cli.as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "not found on PATH".to_string()),
        );
        widgets::kv_row(ui, "Place file", "build/<project>.rbxlx");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let enabled = has_session && cli.is_some() && !building;
            let clicked = ui
                .add_enabled_ui(enabled, |ui| {
                    widgets::button(ui, "Build .rbxlx", ButtonKind::Primary)
                })
                .inner
                .clicked();
            if clicked {
                app.build_place();
            }
            if widgets::button(ui, "Recheck tools", ButtonKind::Ghost).clicked() {
                app.refresh_tools();
            }
            if has_session && widgets::button(ui, "Open build folder", ButtonKind::Quiet).clicked()
            {
                if let Some(session) = &app.session {
                    let dir = session.loaded.dir.join("build");
                    let _ = std::fs::create_dir_all(&dir);
                    app::open_path(&dir);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if building {
                    widgets::tiny(ui, "BUILDING");
                }
            });
        });

        if let Some(result) = &build_result {
            ui.add_space(10.0);
            match result {
                Ok(path) => widgets::inline_note(ui, true, &format!("Built {path}")),
                Err(error) => widgets::inline_note(ui, false, error),
            }
        }

        if cli.is_none() {
            ui.add_space(10.0);
            widgets::note(
                ui,
                "Install Rojo with rokit (rokit add rojo-rbx/rojo) or aftman, then press recheck. Live Studio sync works without Rojo.",
            );
        }
    });
}

fn steps(ui: &mut egui::Ui) {
    widgets::rule_with_label(ui, "Use it in Studio");
    ui.add_space(10.0);
    let steps = [
        (
            "01",
            "Install the plugin",
            "Press Install plugin above. Studio loads it from the local plugins folder on next start.",
        ),
        (
            "02",
            "Allow HTTP requests",
            "In Studio: Game Settings → Security → Allow HTTP Requests. The bridge only listens on 127.0.0.1.",
        ),
        (
            "03",
            "Sync",
            "Open the BloxCode toolbar button, press SYNC NOW. Scripts appear under ServerScriptService, ReplicatedStorage and StarterPlayer.",
        ),
        (
            "04",
            "Iterate",
            "Ask for a change in Build. Save, then sync again — BloxCode replaces only the scripts it manages.",
        ),
    ];
    for (number, title, detail) in steps {
        ui.horizontal_top(|ui| {
            widgets::label(ui, number, mono(10.0), DIM);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                widgets::subheading(ui, title);
                ui.add_space(2.0);
                widgets::body_tight(ui, detail);
            });
        });
        ui.add_space(10.0);
    }
}
