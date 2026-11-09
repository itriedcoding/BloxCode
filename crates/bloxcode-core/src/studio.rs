//! Installs the Roblox Studio plugin that pulls builds from the bridge.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::paths;

pub const PLUGIN_FILE: &str = "BloxCode.luau";
pub const PLUGIN_TEMPLATE: &str = include_str!("../assets/BloxCode.luau");

pub fn render_plugin(port: u16, token: &str) -> String {
    PLUGIN_TEMPLATE
        .replace("__BLOXCODE_PORT__", &port.to_string())
        .replace("__BLOXCODE_TOKEN__", token)
        .replace("__BLOXCODE_VERSION__", crate::VERSION)
}

pub fn plugins_dir() -> PathBuf {
    paths::roblox_plugins_dir().unwrap_or_else(|| paths::app_dir().join("roblox-plugins"))
}

pub fn plugin_path() -> PathBuf {
    plugins_dir().join(PLUGIN_FILE)
}

pub fn is_installed() -> bool {
    plugin_path().is_file()
}

pub fn install(port: u16, token: &str) -> Result<PathBuf> {
    install_to(&plugins_dir(), port, token)
}

pub fn install_to(dir: &Path, port: u16, token: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(PLUGIN_FILE);
    std::fs::write(&path, render_plugin(port, token))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_has_no_unreplaced_placeholders() {
        let rendered = render_plugin(42_811, "secret-token");
        assert!(rendered.contains("local DEFAULT_PORT = 42811"));
        assert!(rendered.contains("secret-token"));
        assert!(!rendered.contains("__BLOXCODE_"));
        assert!(rendered.contains(crate::VERSION));
    }

    #[test]
    fn installs_into_a_directory() {
        let dir =
            std::env::temp_dir().join(format!("bloxcode-plugin-{}", crate::util::random_hex(4)));
        let path = install_to(&dir, 42_811, "tok").expect("install");
        assert!(path.is_file());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("tok"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
