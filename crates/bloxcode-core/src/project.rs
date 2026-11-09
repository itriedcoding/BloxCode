use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::generator::{GeneratedFile, Revision};
use crate::plan::{FilePlan, FileRole, Plan, RemoteSpec};
use crate::util;

pub const META_FILE: &str = "bloxcode.json";
pub const GITIGNORE: &str = "build/\n*.rbxl\n*.rbxlx\n*.rbxm\n*.rbxmx\n";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub pitch: String,
    pub plan: Plan,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
}

impl Default for Project {
    fn default() -> Self {
        let now = util::now_iso8601();
        Self {
            id: util::random_hex(6),
            title: "Untitled Game".into(),
            pitch: String::new(),
            plan: Plan::default(),
            created_at: now.clone(),
            updated_at: now,
            revision: 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LoadedProject {
    pub dir: PathBuf,
    pub project: Project,
}

impl LoadedProject {
    pub fn snapshot(&self) -> Result<Snapshot> {
        snapshot(&self.dir, &self.project)
    }

    pub fn save_meta(&self) -> Result<()> {
        save_meta(&self.dir, &self.project)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StudioFile {
    pub path: String,
    pub name: String,
    pub class: String,
    pub target: String,
    pub bytes: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub title: String,
    pub revision: u64,
    pub updated_at: String,
    pub remotes: Vec<RemoteSpec>,
    pub files: Vec<StudioFile>,
    pub contents: HashMap<String, String>,
}

impl Snapshot {
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn total_bytes(&self) -> usize {
        self.files.iter().map(|file| file.bytes).sum()
    }

    pub fn source(&self, path: &str) -> Option<&String> {
        self.contents.get(path)
    }

    /// JSON consumed by the Roblox Studio plugin.
    pub fn manifest(&self) -> Value {
        json!({
            "app": "BloxCode",
            "version": crate::VERSION,
            "namespace": crate::NAMESPACE,
            "title": self.title,
            "revision": self.revision,
            "updated_at": self.updated_at,
            "files": self.files.iter().map(|file| json!({
                "path": file.path,
                "name": file.name,
                "class": file.class,
                "target": file.target,
                "bytes": file.bytes,
            })).collect::<Vec<_>>(),
            "remotes": self.remotes.iter().map(|remote| json!({
                "name": remote.name,
                "class": remote.class(),
                "notes": remote.notes,
            })).collect::<Vec<_>>(),
        })
    }
}

/// Map a Luau file onto the Studio instance it becomes.
pub fn studio_file(path: &str, bytes: usize) -> Option<StudioFile> {
    let role = FileRole::from_path(path)?;
    let rel = path.strip_prefix(role.dir())?;
    let parts: Vec<&str> = rel.split('/').filter(|part| !part.is_empty()).collect();
    let file_name = parts.last()?;
    let stem = file_name
        .trim_end_matches(".luau")
        .trim_end_matches(".server")
        .trim_end_matches(".client");
    let name = util::pascal_case(stem);
    let (class, mut target) = match role {
        FileRole::Server => ("Script", "ServerScriptService/BloxCode".to_string()),
        FileRole::Client => (
            "LocalScript",
            "StarterPlayer/StarterPlayerScripts/BloxCode".to_string(),
        ),
        FileRole::Shared => (
            "ModuleScript",
            "ReplicatedStorage/BloxCode/Shared".to_string(),
        ),
    };
    for folder in &parts[..parts.len().saturating_sub(1)] {
        target.push('/');
        target.push_str(&util::pascal_case(folder));
    }
    Some(StudioFile {
        path: path.to_string(),
        name,
        class: class.to_string(),
        target,
        bytes,
    })
}

/// Create a new project directory and write every source file.
pub fn create_project(root: &Path, plan: &Plan, files: &[GeneratedFile]) -> Result<LoadedProject> {
    std::fs::create_dir_all(root).with_context(|| format!("creating {}", root.display()))?;
    let slug = util::slugify(&plan.title);
    let mut dir = root.join(&slug);
    let mut suffix = 2;
    while dir.exists() {
        dir = root.join(format!("{slug}-{suffix}"));
        suffix += 1;
        if suffix > 200 {
            bail!(
                "could not find a free project folder under {}",
                root.display()
            );
        }
    }
    std::fs::create_dir_all(&dir)?;
    for role in [FileRole::Shared, FileRole::Server, FileRole::Client] {
        std::fs::create_dir_all(dir.join(role.dir()))?;
    }

    for file in files {
        write_script(&dir, &file.path, &file.content)?;
    }

    let rojo = crate::rojo::project_json(plan);
    std::fs::write(
        dir.join("default.project.json"),
        serde_json::to_string_pretty(&rojo)?,
    )?;
    std::fs::write(dir.join(".gitignore"), GITIGNORE)?;
    std::fs::write(dir.join("README.md"), readme(plan))?;

    let mut project = Project {
        id: util::random_hex(6),
        title: plan.title.clone(),
        pitch: plan.pitch.clone(),
        plan: plan.clone(),
        created_at: util::now_iso8601(),
        updated_at: util::now_iso8601(),
        revision: 1,
    };
    save_meta(&dir, &project)?;
    project.updated_at = util::now_iso8601();
    Ok(LoadedProject { dir, project })
}

pub fn save_meta(dir: &Path, project: &Project) -> Result<()> {
    let path = dir.join(META_FILE);
    std::fs::write(&path, serde_json::to_string_pretty(project)?)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn load_project(dir: &Path) -> Result<LoadedProject> {
    let raw = std::fs::read_to_string(dir.join(META_FILE))
        .with_context(|| format!("reading metadata in {}", dir.display()))?;
    let project: Project = serde_json::from_str(&raw).context("project metadata is corrupt")?;
    Ok(LoadedProject {
        dir: dir.to_path_buf(),
        project,
    })
}

pub fn load_all(root: &Path) -> Vec<LoadedProject> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || !dir.join(META_FILE).is_file() {
            continue;
        }
        if let Ok(loaded) = load_project(&dir) {
            out.push(loaded);
        }
    }
    out.sort_by(|a, b| b.project.updated_at.cmp(&a.project.updated_at));
    out
}

/// Walk `src/` and build the snapshot consumed by the editor and the bridge.
pub fn snapshot(dir: &Path, project: &Project) -> Result<Snapshot> {
    let mut snapshot = Snapshot {
        title: project.title.clone(),
        revision: project.revision,
        updated_at: project.updated_at.clone(),
        remotes: project.plan.remotes.clone(),
        ..Default::default()
    };
    let src = dir.join("src");
    let mut paths = Vec::new();
    collect_luau(&src, &src, &mut paths)?;
    paths.sort();
    for path in paths {
        let full = dir.join(&path);
        let content = std::fs::read_to_string(&full)
            .with_context(|| format!("reading {}", full.display()))?;
        let bytes = content.len();
        if let Some(file) = studio_file(&path, bytes) {
            snapshot.contents.insert(path, content);
            snapshot.files.push(file);
        }
    }
    Ok(snapshot)
}

fn collect_luau(base: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_luau(base, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("luau") {
            if let Ok(relative) = path.strip_prefix(base) {
                let normalized = format!("src/{}", relative.to_string_lossy().replace('\\', "/"));
                out.push(normalized);
            }
        }
    }
    Ok(())
}

/// Reject anything that could escape the project directory.
pub fn safe_path(dir: &Path, relative: &str) -> Result<PathBuf> {
    use std::path::Component;

    let trimmed = relative.trim();
    let flat_segments = !trimmed.is_empty()
        && !trimmed.contains("..")
        && !trimmed.contains('\\')
        && !trimmed.contains(':')
        && trimmed
            .split('/')
            .all(|segment| !segment.is_empty() && segment != ".");
    // A rooted path such as `/etc/passwd` is not `is_absolute()` on Windows, so
    // inspect the components as well; only plain file-name segments are allowed.
    let contained = Path::new(trimmed)
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    if !flat_segments || !contained {
        bail!("unsafe project path `{relative}`");
    }
    Ok(dir.join(trimmed))
}

pub fn write_script(dir: &Path, relative: &str, content: &str) -> Result<()> {
    let path = safe_path(dir, relative)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn delete_script(dir: &Path, relative: &str) -> Result<()> {
    let path = safe_path(dir, relative)?;
    if path.is_file() {
        std::fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))?;
    }
    Ok(())
}

/// Bump the revision and stamp the update time.
pub fn bump(project: &mut Project) {
    project.revision += 1;
    project.updated_at = util::now_iso8601();
}

/// Bump the revision and persist metadata.
pub fn touch(loaded: &mut LoadedProject) -> Result<()> {
    bump(&mut loaded.project);
    save_meta(&loaded.dir, &loaded.project)
}

/// Apply an AI revision to disk: full-file writes, deletions, newly declared
/// remotes, then refresh the Rojo tree and metadata.
pub fn apply_revision(dir: &Path, project: &mut Project, revision: &Revision) -> Result<()> {
    for file in &revision.writes {
        if !crate::plan::is_valid_script_path(&file.path) {
            bail!("model returned an invalid path `{}`", file.path);
        }
        write_script(dir, &file.path, &file.content)?;
        if !project
            .plan
            .files
            .iter()
            .any(|existing| existing.path == file.path)
        {
            project.plan.files.push(FilePlan {
                path: file.path.clone(),
                role: FileRole::from_path(&file.path),
                purpose: "Added by an AI change".into(),
            });
        }
    }

    for path in &revision.deletes {
        delete_script(dir, path)?;
        project.plan.files.retain(|file| &file.path != path);
    }

    for remote in &revision.remotes {
        if !project
            .plan
            .remotes
            .iter()
            .any(|existing| existing.name == remote.name)
        {
            project.plan.remotes.push(remote.clone());
        }
    }

    let rojo = crate::rojo::project_json(&project.plan);
    std::fs::write(
        dir.join("default.project.json"),
        serde_json::to_string_pretty(&rojo)?,
    )?;
    std::fs::write(dir.join("README.md"), readme(&project.plan))?;

    bump(project);
    save_meta(dir, project)
}

pub fn readme(plan: &Plan) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", plan.title));
    out.push_str(&format!("{}\n\n", plan.pitch));
    out.push_str(&format!("**Core loop.** {}\n\n", plan.core_loop));
    out.push_str("Generated with [BloxCode](https://github.com/itriedcoding/BloxCode).\n\n");
    out.push_str("## Systems\n\n");
    for system in &plan.systems {
        out.push_str(&format!("- **{}** — {}\n", system.name, system.detail));
    }
    out.push_str("\n## Layout\n\n");
    out.push_str("- `src/shared` — ModuleScripts shared by both sides.\n");
    out.push_str("- `src/server` — Scripts that own state and authority.\n");
    out.push_str("- `src/client` — LocalScripts that build UI and send intent.\n");
    out.push_str("- `default.project.json` — Rojo tree, mirrors the Studio plugin.\n\n");
    out.push_str(&plan.contract());
    out.push_str("\n## Open it\n\n");
    out.push_str("1. In BloxCode, open **Studio** and install the Studio plugin.\n");
    out.push_str("2. Enable **Game Settings → Security → Allow HTTP Requests** in Studio.\n");
    out.push_str("3. Press **Sync to Studio**.\n\n");
    out.push_str(&format!(
        "Or build a place file with Rojo: `rojo build default.project.json -o build/{}.rbxlx`.\n",
        util::slugify(&plan.title)
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::SystemSpec;

    fn plan() -> Plan {
        Plan {
            title: "Test Game".into(),
            pitch: "p".into(),
            core_loop: "c".into(),
            systems: vec![SystemSpec {
                name: "S".into(),
                detail: "d".into(),
            }],
            files: vec![],
            remotes: vec![RemoteSpec {
                name: "Buy".into(),
                kind: "event".into(),
                notes: String::new(),
            }],
        }
    }

    #[test]
    fn studio_mapping_matches_contract() {
        let server = studio_file("src/server/Main.server.luau", 10).unwrap();
        assert_eq!(server.class, "Script");
        assert_eq!(server.target, "ServerScriptService/BloxCode");
        assert_eq!(server.name, "Main");

        let nested = studio_file("src/shared/Economy/Shop.luau", 10).unwrap();
        assert_eq!(nested.class, "ModuleScript");
        assert_eq!(nested.target, "ReplicatedStorage/BloxCode/Shared/Economy");
        assert_eq!(nested.name, "Shop");

        let client = studio_file("src/client/Hud.client.luau", 10).unwrap();
        assert_eq!(client.class, "LocalScript");
        assert_eq!(client.target, "StarterPlayer/StarterPlayerScripts/BloxCode");
    }

    #[test]
    fn safe_path_blocks_traversal() {
        let base = Path::new("/tmp/project");
        assert!(safe_path(base, "../evil.luau").is_err());
        assert!(safe_path(base, "/etc/passwd").is_err());
        assert!(safe_path(base, "//server/share/x.luau").is_err());
        assert!(safe_path(base, "src//A.luau").is_err());
        assert!(safe_path(base, "./src/A.luau").is_err());
        assert!(safe_path(base, "src\\shared\\A.luau").is_err());
        assert!(safe_path(base, "C:/Windows/x.luau").is_err());
        assert_eq!(
            safe_path(base, "src/shared/A.luau").unwrap(),
            base.join("src").join("shared").join("A.luau")
        );
    }

    #[test]
    fn snapshot_roundtrip_writes_and_reads() {
        let root = std::env::temp_dir().join(format!("bloxcode-test-{}", util::random_hex(6)));
        let files = vec![
            GeneratedFile {
                path: "src/shared/Config.luau".into(),
                content: "return {}\n".into(),
            },
            GeneratedFile {
                path: "src/server/Main.server.luau".into(),
                content: "print('hi')\n".into(),
            },
        ];
        let loaded = create_project(&root, &plan(), &files).unwrap();
        assert!(loaded.dir.join("default.project.json").is_file());
        let snap = loaded.snapshot().unwrap();
        assert_eq!(snap.file_count(), 2);
        assert!(snap.contents.contains_key("src/server/Main.server.luau"));
        let manifest = snap.manifest();
        assert_eq!(manifest["namespace"], crate::NAMESPACE);
        assert_eq!(manifest["files"].as_array().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn duplicate_titles_get_unique_folders() {
        let root = std::env::temp_dir().join(format!("bloxcode-test-{}", util::random_hex(6)));
        let files = vec![GeneratedFile {
            path: "src/shared/A.luau".into(),
            content: "return 1".into(),
        }];
        let first = create_project(&root, &plan(), &files).unwrap();
        let second = create_project(&root, &plan(), &files).unwrap();
        assert_ne!(first.dir, second.dir);
        assert_eq!(load_all(&root).len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn revision_updates_plan_remotes_and_rojo_tree() {
        let root = std::env::temp_dir().join(format!("bloxcode-test-{}", util::random_hex(6)));
        let files = vec![GeneratedFile {
            path: "src/shared/A.luau".into(),
            content: "return 1".into(),
        }];
        let mut loaded = create_project(&root, &plan(), &files).unwrap();
        let revision = crate::generator::Revision {
            reply: "added a remote".into(),
            writes: vec![GeneratedFile {
                path: "src/server/Extra.server.luau".into(),
                content: "print('x')".into(),
            }],
            deletes: vec![],
            remotes: vec![RemoteSpec {
                name: "Purchase".into(),
                kind: "function".into(),
                notes: String::new(),
            }],
        };
        let before = loaded.project.revision;
        apply_revision(&loaded.dir, &mut loaded.project, &revision).unwrap();
        assert_eq!(loaded.project.revision, before + 1);
        assert!(loaded
            .project
            .plan
            .files
            .iter()
            .any(|f| f.path == "src/server/Extra.server.luau"));
        assert!(loaded
            .project
            .plan
            .remotes
            .iter()
            .any(|r| r.name == "Purchase"));
        let tree = std::fs::read_to_string(loaded.dir.join("default.project.json")).unwrap();
        assert!(tree.contains("Purchase"));
        assert!(tree.contains("RemoteFunction"));
        let snap = loaded.snapshot().unwrap();
        assert!(snap.contents.contains_key("src/server/Extra.server.luau"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
