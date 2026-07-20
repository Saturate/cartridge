use super::Provider;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ProviderOptions {
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub permission_mode: Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub append_system_prompt: Option<String>,
    pub mcp_servers: Option<String>,
    pub resume: Option<String>,
    pub approve: Option<bool>,
    pub tools: Option<Vec<String>>,
    pub exclude_tools: Option<Vec<String>>,
    pub no_skills: Option<bool>,
    pub no_extensions: Option<bool>,
    pub auto_approve: Option<bool>,
    pub full_auto: Option<bool>,
    pub quiet: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Provider {
    /// Build the CLI command for interactive mode (no prompt baked in).
    /// The prompt is sent separately via PTY input after the CLI starts.
    pub fn build_command(
        &self,
        options: &ProviderOptions,
        safe_mode: bool,
        custom_command: Option<&[String]>,
    ) -> Vec<String> {
        match self {
            Provider::Claude => build_claude(options, safe_mode),
            Provider::Pi => build_pi(options, safe_mode),
            Provider::Opencode => build_opencode(options, safe_mode),
            Provider::Codex => build_codex(options, safe_mode),
            Provider::Gemini => build_gemini(options, safe_mode),
            Provider::Custom => custom_command.unwrap_or(&[]).to_vec(),
        }
    }

    pub fn build_headless_command(
        &self,
        prompt: &str,
        options: &ProviderOptions,
        safe_mode: bool,
        custom_command: Option<&[String]>,
    ) -> Option<Vec<String>> {
        match self {
            Provider::Claude => {
                let mut cmd = build_claude(options, safe_mode);
                cmd.extend(["-p".into(), prompt.into(), "--output-format".into(), "json".into()]);
                Some(cmd)
            }
            Provider::Codex => {
                let mut cmd = build_codex(options, safe_mode);
                cmd.push(prompt.into());
                Some(cmd)
            }
            Provider::Opencode => {
                // `opencode run` is a distinct subcommand from the interactive TUI,
                // so we build it from scratch rather than via build_opencode().
                // Keep in sync with build_opencode() if new shared flags are added.
                let mut cmd = vec!["opencode".into(), "run".into(), prompt.into()];
                cmd.extend(["--format".into(), "json".into()]);
                if let Some(model) = &options.model {
                    cmd.extend(["--model".into(), model.clone()]);
                }
                Some(cmd)
            }
            Provider::Custom => Some(custom_command.unwrap_or(&[]).to_vec()),
            Provider::Pi | Provider::Gemini => None,
        }
    }

    pub fn startup_delay_ms(&self) -> u64 {
        match self {
            Provider::Claude => 3000,
            Provider::Pi => 2000,
            Provider::Opencode => 2000,
            Provider::Codex => 2000,
            Provider::Gemini => 2000,
            Provider::Custom => 500,
        }
    }
}

fn build_claude(opts: &ProviderOptions, safe_mode: bool) -> Vec<String> {
    let mut cmd = vec!["claude".into()];

    if let Some(mode) = &opts.permission_mode {
        cmd.push("--permission-mode".into());
        cmd.push(mode.clone());
    }
    if let Some(model) = &opts.model {
        cmd.push("--model".into());
        cmd.push(model.clone());
    }
    if let Some(turns) = opts.max_turns {
        cmd.push("--max-turns".into());
        cmd.push(turns.to_string());
    }
    if let Some(tools) = &opts.allowed_tools {
        cmd.push("--allowedTools".into());
        cmd.push(tools.join(","));
    }
    if let Some(sys) = &opts.append_system_prompt {
        cmd.push("--append-system-prompt".into());
        cmd.push(sys.clone());
    }
    if let Some(mcp) = &opts.mcp_servers {
        cmd.push("--mcp-config".into());
        cmd.push(mcp.clone());
    }
    if let Some(session) = &opts.resume {
        cmd.push("--resume".into());
        cmd.push(session.clone());
    }

    let _ = safe_mode;
    cmd
}

fn build_pi(opts: &ProviderOptions, safe_mode: bool) -> Vec<String> {
    let mut cmd = vec!["pi".into()];

    let approve = opts.approve.unwrap_or(!safe_mode);
    if approve {
        cmd.push("--approve".into());
    } else {
        cmd.push("--no-approve".into());
    }

    let model = opts.model.clone().or_else(default_pi_model);
    if let Some(model) = model {
        cmd.push("--model".into());
        cmd.push(model);
    }
    if let Some(tools) = &opts.tools {
        cmd.push("--tools".into());
        cmd.push(tools.join(","));
    }
    if let Some(exclude) = &opts.exclude_tools {
        cmd.push("--exclude-tools".into());
        cmd.push(exclude.join(","));
    }
    if opts.no_skills.unwrap_or(false) {
        cmd.push("--no-skills".into());
    }
    if opts.no_extensions.unwrap_or(false) {
        cmd.push("--no-extensions".into());
    }

    cmd
}

fn build_opencode(opts: &ProviderOptions, _safe_mode: bool) -> Vec<String> {
    // OpenCode doesn't have a CLI auto-approve flag.
    // Permission is controlled via opencode.jsonc config ("permission": "allow").
    // The bootstrap writes this config when not in safe mode.
    let mut cmd = vec!["opencode".into()];

    if let Some(model) = &opts.model {
        cmd.push("--model".into());
        cmd.push(model.clone());
    }

    cmd
}

fn build_codex(opts: &ProviderOptions, safe_mode: bool) -> Vec<String> {
    // codex exec is the headless subcommand (--full-auto is deprecated)
    let mut cmd = vec!["codex".into(), "exec".into()];

    let bypass = opts.full_auto.unwrap_or(!safe_mode);
    if bypass {
        cmd.push("--dangerously-bypass-approvals-and-sandbox".into());
    }

    if let Some(model) = &opts.model {
        cmd.push("--model".into());
        cmd.push(model.clone());
    }

    cmd
}

fn build_gemini(opts: &ProviderOptions, safe_mode: bool) -> Vec<String> {
    let mut cmd = vec!["gemini".into()];

    let yolo = opts.auto_approve.unwrap_or(!safe_mode);
    if yolo {
        cmd.push("--yolo".into());
    }

    if let Some(model) = &opts.model {
        cmd.push("--model".into());
        cmd.push(model.clone());
    }

    cmd
}

fn default_pi_model() -> Option<String> {
    let candidates = [
        format!(
            "{}/.pi/agent/models.json",
            std::env::var("HOME").unwrap_or_default()
        ),
        "/home/dev/.pi/agent/models.json".to_string(),
    ];
    let path = candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())?;
    let data = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&data).ok()?;
    json.get("providers")?
        .as_object()?
        .values()
        .find_map(|provider| {
            provider
                .get("models")?
                .as_array()?
                .first()?
                .get("id")?
                .as_str()
                .map(String::from)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_interactive() {
        let cmd = Provider::Claude.build_command(&ProviderOptions::default(), false, None);
        assert_eq!(cmd, vec!["claude"]);
        assert!(!cmd.contains(&"-p".to_string()));
    }

    #[test]
    fn claude_with_options() {
        let opts = ProviderOptions {
            model: Some("opus".into()),
            max_turns: Some(50),
            allowed_tools: Some(vec!["Read".into(), "Edit".into()]),
            ..Default::default()
        };
        let cmd = Provider::Claude.build_command(&opts, false, None);
        assert_eq!(
            cmd,
            vec!["claude", "--model", "opus", "--max-turns", "50", "--allowedTools", "Read,Edit"]
        );
    }

    #[test]
    fn pi_interactive() {
        let cmd = Provider::Pi.build_command(&ProviderOptions::default(), false, None);
        assert!(cmd.contains(&"--approve".to_string()));
        assert!(!cmd.contains(&"-p".to_string()));
        assert!(!cmd.contains(&"--no-extensions".to_string()));
    }

    #[test]
    fn pi_safe_mode() {
        let cmd = Provider::Pi.build_command(&ProviderOptions::default(), true, None);
        assert!(cmd.contains(&"--no-approve".to_string()));
        assert!(!cmd.contains(&"--approve".to_string()));
    }

    #[test]
    fn codex_bypass() {
        let cmd = Provider::Codex.build_command(&ProviderOptions::default(), false, None);
        assert!(cmd.contains(&"exec".to_string()));
        assert!(cmd.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
    }

    #[test]
    fn codex_safe_mode() {
        let cmd = Provider::Codex.build_command(&ProviderOptions::default(), true, None);
        assert!(cmd.contains(&"exec".to_string()));
        assert!(!cmd.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
    }

    #[test]
    fn gemini_yolo() {
        let cmd = Provider::Gemini.build_command(&ProviderOptions::default(), false, None);
        assert!(cmd.contains(&"--yolo".to_string()));
    }

    #[test]
    fn gemini_safe_mode() {
        let cmd = Provider::Gemini.build_command(&ProviderOptions::default(), true, None);
        assert!(!cmd.contains(&"--yolo".to_string()));
    }

    #[test]
    fn custom_passthrough() {
        let custom = vec!["node".into(), "my-agent.js".into(), "--flag".into()];
        let cmd = Provider::Custom.build_command(&ProviderOptions::default(), false, Some(&custom));
        assert_eq!(cmd, custom);
    }

    #[test]
    fn claude_headless() {
        let cmd = Provider::Claude
            .build_headless_command("fix the bug", &ProviderOptions::default(), false, None)
            .unwrap();
        assert_eq!(cmd[0], "claude");
        assert!(cmd.contains(&"-p".to_string()));
        assert!(cmd.contains(&"fix the bug".to_string()));
        assert!(cmd.contains(&"--output-format".to_string()));
        assert!(cmd.contains(&"json".to_string()));
    }

    #[test]
    fn claude_headless_with_options() {
        let opts = ProviderOptions {
            model: Some("opus".into()),
            max_turns: Some(10),
            ..Default::default()
        };
        let cmd = Provider::Claude
            .build_headless_command("do stuff", &opts, false, None)
            .unwrap();
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"opus".to_string()));
        assert!(cmd.contains(&"--max-turns".to_string()));
        assert!(cmd.contains(&"-p".to_string()));
    }

    #[test]
    fn codex_headless() {
        let cmd = Provider::Codex
            .build_headless_command("fix it", &ProviderOptions::default(), false, None)
            .unwrap();
        assert!(cmd.contains(&"exec".to_string()));
        assert!(cmd.contains(&"fix it".to_string()));
    }

    #[test]
    fn opencode_headless() {
        let cmd = Provider::Opencode
            .build_headless_command("analyze", &ProviderOptions::default(), false, None)
            .unwrap();
        assert_eq!(cmd[0], "opencode");
        assert_eq!(cmd[1], "run");
        assert!(cmd.contains(&"analyze".to_string()));
        assert!(cmd.contains(&"--format".to_string()));
        assert!(cmd.contains(&"json".to_string()));
    }

    #[test]
    fn opencode_headless_with_model() {
        let opts = ProviderOptions {
            model: Some("provider/model".into()),
            ..Default::default()
        };
        let cmd = Provider::Opencode
            .build_headless_command("check", &opts, false, None)
            .unwrap();
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"provider/model".to_string()));
    }

    #[test]
    fn custom_headless() {
        let custom = vec!["my-tool".into(), "--run".into()];
        let cmd = Provider::Custom
            .build_headless_command("unused", &ProviderOptions::default(), false, Some(&custom))
            .unwrap();
        assert_eq!(cmd, custom);
    }

    #[test]
    fn pi_headless_unsupported() {
        assert!(Provider::Pi
            .build_headless_command("test", &ProviderOptions::default(), false, None)
            .is_none());
    }

    #[test]
    fn gemini_headless_unsupported() {
        assert!(Provider::Gemini
            .build_headless_command("test", &ProviderOptions::default(), false, None)
            .is_none());
    }
}
