use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Hard limits that keep a generated project tractable and playable.
pub const MIN_FILES: usize = 3;
pub const MAX_FILES: usize = 18;
pub const MAX_FILE_BYTES: usize = 200_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    Shared,
    Server,
    Client,
}

impl FileRole {
    pub fn dir(self) -> &'static str {
        match self {
            FileRole::Shared => "src/shared/",
            FileRole::Server => "src/server/",
            FileRole::Client => "src/client/",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FileRole::Shared => "shared",
            FileRole::Server => "server",
            FileRole::Client => "client",
        }
    }

    pub fn from_path(path: &str) -> Option<Self> {
        if path.starts_with("src/shared/") {
            Some(FileRole::Shared)
        } else if path.starts_with("src/server/") {
            Some(FileRole::Server)
        } else if path.starts_with("src/client/") {
            Some(FileRole::Client)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemSpec {
    pub name: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FilePlan {
    pub path: String,
    pub role: Option<FileRole>,
    pub purpose: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteSpec {
    pub name: String,
    /// `event` or `function`; anything else is normalised to `event`.
    pub kind: String,
    pub notes: String,
}

impl RemoteSpec {
    pub fn class(&self) -> &'static str {
        if self.kind == "function" {
            "RemoteFunction"
        } else {
            "RemoteEvent"
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Plan {
    pub title: String,
    pub pitch: String,
    pub core_loop: String,
    pub systems: Vec<SystemSpec>,
    pub files: Vec<FilePlan>,
    pub remotes: Vec<RemoteSpec>,
}

impl Plan {
    /// Repair model output into a valid, Rojo-compatible blueprint.
    /// Returns human-readable notes describing what was changed.
    pub fn normalize(&mut self) -> Vec<String> {
        let mut notes = Vec::new();

        self.title = self.title.trim().to_string();
        if self.title.is_empty() {
            self.title = "Untitled Game".into();
            notes.push("title was empty".into());
        }
        if self.title.chars().count() > 60 {
            self.title = self
                .title
                .chars()
                .take(60)
                .collect::<String>()
                .trim()
                .to_string();
        }
        if self.pitch.trim().is_empty() {
            self.pitch = "A Roblox experience generated with BloxCode.".into();
        }
        if self.core_loop.trim().is_empty() {
            self.core_loop =
                "Players join, take part in the main activity and earn progress.".into();
        }

        self.systems.retain(|s| !s.name.trim().is_empty());
        for system in &mut self.systems {
            system.name = system.name.trim().to_string();
            system.detail = system.detail.trim().to_string();
        }

        let mut seen = Vec::new();
        let mut files = Vec::new();
        for file in std::mem::take(&mut self.files) {
            let hint = file.role;
            let Some(path) = normalize_script_path(&file.path, hint) else {
                notes.push(format!("dropped unusable path `{}`", file.path));
                continue;
            };
            if seen.contains(&path) {
                notes.push(format!("removed duplicate `{path}`"));
                continue;
            }
            seen.push(path.clone());
            let role = FileRole::from_path(&path);
            files.push(FilePlan {
                path,
                role,
                purpose: file.purpose.trim().to_string(),
            });
        }
        if files.len() > MAX_FILES {
            notes.push(format!("trimmed file list to {MAX_FILES} entries"));
            files.truncate(MAX_FILES);
        }
        self.files = files;

        let mut remote_names = Vec::new();
        for remote in std::mem::take(&mut self.remotes) {
            let name = crate::util::pascal_case(remote.name.trim());
            if name.is_empty() || remote_names.iter().any(|n| n == &name) {
                continue;
            }
            remote_names.push(name.clone());
            let kind = if remote.kind.trim().eq_ignore_ascii_case("function")
                || remote.kind.trim().eq_ignore_ascii_case("remotefunction")
            {
                "function".to_string()
            } else {
                "event".to_string()
            };
            self.remotes.push(RemoteSpec {
                name,
                kind,
                notes: remote.notes.trim().to_string(),
            });
        }

        notes
    }

    /// Structural requirements for a project that Rojo and the Studio plugin
    /// can both materialise.
    pub fn validate(&self) -> Result<()> {
        if self.files.len() < MIN_FILES {
            bail!(
                "blueprint lists {} scripts, at least {MIN_FILES} are required",
                self.files.len()
            );
        }
        if self.files.len() > MAX_FILES {
            bail!(
                "blueprint lists {} scripts, at most {MAX_FILES} are allowed",
                self.files.len()
            );
        }
        let server = self.files_by_role(FileRole::Server).len();
        let shared = self.files_by_role(FileRole::Shared).len();
        if server == 0 {
            bail!("blueprint has no server script under src/server");
        }
        if shared == 0 {
            bail!("blueprint has no shared module under src/shared");
        }
        for file in &self.files {
            if !is_valid_script_path(&file.path) {
                bail!("invalid script path `{}`", file.path);
            }
            if file.purpose.is_empty() {
                bail!("script `{}` has no stated purpose", file.path);
            }
        }
        if self.systems.is_empty() {
            bail!("blueprint describes no gameplay systems");
        }
        Ok(())
    }

    pub fn files_by_role(&self, role: FileRole) -> Vec<&FilePlan> {
        self.files
            .iter()
            .filter(|f| FileRole::from_path(&f.path) == Some(role))
            .collect()
    }

    pub fn remote_names(&self) -> Vec<String> {
        self.remotes.iter().map(|r| r.name.clone()).collect()
    }

    /// The instance contract both Rojo and the Studio plugin must honour.
    pub fn contract(&self) -> String {
        let mut out = String::new();
        out.push_str("Instance contract (identical under Rojo and the Studio plugin):\n");
        out.push_str("- Shared modules are ModuleScripts in ReplicatedStorage.BloxCode.Shared (nested folders preserved).\n");
        out.push_str("- Server scripts are Scripts in ServerScriptService.BloxCode.\n");
        out.push_str(
            "- Client scripts are LocalScripts in StarterPlayer.StarterPlayerScripts.BloxCode.\n",
        );
        out.push_str("- Remotes live in ReplicatedStorage.BloxCode.Remotes:\n");
        if self.remotes.is_empty() {
            out.push_str("  (no remotes declared)\n");
        }
        for remote in &self.remotes {
            out.push_str(&format!(
                "  - {} ({}){}\n",
                remote.name,
                remote.class(),
                if remote.notes.is_empty() {
                    String::new()
                } else {
                    format!(": {}", remote.notes)
                }
            ));
        }
        out
    }
}

/// Normalise a model-provided path into `src/{shared,server,client}/**\/*.luau`
/// with the Rojo suffix conventions (`.server.luau`, `.client.luau`).
pub fn normalize_script_path(path: &str, hint: Option<FileRole>) -> Option<String> {
    let path = path.trim().replace('\\', "/");
    let path = path.trim_start_matches("./").trim_start_matches('/');
    if path.is_empty() || path.contains("..") {
        return None;
    }

    let role = FileRole::from_path(path).or(hint).or_else(|| {
        let lower = path.to_ascii_lowercase();
        if lower.contains("client") {
            Some(FileRole::Client)
        } else if lower.contains("server") {
            Some(FileRole::Server)
        } else {
            Some(FileRole::Shared)
        }
    })?;

    // Take the part after the role directory if the path already has one.
    let rest = if let Some(stripped) = path.strip_prefix(role.dir()) {
        stripped.to_string()
    } else {
        path.rsplit('/').next().unwrap_or(path).to_string()
    };

    let mut segments: Vec<String> = Vec::new();
    let raw_segments: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    if raw_segments.is_empty() {
        return None;
    }
    for segment in &raw_segments {
        let is_file = std::ptr::eq(*segment, raw_segments[raw_segments.len() - 1]);
        let mut cleaned = String::new();
        for ch in segment.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || (is_file && ch == '.') {
                cleaned.push(ch);
            }
        }
        let cleaned = cleaned.trim_matches(['.', '-', '_']);
        if cleaned.is_empty() {
            return None;
        }
        segments.push(cleaned.to_string());
    }
    if segments.len() > 4 {
        segments.truncate(4);
    }

    let file_name = segments.pop()?;
    let stem = file_name
        .trim_end_matches(".luau")
        .trim_end_matches(".lua")
        .trim_end_matches(".server")
        .trim_end_matches(".client")
        .to_string();
    let stem = crate::util::pascal_case(&stem);
    let file_name = match role {
        FileRole::Server => format!("{stem}.server.luau"),
        FileRole::Client => format!("{stem}.client.luau"),
        FileRole::Shared => format!("{stem}.luau"),
    };

    let mut full = String::from(role.dir());
    for segment in segments {
        full.push_str(&segment);
        full.push('/');
    }
    full.push_str(&file_name);
    Some(full)
}

pub fn is_valid_script_path(path: &str) -> bool {
    if !path.ends_with(".luau") || path.contains("..") || path.contains('\\') {
        return false;
    }
    if FileRole::from_path(path).is_none() {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or_default();
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plan() -> Plan {
        Plan {
            title: "Neon Tycoon".into(),
            pitch: "Build a neon empire".into(),
            core_loop: "Mine, upgrade, repeat".into(),
            systems: vec![SystemSpec {
                name: "Economy".into(),
                detail: "Cash and upgrades".into(),
            }],
            files: vec![
                FilePlan {
                    path: "src/shared/Economy.luau".into(),
                    role: Some(FileRole::Shared),
                    purpose: "Economy helpers".into(),
                },
                FilePlan {
                    path: "src/server/main.server.luau".into(),
                    role: Some(FileRole::Server),
                    purpose: "Server bootstrap".into(),
                },
                FilePlan {
                    path: "src/client/Hud.luau".into(),
                    role: Some(FileRole::Client),
                    purpose: "HUD".into(),
                },
            ],
            remotes: vec![RemoteSpec {
                name: "purchase".into(),
                kind: "event".into(),
                notes: "Buy upgrade".into(),
            }],
        }
    }

    #[test]
    fn normalize_applies_rojo_suffixes() {
        let mut plan = sample_plan();
        let notes = plan.normalize();
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(plan.files[0].path, "src/shared/Economy.luau");
        assert_eq!(plan.files[1].path, "src/server/Main.server.luau");
        assert_eq!(plan.files[2].path, "src/client/Hud.client.luau");
        assert_eq!(plan.remotes[0].name, "Purchase");
        plan.validate().expect("plan should validate");
    }

    #[test]
    fn normalize_moves_misplaced_files() {
        let mut plan = sample_plan();
        plan.files[0].path = "Economy.luau".into();
        plan.files[1].path = "server/Game.lua".into();
        plan.normalize();
        assert_eq!(plan.files[0].path, "src/shared/Economy.luau");
        assert_eq!(plan.files[1].path, "src/server/Game.server.luau");
    }

    #[test]
    fn traversal_is_rejected() {
        assert!(normalize_script_path("../evil.luau", None).is_none());
        assert!(!is_valid_script_path("src/shared/../../evil.luau"));
    }

    #[test]
    fn validation_requires_server_and_shared() {
        let mut plan = sample_plan();
        plan.files.retain(|f| !f.path.contains("server"));
        assert!(plan.validate().is_err());
    }

    #[test]
    fn duplicate_paths_collapse() {
        let mut plan = sample_plan();
        plan.files.push(plan.files[0].clone());
        plan.files.push(plan.files[0].clone());
        let notes = plan.normalize();
        assert_eq!(plan.files.len(), 3);
        assert!(notes.iter().any(|n| n.contains("duplicate")));
    }
}
