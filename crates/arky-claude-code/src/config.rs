//! Typed Claude Code provider configuration and CLI argument building.

use std::{
    collections::BTreeMap,
    fmt,
    path::{
        Path,
        PathBuf,
    },
    sync::Arc,
};

use arky_provider::ProviderError;
use serde_json::Value;

use crate::SpawnFailurePolicy;

/// Known shorthand Claude model identifiers.
pub const KNOWN_CLAUDE_MODEL_IDS: [&str; 3] = ["opus", "sonnet", "haiku"];

/// Prompt length threshold after which a warning should be surfaced.
pub const MAX_PROMPT_WARNING_LENGTH: usize = 100_000;

/// Shared callback used to surface Claude stderr to callers.
#[derive(Clone)]
pub struct ClaudeStderrCallback(Arc<dyn Fn(&str) + Send + Sync>);

impl ClaudeStderrCallback {
    /// Creates a callback wrapper from any `Fn(&str)` closure.
    #[must_use]
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        Self(Arc::new(callback))
    }

    /// Invokes the wrapped callback.
    pub fn call(&self, stderr: &str) {
        (self.0)(stderr);
    }
}

impl fmt::Debug for ClaudeStderrCallback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClaudeStderrCallback(..)")
    }
}

impl PartialEq for ClaudeStderrCallback {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ClaudeStderrCallback {}

/// Typed local plugin configuration for Claude CLI invocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudePluginConfig {
    /// Local plugin file path.
    pub path: PathBuf,
}

impl ClaudePluginConfig {
    /// Creates a local plugin descriptor.
    #[must_use]
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Converts the plugin into a CLI JSON payload with a resolved local path.
    pub fn to_value(&self, cwd: Option<&Path>) -> Value {
        serde_json::json!({
            "type": "local",
            "path": resolve_plugin_path(self.path.as_path(), cwd),
        })
    }
}

/// Typed sandbox passthrough for Claude CLI JSON flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeSandboxConfig {
    raw: Value,
}

impl ClaudeSandboxConfig {
    /// Creates a sandbox wrapper from an arbitrary JSON payload.
    #[must_use]
    pub const fn new(raw: Value) -> Self {
        Self { raw }
    }

    /// Creates a simple `{ mode }` sandbox payload.
    #[must_use]
    pub fn with_mode(mode: impl Into<String>) -> Self {
        Self::new(serde_json::json!({
            "mode": mode.into(),
        }))
    }

    /// Returns the raw JSON value.
    #[must_use]
    pub const fn as_value(&self) -> &Value {
        &self.raw
    }
}

impl From<ClaudeSandboxConfig> for Value {
    fn from(config: ClaudeSandboxConfig) -> Self {
        config.raw
    }
}

/// CLI-behavior options for Claude invocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCliBehaviorConfig {
    /// Whether `--verbose` should be added to Claude invocations.
    pub verbose: bool,
    /// Whether partial messages should be included.
    pub include_partial_messages: bool,
    /// Whether debug mode is enabled.
    pub debug: bool,
    /// Optional debug file path.
    pub debug_file: Option<PathBuf>,
    /// Optional stderr callback.
    pub stderr_callback: Option<ClaudeStderrCallback>,
    /// Streaming input preference.
    pub streaming_input: Option<String>,
    /// Optional fallback model.
    pub fallback_model: Option<String>,
}

impl Default for ClaudeCliBehaviorConfig {
    fn default() -> Self {
        Self {
            verbose: true,
            include_partial_messages: false,
            debug: false,
            debug_file: None,
            stderr_callback: None,
            streaming_input: Some("auto".to_owned()),
            fallback_model: None,
        }
    }
}

/// Permission-related Claude runtime options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClaudePermissionConfig {
    /// Optional permission mode.
    pub mode: Option<String>,
    /// Whether dangerous permission skipping is enabled.
    pub allow_dangerously_skip_permissions: bool,
    /// Optional permission prompt tool name.
    pub prompt_tool_name: Option<String>,
}

/// Session-related Claude runtime options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClaudeSessionConfig {
    /// Whether the conversation should continue.
    pub continue_conversation: bool,
    /// Optional resume identifier.
    pub resume: Option<String>,
    /// Optional fixed session identifier.
    pub session_id: Option<String>,
    /// Optional point-in-time resume marker.
    pub resume_session_at: Option<String>,
    /// Whether sessions should persist.
    pub persist_session: bool,
    /// Whether sessions may be forked.
    pub fork_session: bool,
}

/// Filesystem-related Claude runtime options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClaudeFilesystemConfig {
    /// Additional directories exposed to Claude.
    pub additional_directories: Vec<PathBuf>,
    /// Whether file checkpointing is enabled.
    pub enable_file_checkpointing: bool,
}

/// Runtime configuration for the Claude Code provider.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCodeProviderConfig {
    /// Binary name or path used for Claude invocations.
    pub binary: String,
    /// Optional working directory for the Claude subprocess.
    pub cwd: Option<PathBuf>,
    /// Extra CLI arguments added before request-specific flags.
    pub extra_args: Vec<String>,
    /// Environment overrides applied to Claude subprocesses.
    pub env: BTreeMap<String, String>,
    /// Arguments used to query the binary version.
    pub version_args: Vec<String>,
    /// CLI-behavior options.
    pub cli_behavior: ClaudeCliBehaviorConfig,
    /// Maximum line length accepted from Claude stdout.
    pub max_frame_len: usize,
    /// Spawn-failure cooldown policy.
    pub spawn_failure_policy: SpawnFailurePolicy,
    /// Optional system prompt override.
    pub system_prompt: Option<String>,
    /// Deprecated system prompt override kept for parity.
    pub custom_system_prompt: Option<String>,
    /// Deprecated appended prompt override kept for parity.
    pub append_system_prompt: Option<String>,
    /// Optional maximum number of Claude turns.
    pub max_turns: Option<u32>,
    /// Optional hard cap on thinking tokens.
    pub max_thinking_tokens: Option<u32>,
    /// Optional reasoning effort.
    pub reasoning_effort: Option<String>,
    /// Optional parent executable wrapper.
    pub executable: Option<String>,
    /// Optional extra args for the parent executable.
    pub executable_args: Vec<String>,
    /// Permission-related options.
    pub permission: ClaudePermissionConfig,
    /// Session-related options.
    pub session: ClaudeSessionConfig,
    /// Explicitly allowed tools.
    pub allowed_tools: Vec<String>,
    /// Explicitly disallowed tools.
    pub disallowed_tools: Vec<String>,
    /// Selected setting sources.
    pub setting_sources: Vec<String>,
    /// Enabled beta flags.
    pub betas: Vec<String>,
    /// Raw hooks configuration.
    pub hooks: Option<Value>,
    /// Raw hook options.
    pub hook_options: Option<Value>,
    /// Raw MCP server definitions.
    pub mcp_servers: BTreeMap<String, Value>,
    /// Filesystem-related options.
    pub filesystem: ClaudeFilesystemConfig,
    /// Optional budget cap.
    pub max_budget_usd: Option<f64>,
    /// Plugin descriptors.
    pub plugins: Vec<Value>,
    /// Sandbox configuration.
    pub sandbox: Option<Value>,
    /// Agent definitions.
    pub agents: Option<Value>,
}

impl ClaudeCodeProviderConfig {
    /// Builds CLI arguments for one Claude request.
    pub fn cli_args(
        &self,
        prompt: String,
        model: String,
        runtime_session_id: Option<&str>,
    ) -> Result<Vec<String>, ProviderError> {
        let mut args = self.extra_args.clone();
        self.push_base_args(&mut args, prompt, model);
        self.push_limit_args(&mut args);
        self.push_permission_args(&mut args);
        self.push_session_args(&mut args, runtime_session_id);
        self.push_prompt_args(&mut args);
        self.push_tool_and_fs_args(&mut args);
        self.push_behavior_args(&mut args);
        self.push_json_args(&mut args)?;
        Ok(args)
    }

    fn push_base_args(&self, args: &mut Vec<String>, prompt: String, model: String) {
        if self.cli_behavior.verbose || !args.iter().any(|arg| arg == "--verbose") {
            args.push("--verbose".to_owned());
        }
        args.push("--print".to_owned());
        args.push(prompt);
        args.push("--output-format".to_owned());
        args.push("stream-json".to_owned());
        args.push("--model".to_owned());
        args.push(self.cli_behavior.fallback_model.clone().unwrap_or(model));
    }

    fn push_limit_args(&self, args: &mut Vec<String>) {
        push_optional_arg(
            args,
            "--max-turns",
            self.max_turns.map(|value| value.to_string()),
        );
        push_optional_arg(
            args,
            "--max-thinking-tokens",
            self.max_thinking_tokens
                .or_else(|| thinking_budget(self.reasoning_effort.as_deref()))
                .map(|value| value.to_string()),
        );
        push_optional_arg(args, "--reasoning-effort", self.reasoning_effort.clone());
        push_optional_arg(
            args,
            "--max-budget-usd",
            self.max_budget_usd.map(|value| value.to_string()),
        );
    }

    fn push_permission_args(&self, args: &mut Vec<String>) {
        push_optional_arg(
            args,
            "--permission-mode",
            self.permission.mode.as_deref().map(remap_permission_mode),
        );
        if self.permission.allow_dangerously_skip_permissions {
            args.push("--allow-dangerously-skip-permissions".to_owned());
        }
        push_optional_arg(
            args,
            "--permission-prompt-tool-name",
            self.permission.prompt_tool_name.clone(),
        );
    }

    fn push_session_args(
        &self,
        args: &mut Vec<String>,
        runtime_session_id: Option<&str>,
    ) {
        if self.session.continue_conversation {
            args.push("--continue".to_owned());
        }
        push_optional_arg(
            args,
            "--resume",
            self.session
                .resume
                .clone()
                .or_else(|| runtime_session_id.map(ToOwned::to_owned)),
        );
        push_optional_arg(
            args,
            "--session-id",
            self.session
                .session_id
                .clone()
                .or_else(|| runtime_session_id.map(ToOwned::to_owned)),
        );
        push_optional_arg(
            args,
            "--resume-session-at",
            self.session.resume_session_at.clone(),
        );
        if self.session.persist_session {
            args.push("--persist-session".to_owned());
        }
        if self.session.fork_session {
            args.push("--fork-session".to_owned());
        }
    }

    fn push_prompt_args(&self, args: &mut Vec<String>) {
        push_optional_arg(args, "--system-prompt", effective_system_prompt(self));
    }

    fn push_tool_and_fs_args(&self, args: &mut Vec<String>) {
        for tool in &self.allowed_tools {
            push_flag_value(args, "--allowed-tool", tool.clone());
        }
        for tool in &self.disallowed_tools {
            push_flag_value(args, "--disallowed-tool", tool.clone());
        }
        for source in &self.setting_sources {
            push_flag_value(args, "--setting-source", source.clone());
        }
        for beta in &self.betas {
            push_flag_value(args, "--beta", beta.clone());
        }
        for directory in &self.filesystem.additional_directories {
            push_flag_value(args, "--add-dir", directory.display().to_string());
        }
        if self.filesystem.enable_file_checkpointing {
            args.push("--enable-file-checkpointing".to_owned());
        }
    }

    fn push_behavior_args(&self, args: &mut Vec<String>) {
        if self.cli_behavior.include_partial_messages {
            args.push("--include-partial-messages".to_owned());
        }
        if self.cli_behavior.debug {
            args.push("--debug".to_owned());
        }
        push_optional_arg(
            args,
            "--streaming-input",
            self.cli_behavior.streaming_input.clone(),
        );
        push_optional_arg(
            args,
            "--debug-file",
            self.cli_behavior
                .debug_file
                .as_ref()
                .map(|path| path.display().to_string()),
        );
    }

    fn push_json_args(&self, args: &mut Vec<String>) -> Result<(), ProviderError> {
        if let Some(hooks) = &self.hooks {
            push_json_flag(args, "--hooks", hooks)?;
        }
        if let Some(hook_options) = &self.hook_options {
            push_json_flag(args, "--hook-options", hook_options)?;
        }
        if !self.mcp_servers.is_empty() {
            push_json_flag(
                args,
                "--mcp-server",
                &Value::Object(self.mcp_servers.clone().into_iter().collect()),
            )?;
        }
        if let Some(sandbox) = &self.sandbox {
            push_json_flag(args, "--sandbox", sandbox)?;
        }
        if let Some(agents) = &self.agents {
            push_json_flag(args, "--agents", agents)?;
        }
        for plugin in &self.plugins {
            push_json_flag(
                args,
                "--plugin",
                &resolve_plugin_value(plugin, self.cwd.as_deref()),
            )?;
        }
        Ok(())
    }
}

impl Default for ClaudeCodeProviderConfig {
    fn default() -> Self {
        Self {
            binary: "claude".to_owned(),
            cwd: None,
            extra_args: Vec::new(),
            env: BTreeMap::new(),
            version_args: vec!["--version".to_owned()],
            cli_behavior: ClaudeCliBehaviorConfig::default(),
            max_frame_len: 256 * 1024,
            spawn_failure_policy: SpawnFailurePolicy::default(),
            system_prompt: None,
            custom_system_prompt: None,
            append_system_prompt: None,
            max_turns: None,
            max_thinking_tokens: None,
            reasoning_effort: None,
            executable: None,
            executable_args: Vec::new(),
            permission: ClaudePermissionConfig::default(),
            session: ClaudeSessionConfig::default(),
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            setting_sources: Vec::new(),
            betas: Vec::new(),
            hooks: None,
            hook_options: None,
            mcp_servers: BTreeMap::new(),
            filesystem: ClaudeFilesystemConfig::default(),
            max_budget_usd: None,
            plugins: Vec::new(),
            sandbox: None,
            agents: None,
        }
    }
}

fn push_optional_arg(args: &mut Vec<String>, flag: &str, value: Option<String>) {
    if let Some(value) = value {
        push_flag_value(args, flag, value);
    }
}

fn push_flag_value(args: &mut Vec<String>, flag: &str, value: String) {
    args.push(flag.to_owned());
    args.push(value);
}

fn push_json_flag(
    args: &mut Vec<String>,
    flag: &str,
    value: &Value,
) -> Result<(), ProviderError> {
    let encoded = serde_json::to_string(value).map_err(|error| {
        ProviderError::protocol_violation(
            format!("failed to serialize `{flag}` argument"),
            Some(serde_json::json!({
                "reason": error.to_string(),
            })),
        )
    })?;
    push_flag_value(args, flag, encoded);
    Ok(())
}

fn resolve_plugin_value(plugin: &Value, cwd: Option<&Path>) -> Value {
    match plugin {
        Value::String(path) => ClaudePluginConfig::local(path).to_value(cwd),
        Value::Object(record) => {
            let mut resolved = record.clone();
            if resolved.get("type").and_then(Value::as_str) == Some("local") {
                for key in ["path", "localPath", "file"] {
                    let Some(Value::String(path)) = resolved.get(key) else {
                        continue;
                    };
                    resolved.insert(
                        key.to_owned(),
                        Value::String(resolve_plugin_path(path, cwd)),
                    );
                }
            }
            Value::Object(resolved)
        }
        other => other.clone(),
    }
}

fn resolve_plugin_path(path: impl AsRef<Path>, cwd: Option<&Path>) -> String {
    let candidate = path.as_ref();
    if candidate.is_absolute() {
        return candidate.to_string_lossy().into_owned();
    }

    cwd.unwrap_or_else(|| Path::new("."))
        .join(candidate)
        .to_string_lossy()
        .into_owned()
}

/// Validates a Claude model identifier and returns a warning when it looks unusual.
#[must_use]
pub fn validate_claude_model_id(model_id: &str) -> Option<String> {
    let normalized = model_id.trim();
    if normalized.is_empty() {
        return Some("Model ID cannot be empty".to_owned());
    }

    if KNOWN_CLAUDE_MODEL_IDS.contains(&normalized) {
        return None;
    }

    Some(format!(
        "Unknown model ID: '{normalized}'. Proceeding with a custom Claude model."
    ))
}

/// Validates prompt length against the Claude warning threshold.
#[must_use]
pub fn validate_prompt_length(prompt: &str) -> Option<String> {
    (prompt.len() > MAX_PROMPT_WARNING_LENGTH).then(|| {
        format!(
            "Very long prompt detected ({} characters). Claude Code performance may degrade.",
            prompt.len()
        )
    })
}

/// Validates Claude session identifier formatting.
#[must_use]
pub fn validate_session_id_format(session_id: &str) -> Option<String> {
    let is_valid = session_id.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
    });
    (!session_id.is_empty() && !is_valid).then(|| {
        "Unusual session ID format. This may cause issues with session resumption."
            .to_owned()
    })
}

fn thinking_budget(reasoning_effort: Option<&str>) -> Option<u32> {
    match reasoning_effort {
        Some("low") => Some(15_999),
        Some("medium") => Some(31_999),
        Some("high") => Some(63_999),
        _ => None,
    }
}

fn remap_permission_mode(permission_mode: &str) -> String {
    if permission_mode == "delegate" {
        return "dontAsk".to_owned();
    }

    permission_mode.to_owned()
}

fn effective_system_prompt(config: &ClaudeCodeProviderConfig) -> Option<String> {
    config
        .system_prompt
        .clone()
        .or_else(|| config.custom_system_prompt.clone())
        .or_else(|| {
            config.append_system_prompt.as_ref().map(|append| {
                format!("{{\"type\":\"preset\",\"preset\":\"claude_code\",\"append\":{append:?}}}")
            })
        })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        path::{
            Path,
            PathBuf,
        },
    };

    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::{
        ClaudeCliBehaviorConfig,
        ClaudeCodeProviderConfig,
        ClaudePermissionConfig,
        ClaudePluginConfig,
        ClaudeSandboxConfig,
        MAX_PROMPT_WARNING_LENGTH,
        validate_claude_model_id,
        validate_prompt_length,
        validate_session_id_format,
    };

    #[test]
    fn config_should_serialize_key_runtime_fields_to_cli_args() {
        let config = ClaudeCodeProviderConfig {
            max_turns: Some(8),
            reasoning_effort: Some("high".to_owned()),
            append_system_prompt: Some("extra".to_owned()),
            permission: ClaudePermissionConfig {
                mode: Some("delegate".to_owned()),
                ..ClaudePermissionConfig::default()
            },
            mcp_servers: BTreeMap::from_iter([(
                "runtime".to_owned(),
                json!({ "type": "http", "url": "http://127.0.0.1:7777/mcp" }),
            )]),
            hooks: Some(json!({ "before": ["echo hi"] })),
            agents: Some(json!({ "researcher": { "prompt": "Investigate" } })),
            sandbox: Some(json!({ "mode": "workspace-write" })),
            plugins: vec![json!({ "name": "plugin-a" })],
            ..ClaudeCodeProviderConfig::default()
        };

        let args = config
            .cli_args("hello".to_owned(), "sonnet".to_owned(), Some("session-123"))
            .expect("args should build");

        assert!(args.windows(2).any(|window| {
            window == ["--session-id".to_owned(), "session-123".to_owned()]
        }));
        assert!(
            args.windows(2)
                .any(|window| { window == ["--max-turns".to_owned(), "8".to_owned()] })
        );
        assert!(args.windows(2).any(|window| {
            window == ["--max-thinking-tokens".to_owned(), "63999".to_owned()]
        }));
        assert!(args.windows(2).any(|window| {
            window == ["--permission-mode".to_owned(), "dontAsk".to_owned()]
        }));
        assert!(args.windows(2).any(|window| {
            window[0] == "--mcp-server" && window[1].contains("runtime")
        }));
        assert!(args.windows(2).any(|window| {
            window[0] == "--agents" && window[1].contains("researcher")
        }));
    }

    #[test]
    fn explicit_max_thinking_tokens_should_override_reasoning_effort_mapping() {
        let config = ClaudeCodeProviderConfig {
            reasoning_effort: Some("high".to_owned()),
            max_thinking_tokens: Some(999),
            ..ClaudeCodeProviderConfig::default()
        };

        let args = config
            .cli_args("hello".to_owned(), "sonnet".to_owned(), None)
            .expect("args should build");

        assert!(args.windows(2).any(|window| {
            window == ["--max-thinking-tokens".to_owned(), "999".to_owned()]
        }));
    }

    #[test]
    fn fallback_model_should_override_requested_model() {
        let config = ClaudeCodeProviderConfig {
            cli_behavior: ClaudeCliBehaviorConfig {
                fallback_model: Some("claude-fallback".to_owned()),
                ..ClaudeCliBehaviorConfig::default()
            },
            ..ClaudeCodeProviderConfig::default()
        };

        let args = config
            .cli_args("hello".to_owned(), "sonnet".to_owned(), None)
            .expect("args should build");

        assert!(args.windows(2).any(|window| {
            window == ["--model".to_owned(), "claude-fallback".to_owned()]
        }));
    }

    #[test]
    fn plugin_flags_should_resolve_relative_paths_against_cwd() {
        let config = ClaudeCodeProviderConfig {
            cwd: Some(PathBuf::from("/workspace")),
            plugins: vec![
                ClaudePluginConfig::local("./plugins/researcher.js")
                    .to_value(Some(Path::new("/workspace"))),
            ],
            ..ClaudeCodeProviderConfig::default()
        };

        let args = config
            .cli_args("hello".to_owned(), "sonnet".to_owned(), None)
            .expect("args should build");

        assert!(args.windows(2).any(|window| {
            window[0] == "--plugin"
                && window[1].contains("/workspace/./plugins/researcher.js")
                && window[1].contains("\"type\":\"local\"")
        }));
    }

    #[test]
    fn sandbox_and_debug_flags_should_serialize_to_cli_args() {
        let config = ClaudeCodeProviderConfig {
            cli_behavior: ClaudeCliBehaviorConfig {
                debug: true,
                debug_file: Some(PathBuf::from("claude.debug.log")),
                ..ClaudeCliBehaviorConfig::default()
            },
            sandbox: Some(ClaudeSandboxConfig::with_mode("workspace-write").into()),
            ..ClaudeCodeProviderConfig::default()
        };

        let args = config
            .cli_args("hello".to_owned(), "sonnet".to_owned(), None)
            .expect("args should build");

        assert!(args.contains(&"--debug".to_owned()));
        assert!(args.windows(2).any(|window| {
            window[0] == "--debug-file" && window[1] == "claude.debug.log"
        }));
        assert!(args.windows(2).any(|window| {
            window[0] == "--sandbox" && window[1].contains("workspace-write")
        }));
    }

    #[test]
    fn validators_should_cover_model_prompt_and_session_warnings() {
        assert_eq!(
            validate_claude_model_id("custom-model"),
            Some(
                "Unknown model ID: 'custom-model'. Proceeding with a custom Claude model."
                    .to_owned()
            )
        );
        assert!(
            validate_prompt_length(&"x".repeat(MAX_PROMPT_WARNING_LENGTH + 1)).is_some()
        );
        assert_eq!(
            validate_session_id_format("session/invalid"),
            Some(
                "Unusual session ID format. This may cause issues with session resumption."
                    .to_owned()
            )
        );
    }
}
