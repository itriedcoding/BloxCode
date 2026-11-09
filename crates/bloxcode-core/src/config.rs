use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;
use crate::util;

/// Vly AI gateway base URL (OpenAI-compatible), from `@vly-ai/integrations`.
pub const DEFAULT_BASE_URL: &str = "https://integrations.vly.ai/v1/llm";
pub const DEFAULT_MODEL: &str = "gpt-5";
pub const DEFAULT_PORT: u16 = 42_811;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Vly deployment token (`VLY_INTEGRATION_KEY`).
    pub vly_token: String,
    /// OpenAI-compatible base URL; `/chat/completions` is appended.
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
    /// Local bridge port used by the Roblox Studio plugin.
    pub bridge_port: u16,
    /// Shared secret required by the bridge; generated once and reused.
    pub bridge_token: String,
    pub projects_root: String,
    /// Optional explicit Rojo CLI path.
    pub rojo_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vly_token: String::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            temperature: 0.4,
            bridge_port: DEFAULT_PORT,
            bridge_token: util::random_hex(24),
            projects_root: paths::default_projects_root().display().to_string(),
            rojo_path: String::new(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let mut config = std::fs::read_to_string(paths::config_file())
            .ok()
            .and_then(|raw| serde_json::from_str::<Config>(&raw).ok())
            .unwrap_or_default();
        config.normalize();
        config
    }

    /// Persist to `%APPDATA%\BloxCode\config.json`. Contains the Vly token;
    /// the file lives in the user's roaming profile and is never committed.
    pub fn save(&self) -> Result<()> {
        let dir = paths::app_dir();
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = paths::config_file();
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, raw).with_context(|| format!("writing {}", path.display()))?;
        restrict_permissions(&path);
        Ok(())
    }

    pub fn normalize(&mut self) {
        if self.base_url.trim().is_empty() {
            self.base_url = DEFAULT_BASE_URL.to_string();
        }
        self.base_url = self.base_url.trim().trim_end_matches('/').to_string();
        if self.model.trim().is_empty() {
            self.model = DEFAULT_MODEL.to_string();
        }
        self.model = self.model.trim().to_string();
        if self.bridge_token.len() < 16 {
            self.bridge_token = util::random_hex(24);
        }
        if self.bridge_port < 1024 {
            self.bridge_port = DEFAULT_PORT;
        }
        self.temperature = self.temperature.clamp(0.0, 1.5);
        if self.projects_root.trim().is_empty() {
            self.projects_root = paths::default_projects_root().display().to_string();
        }
    }

    /// Token from settings, unless `VLY_INTEGRATION_KEY` is set in the environment.
    pub fn effective_token(&self) -> String {
        for key in ["VLY_INTEGRATION_KEY", "VLY_API_KEY"] {
            if let Ok(value) = std::env::var(key) {
                if !value.trim().is_empty() {
                    return value.trim().to_string();
                }
            }
        }
        self.vly_token.trim().to_string()
    }

    pub fn projects_root_path(&self) -> PathBuf {
        PathBuf::from(self.projects_root.trim())
    }
}

#[cfg(windows)]
fn restrict_permissions(path: &std::path::Path) {
    // %APPDATA% is already per-user; tighten the ACL further via icacls so the
    // token is not readable by other local accounts.
    let _ = util::hidden_command(std::path::Path::new("icacls"))
        .arg(path)
        .args(["/inheritance:r", "/grant:r", "%USERNAME%:F"])
        .output();
}

#[cfg(not(windows))]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_repairs_empty_values() {
        let mut config = Config {
            base_url: "  https://integrations.vly.ai/v1/llm/  ".into(),
            model: "  ".into(),
            bridge_port: 80,
            bridge_token: "short".into(),
            projects_root: " ".into(),
            ..Default::default()
        };
        config.normalize();
        assert_eq!(config.base_url, "https://integrations.vly.ai/v1/llm");
        assert_eq!(config.model, DEFAULT_MODEL);
        assert_eq!(config.bridge_port, DEFAULT_PORT);
        assert!(config.bridge_token.len() >= 16);
        assert!(!config.projects_root.is_empty());
    }

    #[test]
    fn default_token_is_random_per_install() {
        assert_ne!(
            Config::default().bridge_token,
            Config::default().bridge_token
        );
    }
}
