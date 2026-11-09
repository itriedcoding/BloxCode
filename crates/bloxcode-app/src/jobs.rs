//! Background work. Generation and revision run on worker threads and stream
//! progress back through a channel so the UI never blocks on the network.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use bloxcode_core::ai::VlyClient;
use bloxcode_core::config::Config;
use bloxcode_core::generator::{self, GenEvent};
use bloxcode_core::plan::Plan;
use bloxcode_core::project::{self, LoadedProject, Project};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GenKind {
    NewGame,
    Revision,
}

pub struct Stage {
    pub label: String,
    pub done: bool,
}

pub struct Generation {
    pub kind: GenKind,
    pub rx: Receiver<WorkerMessage>,
    pub cancel: Arc<AtomicBool>,
    pub stages: Vec<Stage>,
    pub total: usize,
    pub log: Vec<String>,
    pub plan: Option<Plan>,
    pub started: Instant,
    pub files_written: usize,
    pub cancelling: bool,
}

impl Generation {
    fn new(kind: GenKind, rx: Receiver<WorkerMessage>) -> Self {
        Self {
            kind,
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            stages: Vec::new(),
            total: 0,
            log: Vec::new(),
            plan: None,
            started: Instant::now(),
            files_written: 0,
            cancelling: false,
        }
    }

    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        let completed = self.stages.iter().filter(|stage| stage.done).count() as f32;
        (completed / self.total as f32).clamp(0.0, 1.0)
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub fn note(&mut self, message: String) {
        self.log.push(message);
        if self.log.len() > 60 {
            self.log.remove(0);
        }
    }
}

pub enum WorkerMessage {
    Stage { total: usize, label: String },
    Note(String),
    Plan(Plan),
    File(String),
    NewProject(Box<LoadedProject>),
    Revision { reply: String, changed: usize },
    Failed(String),
}

pub fn spawn_generation(config: Config, idea: String) -> Generation {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let root = config.projects_root_path();

    thread::Builder::new()
        .name("bloxcode-generate".into())
        .spawn(move || {
            let client = VlyClient::from_config(&config);
            let events = tx.clone();
            let mut emit = |event: GenEvent| {
                let message = match event {
                    GenEvent::Stage {
                        index: _,
                        total,
                        label,
                    } => WorkerMessage::Stage { total, label },
                    GenEvent::Note(note) => WorkerMessage::Note(note),
                    GenEvent::Plan(plan) => WorkerMessage::Plan(plan),
                    GenEvent::File(path) => WorkerMessage::File(path),
                };
                let _ = events.send(message);
            };
            match generator::generate_game(&client, &idea, &flag, &mut emit) {
                Ok(game) => match project::create_project(&root, &game.plan, &game.files) {
                    Ok(loaded) => {
                        let _ = tx.send(WorkerMessage::NewProject(Box::new(loaded)));
                    }
                    Err(error) => {
                        let _ = tx.send(WorkerMessage::Failed(format!("{error:#}")));
                    }
                },
                Err(error) => {
                    let _ = tx.send(WorkerMessage::Failed(format!("{error:#}")));
                }
            }
        })
        .expect("generation thread spawns");

    Generation::new(GenKind::NewGame, rx)
}

pub fn spawn_revision(
    config: Config,
    dir: PathBuf,
    mut project: Project,
    sources: Vec<(String, String)>,
    instruction: String,
    scope: Option<String>,
) -> Generation {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let worker_instruction = instruction;
    let worker_scope = scope;

    thread::Builder::new()
        .name("bloxcode-revise".into())
        .spawn(move || {
            let client = VlyClient::from_config(&config);
            match generator::revise(
                &client,
                &project.plan,
                &sources,
                &worker_instruction,
                worker_scope.as_deref(),
                &flag,
            ) {
                Ok(revision) => match project::apply_revision(&dir, &mut project, &revision) {
                    Ok(()) => {
                        let _ = tx.send(WorkerMessage::Revision {
                            reply: revision.reply,
                            changed: revision.writes.len() + revision.deletes.len(),
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(WorkerMessage::Failed(format!("{error:#}")));
                    }
                },
                Err(error) => {
                    if flag.load(Ordering::Relaxed) {
                        let _ = tx.send(WorkerMessage::Failed("Revision cancelled.".into()));
                    } else {
                        let _ = tx.send(WorkerMessage::Failed(format!("{error:#}")));
                    }
                }
            }
        })
        .expect("revision thread spawns");

    Generation::new(GenKind::Revision, rx)
}
