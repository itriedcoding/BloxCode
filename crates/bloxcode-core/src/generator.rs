//! Two-pass game generation: an architecture blueprint, then batched Luau
//! source files. Every response is validated before anything reaches disk.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::ai::json::parse_json;
use crate::ai::prompts;
use crate::ai::VlyClient;
use crate::plan::{FilePlan, FileRole, Plan, MAX_FILE_BYTES};

#[derive(Clone, Debug)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct Game {
    pub plan: Plan,
    pub files: Vec<GeneratedFile>,
}

/// Progress events emitted while a generation or revision runs.
#[derive(Clone, Debug)]
pub enum GenEvent {
    Stage {
        index: usize,
        total: usize,
        label: String,
    },
    Plan(Plan),
    File(String),
    Note(String),
}

#[derive(Debug, Default, Deserialize)]
struct BatchPayload {
    #[serde(default)]
    files: Vec<FilePayload>,
}

#[derive(Debug, Default, Deserialize)]
struct FilePayload {
    #[serde(default)]
    path: String,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Default, Deserialize)]
struct RevisionPayload {
    #[serde(default)]
    reply: String,
    #[serde(default)]
    files: Vec<FilePayload>,
    #[serde(default)]
    delete: Vec<String>,
    #[serde(default)]
    remotes: Vec<crate::plan::RemoteSpec>,
}

#[derive(Clone, Debug)]
pub struct Revision {
    pub reply: String,
    pub writes: Vec<GeneratedFile>,
    pub deletes: Vec<String>,
    pub remotes: Vec<crate::plan::RemoteSpec>,
}

fn cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::Relaxed)
}

/// Generate a complete project from a plain-language idea.
pub fn generate_game(
    client: &VlyClient,
    idea: &str,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(GenEvent),
) -> Result<Game> {
    if idea.trim().chars().count() < 12 {
        bail!("describe the game in at least one sentence");
    }

    emit(GenEvent::Stage {
        index: 0,
        total: 0,
        label: "Designing systems".into(),
    });
    let reply = client
        .chat(
            prompts::PLAN_SYSTEM,
            &[("user".into(), prompts::plan_user(idea))],
        )
        .context("blueprint request failed")?;

    let mut plan: Plan = parse_json(&reply).context("blueprint was not valid JSON")?;
    for note in plan.normalize() {
        emit(GenEvent::Note(note));
    }

    if let Err(error) = plan.validate() {
        emit(GenEvent::Note(format!(
            "validator rejected blueprint: {error}"
        )));
        let repaired = client
            .chat(
                prompts::PLAN_SYSTEM,
                &[(
                    "user".into(),
                    prompts::plan_repair_user(idea, &reply, &[error.to_string()]),
                )],
            )
            .context("blueprint repair request failed")?;
        plan = parse_json(&repaired).context("repaired blueprint was not valid JSON")?;
        for note in plan.normalize() {
            emit(GenEvent::Note(note));
        }
        plan.validate()
            .context("blueprint was still invalid after repair")?;
    }

    emit(GenEvent::Plan(plan.clone()));

    let mut batches: Vec<(FileRole, Vec<FilePlan>)> = Vec::new();
    for role in [FileRole::Shared, FileRole::Server, FileRole::Client] {
        let files = plan.files_by_role(role);
        for chunk in files.chunks(4) {
            batches.push((role, chunk.iter().map(|f| (*f).clone()).collect()));
        }
    }
    let total = batches.len() + 1;

    let mut written: Vec<GeneratedFile> = Vec::new();
    for (index, (role, specs)) in batches.iter().enumerate() {
        if cancelled(cancel) {
            bail!("generation cancelled");
        }
        emit(GenEvent::Stage {
            index: index + 1,
            total,
            label: format!("Writing {} scripts", role.label()),
        });

        let prior = context_files(&written, &plan);
        let refs: Vec<&FilePlan> = specs.iter().collect();

        let mut received = request_batch(client, &plan, &refs, &prior, false)
            .with_context(|| format!("{} batch failed", role.label()))?;
        let missing: Vec<String> = specs
            .iter()
            .map(|spec| spec.path.clone())
            .filter(|path| !received.iter().any(|file| &file.path == path))
            .collect();
        if !missing.is_empty() {
            emit(GenEvent::Note(format!(
                "retrying {} batch, missing {}",
                role.label(),
                missing.join(", ")
            )));
            let retry = request_batch(client, &plan, &refs, &prior, true)
                .with_context(|| format!("{} batch retry failed", role.label()))?;
            for file in retry {
                if !received.iter().any(|existing| existing.path == file.path) {
                    received.push(file);
                }
            }
        }

        let still_missing: Vec<&str> = specs
            .iter()
            .map(|spec| spec.path.as_str())
            .filter(|path| !received.iter().any(|file| file.path == *path))
            .collect();
        if !still_missing.is_empty() {
            bail!(
                "model did not return {}; try generating again",
                still_missing.join(", ")
            );
        }

        for file in received {
            if let Some(warning) = placeholder_warning(&file.content) {
                emit(GenEvent::Note(format!("{}: {warning}", file.path)));
            }
            emit(GenEvent::File(file.path.clone()));
            written.push(file);
        }
    }

    // Keep the plan order so the file tree is stable.
    written.sort_by_key(|file| {
        plan.files
            .iter()
            .position(|spec| spec.path == file.path)
            .unwrap_or(usize::MAX)
    });

    Ok(Game {
        plan,
        files: written,
    })
}

fn request_batch(
    client: &VlyClient,
    plan: &Plan,
    specs: &[&FilePlan],
    prior: &[(String, String)],
    strict: bool,
) -> Result<Vec<GeneratedFile>> {
    let reply = client.chat(
        prompts::CODE_SYSTEM,
        &[(
            "user".into(),
            prompts::batch_user(plan, specs, prior, strict),
        )],
    )?;
    let payload: BatchPayload = parse_json(&reply)?;
    let allowed: Vec<String> = specs.iter().map(|spec| spec.path.clone()).collect();
    let mut out = Vec::new();
    for file in payload.files {
        let Some(normalized) = crate::plan::normalize_script_path(&file.path, None) else {
            continue;
        };
        if !allowed.contains(&normalized) {
            continue;
        }
        let content = clean_source(&file.content);
        if content.trim().is_empty() {
            continue;
        }
        if content.len() > MAX_FILE_BYTES {
            bail!("{} exceeds the {} byte limit", normalized, MAX_FILE_BYTES);
        }
        out.push(GeneratedFile {
            path: normalized,
            content,
        });
    }
    Ok(out)
}

/// Ask the model to change an existing project. Returns full-file writes only.
pub fn revise(
    client: &VlyClient,
    plan: &Plan,
    sources: &[(String, String)],
    instruction: &str,
    scope: Option<&str>,
    cancel: &AtomicBool,
) -> Result<Revision> {
    if instruction.trim().len() < 3 {
        bail!("describe the change first");
    }
    if cancelled(cancel) {
        bail!("revision cancelled");
    }
    let selected = select_context(sources, scope, 120_000);
    let reply = client.chat(
        prompts::REVISE_SYSTEM,
        &[(
            "user".into(),
            prompts::revise_user(plan, &selected, instruction, scope),
        )],
    )?;
    let payload: RevisionPayload = parse_json(&reply)?;

    let mut writes = Vec::new();
    for file in payload.files {
        let Some(path) = crate::plan::normalize_script_path(&file.path, None) else {
            continue;
        };
        let content = clean_source(&file.content);
        if content.trim().is_empty() || content.len() > MAX_FILE_BYTES {
            continue;
        }
        writes.push(GeneratedFile { path, content });
    }
    let deletes = payload
        .delete
        .into_iter()
        .filter_map(|path| crate::plan::normalize_script_path(&path, None))
        .filter(|path| sources.iter().any(|(existing, _)| existing == path))
        .filter(|path| !writes.iter().any(|write| &write.path == path))
        .collect::<Vec<_>>();

    if writes.is_empty() && deletes.is_empty() {
        bail!("model returned no file changes; rephrase the request");
    }

    let mut remotes: Vec<crate::plan::RemoteSpec> = Vec::new();
    for remote in payload.remotes {
        let name = crate::util::pascal_case(remote.name.trim());
        if name.is_empty() || remotes.iter().any(|existing| existing.name == name) {
            continue;
        }
        let kind = if remote.kind.trim().eq_ignore_ascii_case("function") {
            "function".to_string()
        } else {
            "event".to_string()
        };
        remotes.push(crate::plan::RemoteSpec {
            name,
            kind,
            notes: remote.notes.trim().to_string(),
        });
    }

    Ok(Revision {
        reply: if payload.reply.trim().is_empty() {
            "Applied changes.".to_string()
        } else {
            payload.reply.trim().to_string()
        },
        writes,
        deletes,
        remotes,
    })
}

/// Context handed to later batches: shared modules always, plus the most
/// recent files so imports stay consistent.
fn context_files(written: &[GeneratedFile], plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut budget = 26_000usize;
    let shared: Vec<&GeneratedFile> = written
        .iter()
        .filter(|file| FileRole::from_path(&file.path) == Some(FileRole::Shared))
        .collect();
    for file in shared.into_iter().chain(written.iter().rev()) {
        if budget == 0 {
            break;
        }
        if out.iter().any(|(path, _)| path == &file.path) {
            continue;
        }
        budget = budget.saturating_sub(file.content.len());
        out.push((file.path.clone(), file.content.clone()));
    }
    let _ = plan;
    out
}

fn select_context(
    sources: &[(String, String)],
    scope: Option<&str>,
    budget: usize,
) -> Vec<(String, String)> {
    let mut ordered: Vec<&(String, String)> = Vec::new();
    if let Some(scope) = scope {
        ordered.extend(sources.iter().find(|(path, _)| path == scope));
    }
    ordered.extend(
        sources
            .iter()
            .filter(|(path, _)| FileRole::from_path(path) == Some(FileRole::Shared)),
    );
    ordered.extend(sources.iter());

    let mut out: Vec<(String, String)> = Vec::new();
    let mut used = 0usize;
    for (path, content) in ordered {
        if out.iter().any(|(existing, _)| existing == path) {
            continue;
        }
        if used + content.len() > budget && !out.is_empty() {
            continue;
        }
        used += content.len();
        out.push((path.clone(), content.clone()));
    }
    out
}

/// Strip a UTF-8 BOM and normalise newlines; models occasionally emit CRLF.
pub fn clean_source(raw: &str) -> String {
    raw.trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end()
        .to_string()
}

fn placeholder_warning(content: &str) -> Option<String> {
    let lower = content.to_ascii_lowercase();
    for marker in [
        "-- todo",
        "--todo",
        "your code here",
        "not implemented",
        "placeholder",
    ] {
        if lower.contains(marker) {
            return Some(format!("contains `{marker}`"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::SystemSpec;

    fn plan() -> Plan {
        Plan {
            title: "T".into(),
            pitch: "p".into(),
            core_loop: "c".into(),
            systems: vec![SystemSpec {
                name: "S".into(),
                detail: "d".into(),
            }],
            files: vec![
                FilePlan {
                    path: "src/shared/Config.luau".into(),
                    role: Some(FileRole::Shared),
                    purpose: "config".into(),
                },
                FilePlan {
                    path: "src/server/Main.server.luau".into(),
                    role: Some(FileRole::Server),
                    purpose: "boot".into(),
                },
            ],
            remotes: vec![],
        }
    }

    #[test]
    fn context_prefers_shared_modules() {
        let written = vec![
            GeneratedFile {
                path: "src/server/Main.server.luau".into(),
                content: "server".into(),
            },
            GeneratedFile {
                path: "src/shared/Config.luau".into(),
                content: "shared".into(),
            },
        ];
        let context = context_files(&written, &plan());
        assert_eq!(context[0].0, "src/shared/Config.luau");
    }

    #[test]
    fn select_context_starts_with_scope() {
        let sources = vec![
            ("src/shared/A.luau".to_string(), "a".to_string()),
            ("src/server/B.server.luau".to_string(), "b".to_string()),
        ];
        let selected = select_context(&sources, Some("src/server/B.server.luau"), 1000);
        assert_eq!(selected[0].0, "src/server/B.server.luau");
    }

    #[test]
    fn clean_source_normalises_newlines() {
        assert_eq!(clean_source("\u{feff}a\r\nb\r\n"), "a\nb");
    }

    #[test]
    fn placeholder_detection_is_case_insensitive() {
        assert!(placeholder_warning("-- TODO: shop").is_some());
        assert!(placeholder_warning("local x = 1").is_none());
    }
}
