use eframe::egui::{self, Align2};
use egui::pos2;

use crate::app::{self, App};
use crate::theme::*;
use crate::widgets::{self, ButtonKind};

const GENRES: [(&str, &str); 6] = [
    (
        "Tycoon",
        "A tycoon where players mine crystals, buy better drills and rebuild a neon city on the cliff.",
    ),
    (
        "Obby",
        "A precision obby with moving platforms, checkpoints, a timer and a fastest-run leaderboard.",
    ),
    (
        "Simulator",
        "A pet simulator where players hatch eggs, train pets, explore three biomes and trade rewards.",
    ),
    (
        "PvP",
        "A round-based arena PvP game with pickable classes, cooldown abilities and a team scoreboard.",
    ),
    (
        "Horror",
        "A co-op horror game: find four fuses with a flashlight while a creature hunts the corridors.",
    ),
    (
        "Racing",
        "A kart racing game with checkpoints, three laps, boost pads and a finish leaderboard.",
    ),
];

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(2.0);
            widgets::kicker(ui, "AI game studio for Roblox");
            ui.add_space(10.0);
            widgets::big_title(ui, "One sentence in. A playable Roblox game out.");
            ui.add_space(10.0);
            widgets::body(
                ui,
                "BloxCode plans the systems, writes the Luau, packages a Rojo project and syncs it into Roblox Studio.",
            );
            ui.add_space(24.0);

            composer(app, ui);

            ui.add_space(30.0);
            widgets::rule_with_label(ui, "Projects");
            ui.add_space(8.0);
            projects(app, ui);
            ui.add_space(24.0);
        });
}

fn composer(app: &mut App, ui: &mut egui::Ui) {
    widgets::card(ui, |ui| {
        widgets::kicker(ui, "New game");
        ui.add_space(9.0);
        let response = widgets::prompt_field(
            ui,
            &mut app.idea,
            "A co-op mining game where players dig deeper for rarer ore, upgrade drills and defend the mine at night.",
            116.0,
        );
        if app.focus_idea {
            response.request_focus();
            app.focus_idea = false;
        }

        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            for (index, (name, seed)) in GENRES.iter().enumerate() {
                if widgets::chip(ui, name, app.genre == Some(index)).clicked() {
                    app.genre = Some(index);
                    let empty = app.idea.trim().is_empty();
                    let is_seed = GENRES
                        .iter()
                        .any(|(_, existing)| app.idea.trim() == existing.trim());
                    if empty || is_seed {
                        app.idea = (*seed).to_string();
                    }
                }
            }
        });

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            widgets::quiet_text(
                ui,
                "Takes about a minute. Nothing is uploaded except your prompt.",
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ready = app.idea.trim().chars().count() >= 12 && app.gen.is_none();
                let clicked = ui
                    .add_enabled_ui(ready, |ui| {
                        widgets::button(ui, "Generate game", ButtonKind::Primary)
                    })
                    .inner
                    .clicked();
                if clicked {
                    app.start_generation();
                }
            });
        });
    });
}

fn projects(app: &mut App, ui: &mut egui::Ui) {
    if app.projects.is_empty() {
        widgets::empty_state(
            ui,
            "No games yet",
            "Describe one above and press Generate game.",
        );
        return;
    }

    let mut open: Option<usize> = None;
    for (index, loaded) in app.projects.iter().enumerate() {
        let (rect, response) = widgets::hover_row(ui, 66.0);
        let painter = ui.painter();
        painter.text(
            pos2(rect.left() + 2.0, rect.top() + 13.0),
            Align2::LEFT_TOP,
            &loaded.project.title,
            semibold(14.5),
            TEXT,
        );
        painter.text(
            pos2(rect.left() + 2.0, rect.top() + 37.0),
            Align2::LEFT_TOP,
            bloxcode_core::util::truncate(&loaded.project.pitch, 105),
            sans(11.5),
            MUTED,
        );
        painter.text(
            pos2(rect.right() - 2.0, rect.center().y),
            Align2::RIGHT_CENTER,
            app::project_age(&loaded.project.updated_at),
            mono(10.0),
            DIM,
        );
        if response.clicked() {
            open = Some(index);
        }
        ui.add_space(2.0);
        widgets::hairline(ui);
    }

    if let Some(index) = open {
        if let Some(loaded) = app.projects.get(index).cloned() {
            app.open_project(loaded);
        }
    }

    ui.add_space(10.0);
    widgets::quiet_text(
        ui,
        &format!(
            "{} project(s) in {}",
            app.projects.len(),
            app.config.projects_root
        ),
    );
    widgets::align_right(ui, |ui| {
        if widgets::button(ui, "Open projects folder", ButtonKind::Quiet).clicked() {
            let root = app.config.projects_root_path();
            let _ = std::fs::create_dir_all(&root);
            app::open_path(&root);
        }
    });
}
