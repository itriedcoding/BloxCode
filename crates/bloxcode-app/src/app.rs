use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bloxcode_core::ai::VlyClient;
use bloxcode_core::bridge::{Bridge, BridgeStats};
use bloxcode_core::config::Config;
use bloxcode_core::project::{self, LoadedProject, Snapshot};
use bloxcode_core::{paths, rojo, studio, util, VERSION};
use eframe::egui;
use egui::{Align, Frame, Layout, Margin, RichText};

use crate::jobs::{self, GenKind, Generation, WorkerMessage};
use crate::theme::{self, *};
use crate::views;
use crate::widgets::{self, ButtonKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Home,
    Build,
    Files,
    Studio,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    Note,
}

pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
}

pub struct Session {
    pub loaded: LoadedProject,
    pub snapshot: Snapshot,
    pub selected: Option<String>,
    pub drafts: BTreeMap<String, String>,
    pub dirty: BTreeSet<String>,
    pub chat: Vec<ChatMessage>,
}

impl Session {
    pub fn open(loaded: LoadedProject) -> anyhow::Result<Self> {
        let snapshot = loaded.snapshot()?;
        let selected = snapshot.files.first().map(|file| file.path.clone());
        let drafts = snapshot
            .contents
            .iter()
            .map(|(path, content)| (path.clone(), content.clone()))
            .collect();
        Ok(Self {
            loaded,
            snapshot,
            selected,
            drafts,
            dirty: BTreeSet::new(),
            chat: Vec::new(),
        })
    }

    /// Re-read metadata and sources from disk, keeping unsaved drafts.
    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.loaded = project::load_project(&self.loaded.dir)?;
        self.snapshot = self.loaded.snapshot()?;
        self.drafts
            .retain(|path, _| self.snapshot.contents.contains_key(path));
        for (path, content) in &self.snapshot.contents {
            if !self.dirty.contains(path) {
                self.drafts.insert(path.clone(), content.clone());
            }
        }
        let selection_valid = self
            .selected
            .as_ref()
            .map(|path| self.snapshot.contents.contains_key(path))
            .unwrap_or(false);
        if !selection_valid {
            self.selected = self.snapshot.files.first().map(|file| file.path.clone());
        }
        Ok(())
    }

    pub fn select(&mut self, path: &str) -> anyhow::Result<()> {
        self.save_current()?;
        self.selected = Some(path.to_string());
        Ok(())
    }

    pub fn is_dirty(&self, path: &str) -> bool {
        self.dirty.contains(path)
    }

    pub fn save_current(&mut self) -> anyhow::Result<bool> {
        let Some(path) = self.selected.clone() else {
            return Ok(false);
        };
        let Some(content) = self.drafts.get(&path).cloned() else {
            return Ok(false);
        };
        if !self.dirty.contains(&path) {
            return Ok(false);
        }
        project::write_script(&self.loaded.dir, &path, &content)?;
        project::touch(&mut self.loaded)?;
        self.dirty.remove(&path);
        self.reload()?;
        Ok(true)
    }

    pub fn save_all(&mut self) -> anyhow::Result<usize> {
        let pending: Vec<(String, String)> = self
            .dirty
            .iter()
            .filter_map(|path| {
                self.drafts
                    .get(path)
                    .map(|content| (path.clone(), content.clone()))
            })
            .collect();
        if pending.is_empty() {
            return Ok(0);
        }
        for (path, content) in &pending {
            project::write_script(&self.loaded.dir, path, content)?;
        }
        self.dirty.clear();
        project::touch(&mut self.loaded)?;
        self.reload()?;
        Ok(pending.len())
    }

    pub fn sources(&self) -> Vec<(String, String)> {
        self.snapshot
            .files
            .iter()
            .filter_map(|file| {
                self.drafts
                    .get(&file.path)
                    .map(|content| (file.path.clone(), content.clone()))
            })
            .collect()
    }

    pub fn push_chat(&mut self, role: ChatRole, text: impl Into<String>) {
        self.chat.push(ChatMessage {
            role,
            text: text.into(),
        });
    }
}

pub struct App {
    pub config: Config,
    pub settings: Config,
    pub port_text: String,
    pub screen: Screen,
    pub projects: Vec<LoadedProject>,
    pub session: Option<Session>,
    pub bridge: Option<Bridge>,
    pub bridge_stats: Option<Arc<BridgeStats>>,
    pub bridge_error: Option<String>,
    pub plugin_installed: Option<PathBuf>,
    pub rojo_cli: Option<PathBuf>,
    pub rojo_version: Option<String>,
    pub idea: String,
    pub genre: Option<usize>,
    pub gen: Option<Generation>,
    pub chat_input: String,
    pub scope_to_file: bool,
    pub token_visible: bool,
    pub connect_test: Option<Receiver<Result<String, String>>>,
    pub connect_result: Option<Result<String, String>>,
    pub build_job: Option<Receiver<Result<String, String>>>,
    pub build_result: Option<Result<String, String>>,
    pub status: String,
    pub error: Option<String>,
    pub focus_idea: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::bootstrap(&cc.egui_ctx)
    }

    /// Build the app against a bare egui context. Used by the windowed entry
    /// point and by `--smoke-ui`, which exercises every screen without a GPU.
    pub fn bootstrap(ctx: &egui::Context) -> Self {
        theme::install(ctx);
        let config = Config::load();
        if !paths::config_file().is_file() {
            let _ = config.save();
        }
        let mut app = Self {
            settings: config.clone(),
            port_text: config.bridge_port.to_string(),
            config,
            screen: Screen::Home,
            projects: Vec::new(),
            session: None,
            bridge: None,
            bridge_stats: None,
            bridge_error: None,
            plugin_installed: if studio::is_installed() {
                Some(studio::plugin_path())
            } else {
                None
            },
            rojo_cli: None,
            rojo_version: None,
            idea: String::new(),
            genre: None,
            gen: None,
            chat_input: String::new(),
            scope_to_file: true,
            token_visible: false,
            connect_test: None,
            connect_result: None,
            build_job: None,
            build_result: None,
            status: String::new(),
            error: None,
            focus_idea: false,
        };
        app.refresh_projects();
        app.refresh_tools();
        app.start_bridge();
        app.status = if app.config.effective_token().is_empty() {
            "Add a Vly deployment token in Settings to generate a game.".into()
        } else {
            "Ready.".into()
        };
        app
    }

    pub fn refresh_projects(&mut self) {
        self.projects = project::load_all(&self.config.projects_root_path());
    }

    pub fn refresh_tools(&mut self) {
        let cli = rojo::find_cli(&self.config.rojo_path);
        self.rojo_version = cli.as_deref().and_then(rojo::version);
        self.rojo_cli = cli;
        self.plugin_installed = if studio::is_installed() {
            Some(studio::plugin_path())
        } else {
            None
        };
    }

    pub fn start_bridge(&mut self) {
        self.bridge = None;
        self.bridge_stats = None;
        self.bridge_error = None;
        match Bridge::start(self.config.bridge_port, self.config.bridge_token.clone()) {
            Ok(bridge) => {
                self.bridge_stats = Some(bridge.stats());
                bridge.publish(
                    self.session
                        .as_ref()
                        .map(|session| session.snapshot.clone()),
                );
                self.status = format!("Studio bridge listening on 127.0.0.1:{}", bridge.port);
                self.bridge = Some(bridge);
            }
            Err(error) => {
                self.bridge_error = Some(format!("{error:#}"));
            }
        }
    }

    pub fn publish_snapshot(&mut self) {
        if let Some(bridge) = &self.bridge {
            bridge.publish(
                self.session
                    .as_ref()
                    .map(|session| session.snapshot.clone()),
            );
        }
    }

    pub fn open_project(&mut self, loaded: LoadedProject) {
        match Session::open(loaded) {
            Ok(session) => {
                self.status = format!(
                    "Opened {} — {} files",
                    session.snapshot.title,
                    session.snapshot.file_count()
                );
                self.session = Some(session);
                self.publish_snapshot();
                self.screen = Screen::Files;
            }
            Err(error) => self.error = Some(format!("Could not open project: {error:#}")),
        }
    }

    pub fn navigate(&mut self, screen: Screen) {
        if self.screen == Screen::Files && screen != Screen::Files {
            self.save_all();
        }
        self.screen = screen;
        if screen == Screen::Studio {
            self.refresh_tools();
        }
    }

    pub fn save_current(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        let result = session.save_current();
        match result {
            Ok(true) => {
                self.status = "Saved.".into();
                self.publish_snapshot();
            }
            Ok(false) => self.status = "Nothing to save.".into(),
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    pub fn save_all(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        let result = session.save_all();
        match result {
            Ok(0) => {}
            Ok(count) => {
                self.status = format!("Saved {count} files.");
                self.publish_snapshot();
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    pub fn start_generation(&mut self) {
        if self.gen.is_some() {
            return;
        }
        if self.config.effective_token().is_empty() {
            self.error = Some("Add your Vly deployment token in Settings first.".into());
            self.screen = Screen::Settings;
            return;
        }
        if self.idea.trim().chars().count() < 12 {
            self.error = Some("Describe the game in at least one sentence.".into());
            return;
        }
        self.gen = Some(jobs::spawn_generation(
            self.config.clone(),
            self.idea.clone(),
        ));
        self.screen = Screen::Build;
        self.status = "Generating game.".into();
    }

    pub fn start_revision(&mut self, instruction: String, scope: Option<String>) {
        if self.gen.is_some() {
            return;
        }
        if self.config.effective_token().is_empty() {
            self.error = Some("Add your Vly deployment token in Settings first.".into());
            self.screen = Screen::Settings;
            return;
        }
        let saved = {
            let Some(session) = &mut self.session else {
                return;
            };
            session.save_all()
        };
        if let Err(error) = saved {
            self.error = Some(format!("{error:#}"));
            return;
        }
        let (dir, project_meta, sources) = {
            let Some(session) = &self.session else {
                return;
            };
            (
                session.loaded.dir.clone(),
                session.loaded.project.clone(),
                session.sources(),
            )
        };
        if sources.is_empty() {
            self.error = Some("This project has no Luau files to edit.".into());
            return;
        }
        if let Some(scope) = &scope {
            if !sources.iter().any(|(path, _)| path == scope) {
                self.error = Some("Select a file before requesting a scoped change.".into());
                return;
            }
        }
        self.gen = Some(jobs::spawn_revision(
            self.config.clone(),
            dir,
            project_meta,
            sources,
            instruction.clone(),
            scope.clone(),
        ));
        self.screen = Screen::Build;
        self.status = "Applying AI change.".into();
        if let Some(session) = &mut self.session {
            session.push_chat(ChatRole::User, instruction);
        }
    }

    pub fn send_chat(&mut self) {
        let instruction = self.chat_input.trim().to_string();
        if instruction.len() < 3 {
            return;
        }
        let scope = if self.scope_to_file {
            self.session
                .as_ref()
                .and_then(|session| session.selected.clone())
        } else {
            None
        };
        self.chat_input.clear();
        self.start_revision(instruction, scope);
    }

    pub fn cancel_work(&mut self) {
        if let Some(gen) = &mut self.gen {
            gen.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            gen.cancelling = true;
            gen.note("Cancelling — the current request finishes first.".into());
        }
    }

    pub fn install_plugin(&mut self) {
        let port = self
            .bridge
            .as_ref()
            .map(|bridge| bridge.port)
            .unwrap_or(self.config.bridge_port);
        match studio::install(port, &self.config.bridge_token) {
            Ok(path) => {
                self.plugin_installed = Some(path.clone());
                self.status = format!("Studio plugin installed to {}", path.display());
            }
            Err(error) => self.error = Some(format!("Could not install plugin: {error:#}")),
        }
    }

    pub fn build_place(&mut self) {
        if self.build_job.is_some() {
            return;
        }
        let Some(session) = &self.session else {
            self.error = Some("Open a project first.".into());
            return;
        };
        let Some(cli) = self.rojo_cli.clone() else {
            self.error = Some(
                "Rojo was not found. Install it with `rokit add rojo-rbx/rojo` or set the path in Settings."
                    .into(),
            );
            return;
        };
        let dir = session.loaded.dir.clone();
        let output = dir
            .join("build")
            .join(format!("{}.rbxlx", util::slugify(&session.snapshot.title)));
        let (tx, rx) = mpsc::channel();
        let output_for_thread = output.clone();
        thread::spawn(move || {
            let result = rojo::build(&cli, &dir, &output_for_thread)
                .map(|_| output_for_thread.display().to_string())
                .map_err(|error| format!("{error:#}"));
            let _ = tx.send(result);
        });
        self.build_job = Some(rx);
        self.build_result = None;
        self.status = "Building place file with Rojo.".into();
    }

    pub fn save_settings(&mut self) {
        let mut draft = self.settings.clone();
        if let Ok(port) = self.port_text.trim().parse::<u16>() {
            if port >= 1024 {
                draft.bridge_port = port;
            }
        }
        draft.normalize();
        let bridge_changed = draft.bridge_port != self.config.bridge_port
            || draft.bridge_token != self.config.bridge_token;
        if let Err(error) = draft.save() {
            self.error = Some(format!("Could not save settings: {error:#}"));
            return;
        }
        self.config = draft.clone();
        self.settings = draft;
        self.status = "Settings saved.".into();
        if bridge_changed {
            self.start_bridge();
            if self.plugin_installed.is_some() {
                self.install_plugin();
            }
        }
    }

    pub fn test_connection(&mut self) {
        let client = VlyClient::from_config(&self.settings);
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(
                client
                    .test_connection()
                    .map_err(|error| format!("{error:#}")),
            );
        });
        self.connect_test = Some(rx);
        self.connect_result = None;
        self.status = "Testing Vly connection.".into();
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let mut new_project: Option<LoadedProject> = None;
        let mut revision: Option<(String, usize)> = None;
        let mut failure: Option<String> = None;

        if let Some(gen) = &mut self.gen {
            while let Ok(message) = gen.rx.try_recv() {
                match message {
                    WorkerMessage::Stage { total, label } => {
                        gen.total = total.max(gen.total);
                        for stage in gen.stages.iter_mut() {
                            stage.done = true;
                        }
                        gen.stages.push(jobs::Stage { label, done: false });
                    }
                    WorkerMessage::Note(note) => gen.note(note),
                    WorkerMessage::Plan(plan) => gen.plan = Some(plan),
                    WorkerMessage::File(path) => {
                        gen.files_written += 1;
                        gen.note(format!("wrote {path}"));
                    }
                    WorkerMessage::NewProject(loaded) => new_project = Some(*loaded),
                    WorkerMessage::Revision { reply, changed } => revision = Some((reply, changed)),
                    WorkerMessage::Failed(message) => failure = Some(message),
                }
            }
        }

        if let Some(loaded) = new_project {
            self.gen = None;
            self.refresh_projects();
            self.status = format!(
                "Generated {} — {} files",
                loaded.project.title,
                project::load_all(&self.config.projects_root_path())
                    .iter()
                    .find(|candidate| candidate.dir == loaded.dir)
                    .and_then(|candidate| candidate.snapshot().ok())
                    .map(|snapshot| snapshot.file_count())
                    .unwrap_or(0)
            );
            self.open_project(loaded);
        }

        if let Some((reply, changed)) = revision {
            self.gen = None;
            if let Some(session) = &mut self.session {
                let text = if reply.trim().is_empty() {
                    format!("Updated {changed} files.")
                } else {
                    reply
                };
                session.push_chat(ChatRole::Assistant, text);
                if let Err(error) = session.reload() {
                    self.error = Some(format!("{error:#}"));
                }
            }
            self.publish_snapshot();
            self.status = format!("AI change applied — {changed} files touched.");
        }

        if let Some(message) = failure {
            let cancelled = self.gen.as_ref().map(|gen| gen.cancelling).unwrap_or(false);
            self.gen = None;
            if let Some(session) = &mut self.session {
                session.push_chat(
                    ChatRole::Note,
                    if cancelled {
                        "Cancelled.".to_string()
                    } else {
                        message.clone()
                    },
                );
            }
            if !cancelled {
                self.error = Some(message);
            }
            self.status = if cancelled {
                "Cancelled.".into()
            } else {
                "Request failed.".into()
            };
        }

        if let Some(rx) = &self.connect_test {
            if let Ok(result) = rx.try_recv() {
                self.connect_result = Some(result);
                self.connect_test = None;
            }
        }

        if let Some(rx) = &self.build_job {
            if let Ok(result) = rx.try_recv() {
                self.build_result = Some(result);
                self.build_job = None;
            }
        }

        if self.gen.is_some() || self.build_job.is_some() || self.connect_test.is_some() {
            ctx.request_repaint_after(Duration::from_millis(120));
        } else {
            ctx.request_repaint_after(Duration::from_millis(900));
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let save = ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
        if save {
            self.save_current();
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.error = None;
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        self.poll(ctx);
        self.shortcuts(ctx);

        egui::TopBottomPanel::top("topbar")
            .exact_height(54.0)
            .frame(
                Frame::none()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(20.0, 0.0)),
            )
            .show(ctx, |ui| self.top_bar(ui));

        egui::SidePanel::left("rail")
            .exact_width(214.0)
            .resizable(false)
            .frame(
                Frame::none()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(12.0, 6.0)),
            )
            .show(ctx, |ui| self.rail(ui));

        egui::TopBottomPanel::bottom("statusbar")
            .exact_height(30.0)
            .frame(
                Frame::none()
                    .fill(BG)
                    .stroke(egui::Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::symmetric(20.0, 0.0)),
            )
            .show(ctx, |ui| self.status_bar(ui));

        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(28.0, 18.0)),
            )
            .show(ctx, |ui| self.content(ui));
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new("BLOXCODE").font(semibold(15.0)).color(TEXT));
            ui.add_space(7.0);
            ui.label(
                RichText::new(format!("v{VERSION}"))
                    .font(mono(9.5))
                    .color(DIM),
            );

            if let Some(session) = &self.session {
                ui.add_space(20.0);
                ui.label(RichText::new("/").font(mono(10.0)).color(DIM));
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&session.snapshot.title)
                        .font(medium(11.5))
                        .color(MUTED),
                );
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "New game", ButtonKind::Ghost).clicked() {
                    self.screen = Screen::Home;
                }
                ui.add_space(14.0);
                let (state, label) = match (
                    &self.bridge,
                    self.bridge_stats
                        .as_ref()
                        .and_then(|stats| stats.seconds_since_seen()),
                ) {
                    (None, _) => (Some(false), "Bridge offline".to_string()),
                    (Some(_), Some(seconds)) if seconds <= 45 => {
                        (Some(true), "Studio connected".to_string())
                    }
                    (Some(bridge), _) => (None, format!("Bridge 127.0.0.1:{}", bridge.port)),
                };
                widgets::status_line(ui, state, &label);
            });
        });
    }

    fn rail(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        let file_count = self
            .session
            .as_ref()
            .map(|session| session.snapshot.file_count());
        let items = [
            (Screen::Home, "Home", None),
            (Screen::Build, "Build", None),
            (Screen::Files, "Files", file_count),
            (Screen::Studio, "Studio", None),
            (Screen::Settings, "Settings", None),
        ];
        for (screen, label, badge) in items {
            let badge = badge.map(|count| count.to_string());
            if widgets::nav_item(ui, label, badge, self.screen == screen).clicked() {
                self.navigate(screen);
            }
        }

        ui.add_space(22.0);
        widgets::rule_with_label(ui, "Project");
        ui.add_space(10.0);
        match &self.session {
            Some(session) => {
                widgets::label(ui, &session.snapshot.title, medium(12.0), TEXT);
                ui.add_space(4.0);
                widgets::label(
                    ui,
                    &util::truncate(&session.loaded.dir.display().to_string(), 30),
                    mono(9.5),
                    DIM,
                );
                ui.add_space(8.0);
                if widgets::button(ui, "Open folder", ButtonKind::Quiet).clicked() {
                    open_path(&session.loaded.dir);
                }
            }
            None => {
                widgets::note(ui, "No project open.");
            }
        }

        let remaining = (ui.available_height() - 76.0).max(0.0);
        ui.add_space(remaining);
        widgets::hairline(ui);
        ui.add_space(8.0);
        let rojo_label = match (&self.rojo_cli, &self.rojo_version) {
            (Some(_), Some(version)) => format!("Rojo {version}"),
            (Some(_), None) => "Rojo detected".to_string(),
            (None, _) => "Rojo not found".to_string(),
        };
        widgets::label(ui, &rojo_label, mono(9.5), DIM);
        ui.add_space(2.0);
        widgets::label(ui, "Windows · x64", mono(9.5), DIM);
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            let active = self.gen.is_some();
            widgets::dot(ui, active, if active { WHITE } else { DIM });
            ui.label(RichText::new(&self.status).font(sans(11.0)).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(VERSION).font(mono(9.5)).color(DIM));
                ui.add_space(14.0);
                ui.label(RichText::new("CTRL+S SAVE").font(mono(9.0)).color(DIM));
            });
        });
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        if let Some(message) = self.error.clone() {
            let mut dismiss = false;
            widgets::error_card(ui, &message, &mut dismiss);
            ui.add_space(12.0);
            if dismiss {
                self.error = None;
            }
        }
        match self.screen {
            Screen::Home => views::home::show(self, ui),
            Screen::Build => views::build::show(self, ui),
            Screen::Files => views::files::show(self, ui),
            Screen::Studio => views::studio::show(self, ui),
            Screen::Settings => views::settings::show(self, ui),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui(ctx);
    }
}

pub fn open_path(path: &Path) {
    #[cfg(windows)]
    {
        let _ = Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }
}

pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}

pub fn project_age(updated_at: &str) -> String {
    let date = updated_at.split('T').next().unwrap_or(updated_at);
    date.to_string()
}

pub fn elapsed_label(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    }
}

pub fn chat_role_label(role: ChatRole) -> &'static str {
    match role {
        ChatRole::User => "You",
        ChatRole::Assistant => "BloxCode",
        ChatRole::Note => "Note",
    }
}

pub fn chat_role_color(role: ChatRole) -> egui::Color32 {
    match role {
        ChatRole::User => MUTED,
        ChatRole::Assistant => WHITE,
        ChatRole::Note => DIM,
    }
}

pub fn active_generation_title(kind: GenKind) -> &'static str {
    match kind {
        GenKind::NewGame => "Building your game",
        GenKind::Revision => "Editing your game",
    }
}

pub fn bridge_display(app: &App) -> String {
    match &app.bridge {
        Some(bridge) => format!("127.0.0.1:{}", bridge.port),
        None => "offline".to_string(),
    }
}

pub fn connection_state(app: &App) -> (Option<bool>, String) {
    match (
        &app.bridge,
        app.bridge_stats
            .as_ref()
            .and_then(|stats| stats.seconds_since_seen()),
    ) {
        (None, _) => (Some(false), "Bridge offline".into()),
        (Some(_), Some(seconds)) if seconds <= 45 => (Some(true), "Studio connected".into()),
        (Some(_), _) => (None, "Waiting for Studio".into()),
    }
}

pub fn plugin_state(app: &App) -> (bool, String) {
    match &app.plugin_installed {
        Some(path) => (true, path.display().to_string()),
        None => (false, "Not installed".to_string()),
    }
}
