#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod editor;
mod jobs;
mod theme;
mod views;
mod widgets;

use bloxcode_core::{config::Config, paths, rojo, studio, VERSION};
use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "BloxCode {VERSION}\n\nUsage:\n  bloxcode            launch the app\n  bloxcode --doctor   print diagnostics and write doctor.txt\n  bloxcode --version  print the version"
        );
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("BloxCode {VERSION}");
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--doctor") {
        doctor();
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--smoke-ui") {
        smoke_ui();
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 880.0])
            .with_min_inner_size([1060.0, 720.0])
            .with_title("BloxCode")
            .with_icon(window_icon()),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "BloxCode",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

fn window_icon() -> egui::IconData {
    const SIZE: u32 = 64;
    egui::IconData {
        rgba: include_bytes!("../assets/icon-64.rgba").to_vec(),
        width: SIZE,
        height: SIZE,
    }
}

/// Drive every screen through a real egui context without a window or GPU.
/// This catches layout panics and invalid widget usage in CI.
fn smoke_ui() {
    use bloxcode_core::generator::GeneratedFile;
    use bloxcode_core::plan::{FilePlan, Plan, RemoteSpec, SystemSpec};
    use bloxcode_core::project;

    let ctx = egui::Context::default();
    let mut app = app::App::bootstrap(&ctx);

    let root = std::env::temp_dir().join(format!(
        "bloxcode-smoke-{}",
        bloxcode_core::util::random_hex(4)
    ));
    let plan = Plan {
        title: "Smoke Test Game".into(),
        pitch: "A test project used to exercise the BloxCode interface.".into(),
        core_loop: "Join, play, leave.".into(),
        systems: vec![SystemSpec {
            name: "Economy".into(),
            detail: "Cash and upgrades".into(),
        }],
        files: vec![
            FilePlan {
                path: "src/shared/Config.luau".into(),
                role: None,
                purpose: "Tuning".into(),
            },
            FilePlan {
                path: "src/server/Main.server.luau".into(),
                role: None,
                purpose: "Bootstrap".into(),
            },
        ],
        remotes: vec![RemoteSpec {
            name: "Purchase".into(),
            kind: "event".into(),
            notes: "Buy upgrade".into(),
        }],
    };
    let files = vec![
        GeneratedFile {
            path: "src/shared/Config.luau".into(),
            content: "--!strict\nreturn { startingCash = 100 }\n".into(),
        },
        GeneratedFile {
            path: "src/server/Main.server.luau".into(),
            content: "local Players = game:GetService(\"Players\")\nprint(\"boot\")\n".into(),
        },
    ];

    if let Ok(loaded) = project::create_project(&root, &plan, &files) {
        app.open_project(loaded);
    }
    if let Some(session) = app.session.as_mut() {
        session.push_chat(
            app::ChatRole::User,
            "Add a shop with three drills and server-validated purchases.",
        );
        session.push_chat(
            app::ChatRole::Assistant,
            "Added a shared item table and a purchase remote.",
        );
        session.dirty.insert("src/shared/Config.luau".into());
    }
    app.idea = "A co-op mining game with upgradeable drills.".into();

    let screens = [
        app::Screen::Home,
        app::Screen::Build,
        app::Screen::Files,
        app::Screen::Studio,
        app::Screen::Settings,
    ];

    for (index, screen) in screens.iter().enumerate() {
        app.screen = *screen;
        for frame in 0..2 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(1360.0, 880.0),
                )),
                time: Some(index as f64 + frame as f64 * 0.016),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| app.ui(ctx));
        }
        println!("screen ok: {screen:?}");
    }

    let shapes = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1360.0, 880.0),
            )),
            ..Default::default()
        },
        |ctx| app.ui(ctx),
    );
    println!("primitives: {}", shapes.shapes.len());
    println!("UI smoke test passed");

    let _ = std::fs::remove_dir_all(&root);
}

fn doctor() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Console::AllocConsole();
    }

    let config = Config::load();
    let mut report = Vec::new();
    report.push(format!("BloxCode {VERSION}"));
    report.push(format!("os: {}", std::env::consts::OS));
    report.push(format!("config: {}", paths::config_file().display()));
    report.push(format!(
        "vly token: {}",
        if config.effective_token().is_empty() {
            "missing"
        } else {
            "configured"
        }
    ));
    report.push(format!("gateway: {}", config.base_url));
    report.push(format!("model: {}", config.model));
    report.push(format!("projects: {}", config.projects_root));
    report.push(format!("bridge port: {}", config.bridge_port));
    report.push(format!(
        "studio plugin: {}",
        if studio::is_installed() {
            studio::plugin_path().display().to_string()
        } else {
            "not installed".to_string()
        }
    ));
    match rojo::find_cli(&config.rojo_path) {
        Some(cli) => report.push(format!(
            "rojo: {} ({})",
            cli.display(),
            rojo::version(&cli).unwrap_or_else(|| "unknown version".into())
        )),
        None => report.push("rojo: not found".into()),
    }
    report.push(format!(
        "projects found: {}",
        bloxcode_core::project::load_all(&config.projects_root_path()).len()
    ));

    let text = report.join("\n");
    println!("{text}");

    let target = paths::app_dir().join("doctor.txt");
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&target, format!("{text}\n"));
    println!("\nWritten to {}", target.display());
}
