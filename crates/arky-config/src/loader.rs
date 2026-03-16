//! Configuration types, builders, and loading entry points.

use std::{
    collections::BTreeMap,
    fs,
    path::{
        Path,
        PathBuf,
    },
};

use serde::{
    Deserialize,
    Serialize,
};
use serde_json::{
    Map,
    Value,
};

use crate::{
    error::ConfigError,
    merge::{
        merge_agent,
        merge_config,
        merge_provider,
        merge_workspace,
    },
    validate::{
        check_provider_prerequisites,
        validate_config,
    },
};

const DEFAULT_ENV_PREFIX: &str = "ARKY";

/// Supported on-disk configuration formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// TOML configuration.
    Toml,
    /// YAML configuration.
    Yaml,
}

impl ConfigFormat {
    /// Infers a config format from a file extension.
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Toml => "toml",
            Self::Yaml => "yaml",
        }
    }
}

/// Fully merged and validated Arky configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArkyConfig {
    workspace: WorkspaceConfig,
    providers: BTreeMap<String, ProviderConfig>,
    agents: BTreeMap<String, AgentConfig>,
}

impl ArkyConfig {
    pub(super) const fn new(
        workspace: WorkspaceConfig,
        providers: BTreeMap<String, ProviderConfig>,
        agents: BTreeMap<String, AgentConfig>,
    ) -> Self {
        Self {
            workspace,
            providers,
            agents,
        }
    }

    /// Creates a programmatic builder for Arky configuration.
    #[must_use]
    pub fn builder() -> ArkyConfigBuilder {
        ArkyConfigBuilder::new()
    }

    /// Loads configuration from a file plus current `ARKY_*` environment overrides.
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        ConfigLoader::from_path(path).load()
    }

    /// Returns workspace-level settings.
    #[must_use]
    pub const fn workspace(&self) -> &WorkspaceConfig {
        &self.workspace
    }

    /// Returns all configured providers.
    #[must_use]
    pub const fn providers(&self) -> &BTreeMap<String, ProviderConfig> {
        &self.providers
    }

    /// Returns one provider by name.
    #[must_use]
    pub fn provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    /// Returns all configured agents.
    #[must_use]
    pub const fn agents(&self) -> &BTreeMap<String, AgentConfig> {
        &self.agents
    }

    /// Returns one agent by name.
    #[must_use]
    pub fn agent(&self, name: &str) -> Option<&AgentConfig> {
        self.agents.get(name)
    }

    /// Verifies that every configured provider prerequisite binary exists.
    pub fn check_prerequisites(&self) -> Result<BTreeMap<String, PathBuf>, ConfigError> {
        check_provider_prerequisites(self)
    }
}

/// Workspace-scoped defaults shared across providers and agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceConfig {
    name: Option<String>,
    default_provider: Option<String>,
    data_dir: Option<PathBuf>,
    env: BTreeMap<String, String>,
}

impl WorkspaceConfig {
    pub(super) const fn new(
        name: Option<String>,
        default_provider: Option<String>,
        data_dir: Option<PathBuf>,
        env: BTreeMap<String, String>,
    ) -> Self {
        Self {
            name,
            default_provider,
            data_dir,
            env,
        }
    }

    /// Creates a builder for workspace-level settings.
    #[must_use]
    pub fn builder() -> WorkspaceConfigBuilder {
        WorkspaceConfigBuilder::new()
    }

    /// Returns the workspace name.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the default provider name, if configured.
    #[must_use]
    pub fn default_provider(&self) -> Option<&str> {
        self.default_provider.as_deref()
    }

    /// Returns the configured data directory.
    #[must_use]
    pub fn data_dir(&self) -> Option<&Path> {
        self.data_dir.as_deref()
    }

    /// Returns injected workspace environment variables.
    #[must_use]
    pub const fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }
}

/// Provider-level runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderConfig {
    kind: String,
    binary: Option<PathBuf>,
    model: Option<String>,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

impl ProviderConfig {
    pub(super) const fn new(
        kind: String,
        binary: Option<PathBuf>,
        model: Option<String>,
        args: Vec<String>,
        env: BTreeMap<String, String>,
    ) -> Self {
        Self {
            kind,
            binary,
            model,
            args,
            env,
        }
    }

    /// Creates a provider builder.
    #[must_use]
    pub fn builder() -> ProviderConfigBuilder {
        ProviderConfigBuilder::new()
    }

    /// Returns the provider family or adapter kind.
    #[must_use]
    pub const fn kind(&self) -> &str {
        self.kind.as_str()
    }

    /// Returns an explicit binary override, if configured.
    #[must_use]
    pub fn binary(&self) -> Option<&Path> {
        self.binary.as_deref()
    }

    /// Returns the provider model override.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Returns provider command-line arguments.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Returns provider-specific environment variables.
    #[must_use]
    pub const fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    pub(super) fn prerequisite_binary(&self) -> String {
        let Some(binary) = self.binary.as_ref() else {
            return default_binary_for_kind(self.kind.as_str());
        };

        binary.to_string_lossy().into_owned()
    }
}

/// Agent-level defaults used by the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentConfig {
    provider: String,
    model: Option<String>,
    instructions: Option<String>,
    max_turns: Option<u16>,
    tools: Vec<String>,
    env: BTreeMap<String, String>,
}

impl AgentConfig {
    pub(super) const fn new(
        provider: String,
        model: Option<String>,
        instructions: Option<String>,
        max_turns: Option<u16>,
        tools: Vec<String>,
        env: BTreeMap<String, String>,
    ) -> Self {
        Self {
            provider,
            model,
            instructions,
            max_turns,
            tools,
            env,
        }
    }

    /// Creates an agent builder.
    #[must_use]
    pub fn builder() -> AgentConfigBuilder {
        AgentConfigBuilder::new()
    }

    /// Returns the provider entry used by this agent.
    #[must_use]
    pub const fn provider(&self) -> &str {
        self.provider.as_str()
    }

    /// Returns the per-agent model override.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Returns per-agent instructions.
    #[must_use]
    pub fn instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// Returns the optional maximum turn budget.
    #[must_use]
    pub const fn max_turns(&self) -> Option<u16> {
        self.max_turns
    }

    /// Returns the tool allow-list for the agent.
    #[must_use]
    pub fn tools(&self) -> &[String] {
        &self.tools
    }

    /// Returns agent-specific environment variables.
    #[must_use]
    pub const fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }
}

/// Builder for programmatic `ArkyConfig` creation.
#[derive(Debug, Clone, Default)]
pub struct ArkyConfigBuilder {
    partial: PartialArkyConfig,
}

impl ArkyConfigBuilder {
    /// Creates an empty config builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Merges workspace-level settings into the builder.
    #[must_use]
    pub fn workspace(mut self, workspace: WorkspaceConfigBuilder) -> Self {
        self.partial.workspace =
            merge_workspace(self.partial.workspace, workspace.into_partial());
        self
    }

    /// Merges or creates a provider entry.
    #[must_use]
    pub fn provider(
        mut self,
        name: impl Into<String>,
        provider: ProviderConfigBuilder,
    ) -> Self {
        let name = name.into();
        let merged = match self.partial.providers.remove(&name) {
            Some(existing) => merge_provider(existing, provider.into_partial()),
            None => provider.into_partial(),
        };
        self.partial.providers.insert(name, merged);
        self
    }

    /// Merges or creates an agent entry.
    #[must_use]
    pub fn agent(mut self, name: impl Into<String>, agent: AgentConfigBuilder) -> Self {
        let name = name.into();
        let merged = match self.partial.agents.remove(&name) {
            Some(existing) => merge_agent(existing, agent.into_partial()),
            None => agent.into_partial(),
        };
        self.partial.agents.insert(name, merged);
        self
    }

    pub(crate) fn into_partial(self) -> PartialArkyConfig {
        self.partial
    }

    /// Builds the final merged and validated config.
    pub fn build(self) -> Result<ArkyConfig, ConfigError> {
        validate_config(self.partial)
    }
}

/// Builder for `WorkspaceConfig`.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceConfigBuilder {
    partial: PartialWorkspaceConfig,
}

impl WorkspaceConfigBuilder {
    /// Creates an empty workspace builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the workspace name.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.partial.name = Some(name.into());
        self
    }

    /// Sets the default provider name.
    #[must_use]
    pub fn default_provider(mut self, default_provider: impl Into<String>) -> Self {
        self.partial.default_provider = Some(default_provider.into());
        self
    }

    /// Sets the workspace data directory.
    #[must_use]
    pub fn data_dir(mut self, data_dir: impl Into<PathBuf>) -> Self {
        self.partial.data_dir = Some(data_dir.into());
        self
    }

    /// Adds or replaces one workspace environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let env = self.partial.env.get_or_insert_with(BTreeMap::new);
        env.insert(key.into(), value.into());
        self
    }

    fn into_partial(self) -> PartialWorkspaceConfig {
        self.partial
    }
}

/// Builder for `ProviderConfig`.
#[derive(Debug, Clone, Default)]
pub struct ProviderConfigBuilder {
    partial: PartialProviderConfig,
}

impl ProviderConfigBuilder {
    /// Creates an empty provider builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the provider kind.
    #[must_use]
    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.partial.kind = Some(kind.into());
        self
    }

    /// Sets an explicit provider binary path or command name.
    #[must_use]
    pub fn binary(mut self, binary: impl Into<PathBuf>) -> Self {
        self.partial.binary = Some(binary.into());
        self
    }

    /// Sets a provider model override.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.partial.model = Some(model.into());
        self
    }

    /// Replaces provider arguments.
    #[must_use]
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.partial.args = Some(args.into_iter().map(Into::into).collect());
        self
    }

    /// Adds or replaces one provider environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let env = self.partial.env.get_or_insert_with(BTreeMap::new);
        env.insert(key.into(), value.into());
        self
    }

    fn into_partial(self) -> PartialProviderConfig {
        self.partial
    }
}

/// Builder for `AgentConfig`.
#[derive(Debug, Clone, Default)]
pub struct AgentConfigBuilder {
    partial: PartialAgentConfig,
}

impl AgentConfigBuilder {
    /// Creates an empty agent builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the provider entry name for the agent.
    #[must_use]
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.partial.provider = Some(provider.into());
        self
    }

    /// Sets the model override for the agent.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.partial.model = Some(model.into());
        self
    }

    /// Sets per-agent instructions.
    #[must_use]
    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.partial.instructions = Some(instructions.into());
        self
    }

    /// Sets the maximum turn budget.
    #[must_use]
    pub const fn max_turns(mut self, max_turns: u16) -> Self {
        self.partial.max_turns = Some(max_turns);
        self
    }

    /// Replaces the tool allow-list.
    #[must_use]
    pub fn tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.partial.tools = Some(tools.into_iter().map(Into::into).collect());
        self
    }

    /// Adds or replaces one agent environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let env = self.partial.env.get_or_insert_with(BTreeMap::new);
        env.insert(key.into(), value.into());
        self
    }

    fn into_partial(self) -> PartialAgentConfig {
        self.partial
    }
}

/// Loads config from files, environment variables, and builder overrides.
#[derive(Debug, Clone)]
pub struct ConfigLoader {
    file_path: Option<PathBuf>,
    env_prefix: String,
    env_overrides: Option<Vec<(String, String)>>,
    builder_overrides: PartialArkyConfig,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self {
            file_path: None,
            env_prefix: DEFAULT_ENV_PREFIX.to_owned(),
            env_overrides: None,
            builder_overrides: PartialArkyConfig::default(),
        }
    }
}

impl ConfigLoader {
    /// Creates a loader using the current process environment.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a loader that starts from one config file.
    #[must_use]
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::new().with_file(path)
    }

    /// Sets the source config file.
    #[must_use]
    pub fn with_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Replaces the environment variable prefix used for overrides.
    #[must_use]
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = prefix.into();
        self
    }

    /// Injects explicit environment overrides in `KEY=value` form.
    ///
    /// This is primarily useful for deterministic tests or hosts that want to
    /// avoid reading from the global process environment.
    #[must_use]
    pub fn with_env_overrides<I, K, V>(mut self, overrides: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.env_overrides = Some(
            overrides
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        );
        self
    }

    /// Merges builder overrides after file and environment values.
    #[must_use]
    pub fn with_builder_overrides(mut self, builder: ArkyConfigBuilder) -> Self {
        self.builder_overrides =
            merge_config(self.builder_overrides, builder.into_partial());
        self
    }

    /// Loads, merges, and validates the final config.
    pub fn load(self) -> Result<ArkyConfig, ConfigError> {
        let mut merged = match self.file_path.as_deref() {
            Some(path) => load_file(path)?,
            None => PartialArkyConfig::default(),
        };

        let env_overrides = match self.env_overrides {
            Some(overrides) => load_env_overrides(self.env_prefix.as_str(), overrides)?,
            None => load_env_overrides(self.env_prefix.as_str(), std::env::vars())?,
        };

        merged = merge_config(merged, env_overrides);
        merged = merge_config(merged, self.builder_overrides);

        validate_config(merged)
    }
}

pub fn load_file(path: &Path) -> Result<PartialArkyConfig, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::NotFound {
            path: path.to_path_buf(),
        });
    }

    let format = ConfigFormat::from_path(path).ok_or_else(|| {
        ConfigError::parse(
            format!(
                "unsupported config file format for `{}`",
                path.to_string_lossy()
            ),
            Some(path.to_path_buf()),
            None,
        )
    })?;

    let contents = fs::read_to_string(path).map_err(|error| {
        ConfigError::parse(
            format!("failed to read config file: {error}"),
            Some(path.to_path_buf()),
            Some(format.name()),
        )
    })?;

    match format {
        ConfigFormat::Toml => toml::from_str(&contents).map_err(|error| {
            ConfigError::parse(
                format!("failed to parse TOML config: {error}"),
                Some(path.to_path_buf()),
                Some(format.name()),
            )
        }),
        ConfigFormat::Yaml => serde_norway::from_str(&contents).map_err(|error| {
            ConfigError::parse(
                format!("failed to parse YAML config: {error}"),
                Some(path.to_path_buf()),
                Some(format.name()),
            )
        }),
    }
}

fn load_env_overrides<I>(
    prefix: &str,
    overrides: I,
) -> Result<PartialArkyConfig, ConfigError>
where
    I: IntoIterator<Item = (String, String)>,
{
    let prefix = format!("{prefix}_");
    let mut root = Map::new();

    for (key, value) in overrides {
        let Some(rest) = key.strip_prefix(prefix.as_str()) else {
            continue;
        };

        let path = normalize_env_path(rest);
        if path.is_empty() {
            continue;
        }

        insert_path_value(&mut root, &path, parse_env_value(value.as_str()));
    }

    if root.is_empty() {
        return Ok(PartialArkyConfig::default());
    }

    serde_json::from_value(Value::Object(root)).map_err(|error| {
        ConfigError::parse(
            format!("failed to parse environment overrides: {error}"),
            None,
            Some("env"),
        )
    })
}

fn normalize_env_path(key: &str) -> Vec<String> {
    let raw_segments = key
        .split("__")
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let mut segments = Vec::with_capacity(raw_segments.len());
    let mut index = 0;

    while index < raw_segments.len() {
        if matches!(segments.last(), Some(last) if last == "env") {
            segments.push(raw_segments[index..].join("__"));
            break;
        }

        segments.push(raw_segments[index].to_ascii_lowercase());
        index += 1;
    }

    segments
}

fn insert_path_value(current: &mut Map<String, Value>, path: &[String], value: Value) {
    if path.len() == 1 {
        current.insert(path[0].clone(), value);
        return;
    }

    let key = path[0].clone();
    let entry = current
        .entry(key)
        .or_insert_with(|| Value::Object(Map::new()));
    let next = ensure_object(entry);

    insert_path_value(next, &path[1..], value);
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !matches!(value, Value::Object(_)) {
        *value = Value::Object(Map::new());
    }

    match value {
        Value::Object(map) => map,
        _ => unreachable!("value was normalized to an object"),
    }
}

fn parse_env_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
}

fn default_binary_for_kind(kind: &str) -> String {
    match kind {
        "claude" | "claude-code" | "zai" | "openrouter" | "vercel" | "moonshot"
        | "minimax" | "bedrock" | "vertex" | "ollama" => "claude".to_owned(),
        "codex" => "codex".to_owned(),
        other => other.to_owned(),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialArkyConfig {
    #[serde(default)]
    pub workspace: PartialWorkspaceConfig,
    #[serde(default)]
    pub providers: BTreeMap<String, PartialProviderConfig>,
    #[serde(default)]
    pub agents: BTreeMap<String, PartialAgentConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialWorkspaceConfig {
    pub name: Option<String>,
    pub default_provider: Option<String>,
    pub data_dir: Option<PathBuf>,
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialProviderConfig {
    pub kind: Option<String>,
    pub binary: Option<PathBuf>,
    pub model: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialAgentConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub instructions: Option<String>,
    pub max_turns: Option<u16>,
    pub tools: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{
            Path,
            PathBuf,
        },
    };

    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use super::{
        ArkyConfig,
        ArkyConfigBuilder,
        ConfigLoader,
        ProviderConfigBuilder,
        WorkspaceConfigBuilder,
    };
    use crate::ConfigError;

    #[test]
    fn file_loading_should_parse_valid_toml() {
        let directory = tempdir().expect("temp directory should be created");
        let path = directory.path().join("arky.toml");
        fs::write(
            &path,
            r#"
                [workspace]
                name = "demo"
                default_provider = "default"

                [providers.default]
                kind = "claude-code"
                model = "claude-sonnet-4"

                [agents.writer]
                provider = "default"
                max_turns = 8
            "#,
        )
        .expect("config file should be written");

        let config = ArkyConfig::from_path(&path).expect("config should load");

        let actual = (
            config.workspace().name(),
            config.workspace().default_provider(),
            config.provider("default").and_then(|value| value.model()),
            config
                .agent("writer")
                .and_then(super::AgentConfig::max_turns),
        );

        let expected = (
            Some("demo"),
            Some("default"),
            Some("claude-sonnet-4"),
            Some(8),
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn file_loading_should_return_parse_failed_for_invalid_toml() {
        let directory = tempdir().expect("temp directory should be created");
        let path = directory.path().join("arky.toml");
        fs::write(&path, "not = [valid").expect("config file should be written");

        let error = ArkyConfig::from_path(&path).expect_err("invalid TOML should fail");

        let actual = match error {
            ConfigError::ParseFailed { path, format, .. } => (
                path.map(|value| value.to_string_lossy().into_owned()),
                format,
            ),
            other => panic!("expected parse error, got {other:?}"),
        };

        let expected = (Some(path.to_string_lossy().into_owned()), Some("toml"));

        assert_eq!(actual, expected);
    }

    #[test]
    fn env_overrides_should_override_file_values() {
        let directory = tempdir().expect("temp directory should be created");
        let path = directory.path().join("arky.toml");
        fs::write(
            &path,
            r#"
                [workspace]
                default_provider = "default"

                [providers.default]
                kind = "claude-code"
                model = "file-model"

                [agents.writer]
                provider = "default"
                model = "file-model"
            "#,
        )
        .expect("config file should be written");

        let config = ConfigLoader::from_path(&path)
            .with_env_overrides([
                (
                    "ARKY_PROVIDERS__DEFAULT__MODEL".to_owned(),
                    "env-model".to_owned(),
                ),
                (
                    "ARKY_AGENTS__WRITER__MODEL".to_owned(),
                    "env-agent-model".to_owned(),
                ),
            ])
            .load()
            .expect("config should load");

        let actual = (
            config.provider("default").and_then(|value| value.model()),
            config.agent("writer").and_then(|value| value.model()),
        );

        let expected = (Some("env-model"), Some("env-agent-model"));

        assert_eq!(actual, expected);
    }

    #[test]
    fn builder_should_override_environment_values() {
        let config = ConfigLoader::new()
            .with_env_overrides([(
                "ARKY_PROVIDERS__DEFAULT__MODEL".to_owned(),
                "env-model".to_owned(),
            )])
            .with_builder_overrides(
                ArkyConfig::builder()
                    .workspace(WorkspaceConfigBuilder::new().default_provider("default"))
                    .provider(
                        "default",
                        ProviderConfigBuilder::new()
                            .kind("claude-code")
                            .model("builder-model"),
                    ),
            )
            .load()
            .expect("config should build");

        let actual = config.provider("default").and_then(|value| value.model());

        let expected = Some("builder-model");

        assert_eq!(actual, expected);
    }

    #[test]
    fn yaml_loading_should_parse_valid_config() {
        let directory = tempdir().expect("temp directory should be created");
        let path = directory.path().join("arky.yaml");
        fs::write(
            &path,
            r"
workspace:
  default_provider: default
providers:
  default:
    kind: codex
    binary: cargo
agents:
  reviewer:
    provider: default
    tools:
      - read_file
",
        )
        .expect("config file should be written");

        let config = ArkyConfig::from_path(&path).expect("config should load");

        let actual = (
            config.workspace().default_provider(),
            config.provider("default").map(super::ProviderConfig::kind),
            config.agent("reviewer").map(|value| value.tools().to_vec()),
        );

        let expected = (
            Some("default"),
            Some("codex"),
            Some(vec!["read_file".to_owned()]),
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn builder_should_support_programmatic_configuration() {
        let config = ArkyConfigBuilder::new()
            .workspace(
                WorkspaceConfigBuilder::new()
                    .name("workspace")
                    .default_provider("default")
                    .data_dir(PathBuf::from("/tmp/arky"))
                    .env("RUST_LOG", "debug"),
            )
            .provider(
                "default",
                ProviderConfigBuilder::new()
                    .kind("claude-code")
                    .binary("claude")
                    .model("claude-sonnet-4")
                    .args(["--json"])
                    .env("API_KEY", "secret"),
            )
            .agent(
                "writer",
                crate::AgentConfigBuilder::new()
                    .provider("default")
                    .model("claude-sonnet-4")
                    .instructions("write clearly")
                    .max_turns(6)
                    .tools(["search", "edit"])
                    .env("MODE", "draft"),
            )
            .build()
            .expect("config should build");

        let actual = (
            config.workspace().name(),
            config.workspace().data_dir(),
            config
                .provider("default")
                .map(|value| value.args().to_vec()),
            config
                .agent("writer")
                .and_then(|value| value.instructions()),
            config.agent("writer").map(|value| value.tools().to_vec()),
        );

        let expected = (
            Some("workspace"),
            Some(Path::new("/tmp/arky")),
            Some(vec!["--json".to_owned()]),
            Some("write clearly"),
            Some(vec!["search".to_owned(), "edit".to_owned()]),
        );

        assert_eq!(actual, expected);
    }
}
