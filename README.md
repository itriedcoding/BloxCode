# BloxCode

A Windows desktop app for building complete Roblox games with AI. Describe a game, and BloxCode designs the systems, writes the Luau, packages a Rojo project and syncs it into Roblox Studio.

Built in Rust with `eframe`/`egui`. Black and white, no telemetry, no accounts beyond your own AI key.

![BloxCode mark](docs/icon.png)

## Download

Grab `BloxCode.exe` from [Releases](https://github.com/itriedcoding/BloxCode/releases). It is a single portable executable — no installer, no runtime dependencies.

Windows SmartScreen may warn because the binary is unsigned. Choose **More info → Run anyway**, or verify the SHA-256 listed with the release.

## What it does

- **Whole games, not snippets.** A two-pass pipeline: first a structured blueprint (systems, files, remotes), then the complete Luau sources, validated before anything is written to disk.
- **Server-authoritative Luau.** Generated code uses `task.wait`, `GetService`, `WaitForChild` remotes, guarded DataStores and client UI built in code. No placeholders, no TODOs.
- **A real project on disk.** `src/shared`, `src/server`, `src/client`, a Rojo `default.project.json`, README and gitignore. Versioned locally with a revision counter.
- **Roblox Studio integration.** A local bridge serves the current build; the bundled Studio plugin materialises scripts under `ServerScriptService.BloxCode`, `ReplicatedStorage.BloxCode` and `StarterPlayerScripts.BloxCode`.
- **Focused edits.** Ask for a shop, a save system or a UI pass. Only the files that need to change are rewritten, and new remotes are declared and created.
- **Code editor.** File tree, Luau syntax highlighting, line numbers, Ctrl+S, unsaved markers.
- **Optional Rojo build.** Produces a `.rbxlx` place file when the Rojo CLI is installed.

## Vly AI

BloxCode talks to the Vly AI gateway (`https://integrations.vly.ai/v1/llm`), an OpenAI-compatible chat completions endpoint authenticated with a Vly deployment token.

1. Create a Vly deployment and copy its deployment token.
2. Open **Settings → Vly AI** and paste it into *Deployment token*.
3. Press **Test connection**.

The token is stored in `%APPDATA%\BloxCode\config.json` and is never written into generated projects. You can also set `VLY_INTEGRATION_KEY` in the environment to override the stored token. Model, base URL and temperature are configurable.

## Roblox Studio

1. Open a project in BloxCode.
2. Go to **Studio → Install plugin**.
3. In Roblox Studio, enable **Game Settings → Security → Allow HTTP Requests**.
4. Open the **BloxCode** toolbar button and press **SYNC NOW** (auto sync is available).

The bridge listens on `127.0.0.1` only and requires a per-install token. Scripts are replaced only inside the BloxCode folders it manages; the plugin never touches the rest of your place.

If you rotate the bridge token or change the port, save Settings — the plugin is reinstalled with the new values.

## Build from source

Requirements: Rust 1.79+ and, on Windows, the MSVC toolchain.

```bash
cargo test -p bloxcode-core      # engine tests
cargo build --release -p bloxcode-app
./target/release/bloxcode.exe    # or target\release\bloxcode.exe
```

The executable embeds the IBM Plex fonts and the Studio plugin; nothing needs to ship beside it.

Optional tools:

```bash
python3 tools/prepare_fonts.py   # regenerate the bundled static font weights
python3 tools/make_icon.py       # regenerate icon.ico and the window icon
```

## CLI

| Command | Purpose |
| --- | --- |
| `bloxcode` | Launch the app |
| `bloxcode --doctor` | Print configuration, bridge, plugin and Rojo diagnostics to the console and `%APPDATA%\BloxCode\doctor.txt` |
| `bloxcode --smoke-ui` | Render every screen headlessly; used by CI |
| `bloxcode --version` | Print the version |

## Architecture

```
crates/bloxcode-core        engine, no UI dependencies
  ai/                       Vly gateway client, JSON extraction, prompts
  generator.rs              two-pass generation, batching, revision parsing
  plan.rs                   blueprint schema, path normalisation, validation
  project.rs                project storage, snapshots, safe file IO, revisions
  rojo.rs                   Rojo project tree + `rojo build`
  bridge.rs                 token-authenticated loopback HTTP bridge
  studio.rs                 Studio plugin rendering and installation
  assets/BloxCode.luau      the Studio plugin

crates/bloxcode-app         desktop UI
  theme.rs                  monochrome design tokens and bundled typography
  widgets.rs                hand-painted controls (buttons, fields, rails)
  editor.rs                 Luau tokenizer and editor gutter
  jobs.rs                   worker threads and progress streaming
  views/                    Home, Build, Files, Studio, Settings
```

Generated projects follow one instance contract everywhere:

```
ReplicatedStorage.BloxCode.Shared      ModuleScripts (nested folders preserved)
ReplicatedStorage.BloxCode.Remotes     RemoteEvents / RemoteFunctions
ServerScriptService.BloxCode           Scripts
StarterPlayer.StarterPlayerScripts.BloxCode  LocalScripts
```

Rojo and the Studio plugin both materialise this exact tree, so a project behaves the same however you open it.

## Privacy

- Prompts and current sources are sent only to the Vly gateway you configure.
- No analytics, crash reporting or background network calls.
- The Studio bridge binds to loopback, requires a token, and serves only the open project.

## Limitations

- Live sync requires Roblox Studio's *Allow HTTP Requests* setting.
- `.rbxlx` output requires the Rojo CLI; sync works without it.
- Generated scripts are managed by BloxCode — manual edits inside its managed folders will be overwritten on the next sync.
- The AI needs a valid Vly deployment token and network access.

## License

MIT — see [LICENSE](LICENSE). Bundled IBM Plex fonts are licensed under the SIL Open Font License.
