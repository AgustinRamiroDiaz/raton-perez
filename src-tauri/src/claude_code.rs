//! Post-processing through the user's installed Claude Code CLI (`claude -p`),
//! which uses their Claude login instead of an API key.

use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub const PROVIDER_ID: &str = "claude_code";
pub const DEFAULT_MODEL: &str = "haiku";
pub const MODELS: [&str; 3] = ["haiku", "sonnet", "opus"];

const SYSTEM_PROMPT: &str = "You are a text editing tool. Follow the instructions in the \
message and output only the resulting text, with no preamble or commentary.";

/// GUI apps (macOS especially) don't inherit the shell's PATH, so also try the
/// installer's usual locations.
fn claude_binary() -> PathBuf {
    let exe = if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    };
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(exe)));
    }
    if let Some(home) = home {
        candidates.push(home.join(".local/bin").join(exe));
        candidates.push(home.join(".claude/local").join(exe));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin").join(exe));
    candidates.push(PathBuf::from("/usr/local/bin").join(exe));
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(exe))
}

/// Sends `input` to Claude Code and returns its reply.
pub async fn run(model: &str, input: &str) -> Result<String, String> {
    let mut cmd = Command::new(claude_binary());
    cmd.args([
        "-p",
        "--model",
        model,
        // Plain text editing: no tools, MCP servers, project settings or saved session.
        "--tools",
        "",
        "--strict-mcp-config",
        "--setting-sources",
        "",
        "--no-session-persistence",
        "--system-prompt",
        SYSTEM_PROMPT,
    ])
    // Outside any project, so no CLAUDE.md gets loaded.
    .current_dir(std::env::temp_dir())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    // The post-processing timeout drops this future; take the process with it.
    .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start Claude Code (is `claude` installed?): {e}"))?;
    let mut stdin = child.stdin.take().ok_or("Claude Code stdin unavailable")?;
    stdin
        .write_all(input.as_bytes())
        .await
        .map_err(|e| format!("Writing to Claude Code failed: {e}"))?;
    drop(stdin);

    let out = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Claude Code failed: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Err(format!(
            "Claude Code exited with {}: {}",
            out.status,
            if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            }
            .trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    // Needs an installed, logged-in Claude Code: `cargo test claude_code -- --ignored`.
    #[test]
    #[ignore]
    fn edits_text_through_the_cli() {
        let out = tauri::async_runtime::block_on(super::run(
            super::DEFAULT_MODEL,
            "<text>\ni has went to the store yesterday\n</text>\nFix the grammar. Output only the result.",
        ))
        .unwrap();
        assert!(out.to_lowercase().contains("went to the store"), "{out}");
        assert!(!out.contains("<text>"), "{out}");
    }
}
