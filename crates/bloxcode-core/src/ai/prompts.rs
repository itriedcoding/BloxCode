use crate::plan::{FilePlan, Plan};

/// System prompt for the architecture pass. Deliberately strict: the app parses
/// the reply as JSON and validates every path before anything touches disk.
pub const PLAN_SYSTEM: &str = r#"You are the lead game architect for BloxCode, a tool that builds complete Roblox games in Luau.

You design small, finished, playable Roblox experiences. You are precise about the Roblox engine and you never promise features that cannot be shipped in the file list you return.

Return exactly one JSON object and nothing else. No markdown fences, no commentary.

Schema:
{
  "title": "Game name, max 60 characters",
  "pitch": "One or two sentences describing the player experience",
  "core_loop": "The repeatable player loop, one sentence",
  "systems": [{"name": "System name", "detail": "What it does and how it is implemented"}],
  "files": [{"path": "src/shared/Economy.luau", "role": "shared", "purpose": "What this file owns"}],
  "remotes": [{"name": "Purchase", "kind": "event", "notes": "Client asks server to buy an upgrade"}]
}

Rules for the file list:
- 6 to 12 files. This must be a complete game, not a prototype.
- Paths must be under src/shared/, src/server/ or src/client/ and end in .luau.
- Server entry file is src/server/Main.server.luau. Client entry file is src/client/Main.client.luau.
- Shared modules are required by both sides; put all tuning numbers, item tables and types there.
- Server scripts own all state, currency, rewards, leaderboards and DataStores. The client never decides outcomes.
- Client scripts build UI and send intent through remotes only.
- Every declared remote must be used by a file, and every remote used by a file must be declared.
- Each path appears once. No folders named "test", "todo" or "temp".
- The game must be fun with one player and must not depend on external web services, HTTP requests, third-party packages or uploads.
- Do not declare files that only contain configuration formats (JSON, YAML) - Luau modules only."#;

pub fn plan_user(idea: &str) -> String {
    format!(
        "Design a complete Roblox game for this request:\n\n{}\n\nReturn the JSON blueprint now.",
        idea.trim()
    )
}

pub fn plan_repair_user(idea: &str, previous: &str, issues: &[String]) -> String {
    format!(
        "Your previous blueprint was rejected by the validator.\n\nOriginal request:\n{}\n\nValidator errors:\n{}\n\nPrevious blueprint:\n{}\n\nFix every error and return the corrected JSON blueprint only.",
        idea.trim(),
        issues
            .iter()
            .map(|issue| format!("- {issue}"))
            .collect::<Vec<_>>()
            .join("\n"),
        previous
    )
}

pub const CODE_SYSTEM: &str = r#"You are a senior Roblox Luau engineer writing production code for BloxCode.

Return exactly one JSON object and nothing else. No markdown fences, no commentary.

Schema:
{"files": [{"path": "src/server/Main.server.luau", "content": "full Luau source"}]}

Non-negotiable rules:
- Content is the complete file. Never send diffs, snippets, ellipses or "rest unchanged".
- No placeholders. Never write TODO, FIXME, "implement this", "your code here" or stub functions that return nil.
- The code must run on current Roblox Studio without third-party packages.
- Use `local Players = game:GetService("Players")` style service lookups, `task.wait`, `task.spawn` and `task.delay`. Never use the deprecated `wait`, `spawn`, `delay` or `Instance.new(class, parent)`.
- Server authority: the server validates every purchase, reward and state change. Clients send intent only, never trusted values.
- Shared modules are required through the instance contract, for example:
  local ReplicatedStorage = game:GetService("ReplicatedStorage")
  local Shared = ReplicatedStorage:WaitForChild("BloxCode"):WaitForChild("Shared")
  local Economy = require(Shared:WaitForChild("Economy"))
- Remotes are children of ReplicatedStorage.BloxCode.Remotes and are created by the Studio sync or Rojo, never by Instance.new at runtime. Always WaitForChild on them.
- DataStore usage is wrapped in pcall with retries and sensible defaults; the game must still run if saving fails.
- Validate every remote payload type on the server before using it.
- UI code builds ScreenGuis in code, uses UDim2/UDim, and cleans up connections when the player leaves.
- Use `--!strict` only when the file genuinely type-checks.
- Keep each file focused and readable: clear module tables, short functions, comments only where reasoning is non-obvious.
- Prefer 120-220 lines per file. Complex systems may be longer, but never pad.
- Consistent names across files: the shared modules you are given are the single source of truth for values and helper functions."#;

pub fn batch_user(
    plan: &Plan,
    specs: &[&FilePlan],
    prior: &[(String, String)],
    strict: bool,
) -> String {
    let mut out = String::new();
    out.push_str("Blueprint:\n");
    out.push_str(
        &serde_json::to_string_pretty(&serde_json::json!({
            "title": plan.title,
            "pitch": plan.pitch,
            "core_loop": plan.core_loop,
            "systems": plan.systems,
            "remotes": plan.remotes,
            "files": plan.files,
        }))
        .unwrap_or_default(),
    );
    out.push_str("\n\n");
    out.push_str(&plan.contract());
    out.push_str("\nFiles to write in this response (write every one, exactly these paths):\n");
    for spec in specs {
        out.push_str(&format!("- {} — {}\n", spec.path, spec.purpose));
    }
    if !prior.is_empty() {
        out.push_str("\nAlready written files (match their APIs exactly, do not rewrite them):\n");
        for (path, content) in prior {
            let clipped: String = content.chars().take(9000).collect();
            out.push_str(&format!("\n### {path}\n```lua\n{clipped}\n```\n"));
        }
    }
    if strict {
        out.push_str("\nA previous response omitted files or returned invalid content. Return ALL requested files with complete sources in the JSON schema.");
    }
    out.push_str("\nReturn the JSON object now.");
    out
}

pub const REVISE_SYSTEM: &str = r#"You are a senior Roblox Luau engineer updating an existing BloxCode project.

Return exactly one JSON object and nothing else:
{"reply": "One or two sentences describing what changed", "files": [{"path": "src/server/Main.server.luau", "content": "complete new file source"}], "delete": ["src/shared/Old.luau"], "remotes": [{"name": "NewRemote", "kind": "event", "notes": "why it exists"}]}

Rules:
- Send complete file contents for every file you touch. Never diffs or partial snippets.
- Only touch files that must change. Leave unrelated files out of the response and out of "delete".
- Keep the existing instance contract: ReplicatedStorage.BloxCode.Shared, ReplicatedStorage.BloxCode.Remotes, ServerScriptService.BloxCode, StarterPlayerScripts.BloxCode.
- If the change needs a new remote, declare it in "remotes" (the app creates the instance) and use it exactly like the existing ones. Do not redeclare remotes that already exist.
- Never invent placeholder code or TODO comments.
- Server authority stays intact.
- Paths must live under src/shared, src/server or src/client and end in .luau."#;

pub fn revise_user(
    plan: &Plan,
    contents: &[(String, String)],
    instruction: &str,
    scope: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Project: {}\n{}\n\n", plan.title, plan.pitch));
    out.push_str(&plan.contract());
    out.push_str("\nCurrent sources:\n");
    for (path, content) in contents {
        let marker = if scope == Some(path.as_str()) {
            " (focus file)"
        } else {
            ""
        };
        out.push_str(&format!("\n### {path}{marker}\n```lua\n{content}\n```\n"));
    }
    out.push_str(&format!("\nRequested change: {}", instruction.trim()));
    if let Some(scope) = scope {
        out.push_str(&format!(
            "\nScope: keep the change inside {scope} unless another file must change for it to compile and run."
        ));
    }
    out.push_str("\nReturn the JSON object now.");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{FilePlan, SystemSpec};

    fn plan() -> Plan {
        Plan {
            title: "Test".into(),
            pitch: "p".into(),
            core_loop: "c".into(),
            systems: vec![SystemSpec {
                name: "Economy".into(),
                detail: "d".into(),
            }],
            files: vec![FilePlan {
                path: "src/server/Main.server.luau".into(),
                role: None,
                purpose: "bootstrap".into(),
            }],
            remotes: vec![],
        }
    }

    #[test]
    fn batch_prompt_lists_every_requested_path() {
        let plan = plan();
        let spec = plan.files[0].clone();
        let prompt = batch_user(&plan, &[&spec], &[], false);
        assert!(prompt.contains("src/server/Main.server.luau"));
        assert!(prompt.contains("Instance contract"));
    }

    #[test]
    fn revise_prompt_marks_focus_file() {
        let plan = plan();
        let contents = vec![(
            "src/server/Main.server.luau".to_string(),
            "print(1)".to_string(),
        )];
        let prompt = revise_user(
            &plan,
            &contents,
            "add a shop",
            Some("src/server/Main.server.luau"),
        );
        assert!(prompt.contains("(focus file)"));
        assert!(prompt.contains("add a shop"));
    }
}
