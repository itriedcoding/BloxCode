use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

use crate::plan::Plan;
use crate::util;

/// Rojo project tree. The instance layout here is mirrored exactly by the
/// Studio plugin so a project behaves the same whether it is built or synced.
pub fn project_json(plan: &Plan) -> Value {
    let mut root = json!({
        "name": plan.title,
        "tree": {
            "$className": "DataModel",
            "ReplicatedStorage": {
                "$className": "ReplicatedStorage",
                "BloxCode": {
                    "$className": "Folder",
                    "Shared": {
                        "$className": "Folder",
                        "$path": "src/shared"
                    },
                    "Remotes": {
                        "$className": "Folder",
                        "$ignoreUnknownInstances": true
                    }
                }
            },
            "ServerScriptService": {
                "$className": "ServerScriptService",
                "BloxCode": {
                    "$className": "Folder",
                    "$path": "src/server"
                }
            },
            "StarterPlayer": {
                "$className": "StarterPlayer",
                "StarterPlayerScripts": {
                    "$className": "StarterPlayerScripts",
                    "BloxCode": {
                        "$className": "Folder",
                        "$path": "src/client"
                    }
                }
            }
        }
    });

    if let Some(Value::Object(remotes)) =
        root.pointer_mut("/tree/ReplicatedStorage/BloxCode/Remotes")
    {
        let mut built = Map::new();
        for remote in &plan.remotes {
            built.insert(remote.name.clone(), json!({ "$className": remote.class() }));
        }
        remotes.extend(built);
    }

    root
}

/// Locate the Rojo CLI: explicit setting first, then `PATH`.
pub fn find_cli(configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
    }
    util::which("rojo").or_else(|| util::which("rojo.exe"))
}

pub fn version(cli: &Path) -> Option<String> {
    let output = util::hidden_command(cli).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next().unwrap_or("").trim().to_string();
    if first.is_empty() {
        Some("rojo".into())
    } else {
        Some(first)
    }
}

/// `rojo build` the project into a `.rbxlx` place file.
pub fn build(cli: &Path, project_dir: &Path, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let result = util::hidden_command(cli)
        .current_dir(project_dir)
        .args(["build", "default.project.json", "--output"])
        .arg(output)
        .output()
        .with_context(|| format!("running {}", cli.display()))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let stdout = String::from_utf8_lossy(&result.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        };
        bail!("rojo build failed: {}", util::truncate(detail.trim(), 600));
    }
    if !output.is_file() {
        bail!(
            "rojo reported success but {} was not created",
            output.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{RemoteSpec, SystemSpec};

    fn plan() -> Plan {
        Plan {
            title: "Demo".into(),
            pitch: "p".into(),
            core_loop: "c".into(),
            systems: vec![SystemSpec {
                name: "S".into(),
                detail: "d".into(),
            }],
            files: vec![],
            remotes: vec![RemoteSpec {
                name: "Purchase".into(),
                kind: "event".into(),
                notes: String::new(),
            }],
        }
    }

    #[test]
    fn tree_matches_bridge_namespace() {
        let value = project_json(&plan());
        let tree = value.get("tree").unwrap();
        let bloxcode = tree
            .get("ReplicatedStorage")
            .and_then(|v| v.get("BloxCode"))
            .expect("BloxCode folder");
        assert_eq!(
            bloxcode.get("Shared").and_then(|v| v.get("$path")).unwrap(),
            "src/shared"
        );
        assert_eq!(
            bloxcode
                .get("Remotes")
                .and_then(|v| v.get("Purchase"))
                .and_then(|v| v.get("$className"))
                .unwrap(),
            "RemoteEvent"
        );
        assert!(tree
            .get("ServerScriptService")
            .and_then(|v| v.get("BloxCode"))
            .and_then(|v| v.get("$path"))
            .is_some());
    }

    #[test]
    fn remote_functions_use_remote_function_class() {
        let mut plan = plan();
        plan.remotes[0].kind = "function".into();
        let value = project_json(&plan);
        assert_eq!(
            value
                .pointer("/tree/ReplicatedStorage/BloxCode/Remotes/Purchase/$className")
                .unwrap(),
            "RemoteFunction"
        );
    }
}
