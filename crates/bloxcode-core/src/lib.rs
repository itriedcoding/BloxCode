//! Core engine for BloxCode: Vly AI gateway client, game blueprint planning,
//! Luau project generation, Rojo packaging and the local Roblox Studio bridge.

pub mod ai;
pub mod bridge;
pub mod config;
pub mod generator;
pub mod paths;
pub mod plan;
pub mod project;
pub mod rojo;
pub mod studio;
pub mod util;

/// Version reported by the local bridge and the UI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Contract namespace mirrored by both the Rojo tree and the Studio plugin.
pub const NAMESPACE: &str = "BloxCode";
