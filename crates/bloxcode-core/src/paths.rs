use std::path::PathBuf;

/// `%APPDATA%\BloxCode` on Windows, `~/.config/BloxCode` elsewhere.
pub fn app_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("BloxCode")
}

pub fn config_file() -> PathBuf {
    app_dir().join("config.json")
}

/// Default location for generated games: `Documents/BloxCode`.
pub fn default_projects_root() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("BloxCode")
}

/// Roblox Studio plugin folder. On Windows this is `%LOCALAPPDATA%\Roblox\Plugins`.
pub fn roblox_plugins_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|base| base.join("Roblox").join("Plugins"))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Best-effort Studio install roots, used only for "open Studio" affordances.
pub fn roblox_studio_versions_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|base| base.join("Roblox").join("Versions"))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn is_windows() -> bool {
    cfg!(windows)
}
