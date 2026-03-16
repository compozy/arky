//! Validation and prerequisite checks for finalized configuration.

use std::{
    collections::BTreeMap,
    env,
    path::{
        Path,
        PathBuf,
    },
};

use crate::{
    error::{
        ConfigError,
        ValidationIssue,
    },
    loader::{
        AgentConfig,
        ArkyConfig,
        PartialAgentConfig,
        PartialArkyConfig,
        PartialProviderConfig,
        PartialWorkspaceConfig,
        ProviderConfig,
        WorkspaceConfig,
    },
};

pub fn validate_config(config: PartialArkyConfig) -> Result<ArkyConfig, ConfigError> {
    let mut issues = Vec::new();

    let workspace = finalize_workspace(config.workspace, &mut issues);
    let providers = finalize_providers(config.providers, &mut issues);
    let agents = finalize_agents(config.agents, &providers, &mut issues);

    if let Some(default_provider) = workspace.default_provider()
        && !providers.contains_key(default_provider)
    {
        issues.push(ValidationIssue::new(
            "workspace.default_provider",
            format!("references unknown provider `{default_provider}`"),
        ));
    }

    if issues.is_empty() {
        Ok(ArkyConfig::new(workspace, providers, agents))
    } else {
        Err(ConfigError::validation(issues))
    }
}

pub fn check_provider_prerequisites(
    config: &ArkyConfig,
) -> Result<BTreeMap<String, PathBuf>, ConfigError> {
    let mut resolved = BTreeMap::new();

    for (name, provider) in config.providers() {
        let binary = provider.prerequisite_binary();
        let path = find_binary_on_path(binary.as_str()).ok_or_else(|| {
            ConfigError::MissingBinary {
                provider: name.clone(),
                binary: binary.clone(),
            }
        })?;
        resolved.insert(name.clone(), path);
    }

    Ok(resolved)
}

/// Finds an executable on `PATH`.
#[must_use]
pub fn find_binary_on_path(binary: &str) -> Option<PathBuf> {
    let candidate = Path::new(binary);
    if candidate.components().count() > 1 || candidate.is_absolute() {
        return candidate
            .is_file()
            .then(|| candidate.to_path_buf())
            .filter(|path| is_executable(path));
    }

    let path_value = env::var_os("PATH")?;
    for directory in env::split_paths(&path_value) {
        for candidate_name in candidate_names(binary) {
            let candidate_path = directory.join(candidate_name);
            if candidate_path.is_file() && is_executable(&candidate_path) {
                return Some(candidate_path);
            }
        }
    }

    None
}

fn finalize_workspace(
    partial: PartialWorkspaceConfig,
    issues: &mut Vec<ValidationIssue>,
) -> WorkspaceConfig {
    let name = validate_optional_string(partial.name, "workspace.name", issues);
    let default_provider = validate_optional_string(
        partial.default_provider,
        "workspace.default_provider",
        issues,
    );

    WorkspaceConfig::new(
        name,
        default_provider,
        partial.data_dir,
        partial.env.unwrap_or_default(),
    )
}

fn finalize_providers(
    partials: BTreeMap<String, PartialProviderConfig>,
    issues: &mut Vec<ValidationIssue>,
) -> BTreeMap<String, ProviderConfig> {
    let mut providers = BTreeMap::new();

    for (name, partial) in partials {
        let field_prefix = format!("providers.{name}");
        let Some(kind) =
            require_string(partial.kind, format!("{field_prefix}.kind"), issues)
        else {
            continue;
        };

        let model = validate_optional_string(
            partial.model,
            format!("{field_prefix}.model"),
            issues,
        );

        let binary = partial.binary.and_then(|value| {
            let rendered = value.to_string_lossy().trim().to_owned();
            if rendered.is_empty() {
                issues.push(ValidationIssue::new(
                    format!("{field_prefix}.binary"),
                    "must not be empty",
                ));
                None
            } else {
                Some(value)
            }
        });

        providers.insert(
            name,
            ProviderConfig::new(
                kind,
                binary,
                model,
                partial.args.unwrap_or_default(),
                partial.env.unwrap_or_default(),
            ),
        );
    }

    providers
}

fn finalize_agents(
    partials: BTreeMap<String, PartialAgentConfig>,
    providers: &BTreeMap<String, ProviderConfig>,
    issues: &mut Vec<ValidationIssue>,
) -> BTreeMap<String, AgentConfig> {
    let mut agents = BTreeMap::new();

    for (name, partial) in partials {
        let field_prefix = format!("agents.{name}");
        let Some(provider) =
            require_string(partial.provider, format!("{field_prefix}.provider"), issues)
        else {
            continue;
        };

        if !providers.contains_key(provider.as_str()) {
            issues.push(ValidationIssue::new(
                format!("{field_prefix}.provider"),
                format!("references unknown provider `{provider}`"),
            ));
            continue;
        }

        let model = validate_optional_string(
            partial.model,
            format!("{field_prefix}.model"),
            issues,
        );
        let instructions = validate_optional_string(
            partial.instructions,
            format!("{field_prefix}.instructions"),
            issues,
        );
        let max_turns = partial.max_turns.and_then(|value| {
            if value == 0 {
                issues.push(ValidationIssue::new(
                    format!("{field_prefix}.max_turns"),
                    "must be greater than zero",
                ));
                None
            } else {
                Some(value)
            }
        });

        agents.insert(
            name,
            AgentConfig::new(
                provider,
                model,
                instructions,
                max_turns,
                partial.tools.unwrap_or_default(),
                partial.env.unwrap_or_default(),
            ),
        );
    }

    agents
}

fn require_string(
    value: Option<String>,
    field: impl Into<String>,
    issues: &mut Vec<ValidationIssue>,
) -> Option<String> {
    let field = field.into();

    let Some(value) = validate_optional_string(value, field.clone(), issues) else {
        issues.push(ValidationIssue::new(field, "is required"));
        return None;
    };

    Some(value)
}

fn validate_optional_string(
    value: Option<String>,
    field: impl Into<String>,
    issues: &mut Vec<ValidationIssue>,
) -> Option<String> {
    let field = field.into();

    value.and_then(|value| {
        let trimmed = value.trim().to_owned();
        if trimmed.is_empty() {
            issues.push(ValidationIssue::new(field, "must not be empty"));
            None
        } else {
            Some(trimmed)
        }
    })
}

fn candidate_names(binary: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let path = Path::new(binary);
        if path.extension().is_some() {
            return vec![binary.to_owned()];
        }

        if let Some(extensions) = env::var_os("PATHEXT") {
            let mut names = vec![binary.to_owned()];
            for extension in env::split_paths(&extensions) {
                names.push(format!("{binary}{}", extension.to_string_lossy()));
            }
            return names;
        }

        return vec![
            binary.to_owned(),
            format!("{binary}.exe"),
            format!("{binary}.cmd"),
            format!("{binary}.bat"),
        ];
    }

    #[cfg(not(windows))]
    {
        vec![binary.to_owned()]
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pretty_assertions::assert_eq;

    use super::{
        check_provider_prerequisites,
        validate_config,
    };
    use crate::{
        ConfigError,
        loader::{
            PartialAgentConfig,
            PartialArkyConfig,
            PartialProviderConfig,
            PartialWorkspaceConfig,
        },
    };

    #[test]
    fn validation_should_fail_when_required_fields_are_missing() {
        let config = PartialArkyConfig {
            workspace: PartialWorkspaceConfig::default(),
            providers: BTreeMap::from([(
                "default".to_owned(),
                PartialProviderConfig::default(),
            )]),
            agents: BTreeMap::from([(
                "writer".to_owned(),
                PartialAgentConfig {
                    provider: Some("missing".to_owned()),
                    ..PartialAgentConfig::default()
                },
            )]),
        };

        let error = validate_config(config).expect_err("validation should fail");

        let actual = match error {
            ConfigError::ValidationFailed { issues, .. } => issues
                .iter()
                .map(|issue| (issue.field().to_owned(), issue.message().to_owned()))
                .collect::<Vec<_>>(),
            other => panic!("expected validation error, got {other:?}"),
        };

        let expected = vec![
            (
                "providers.default.kind".to_owned(),
                "is required".to_owned(),
            ),
            (
                "agents.writer.provider".to_owned(),
                "references unknown provider `missing`".to_owned(),
            ),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn prerequisite_check_should_return_missing_binary_when_not_available() {
        let config = validate_config(PartialArkyConfig {
            workspace: PartialWorkspaceConfig::default(),
            providers: BTreeMap::from([(
                "default".to_owned(),
                PartialProviderConfig {
                    kind: Some("custom-provider".to_owned()),
                    binary: Some("definitely-not-a-real-binary-for-arky-tests".into()),
                    model: None,
                    args: None,
                    env: None,
                },
            )]),
            agents: BTreeMap::new(),
        })
        .expect("config should validate");

        let error =
            check_provider_prerequisites(&config).expect_err("binary lookup should fail");

        let actual = match error {
            ConfigError::MissingBinary { provider, binary } => (provider, binary),
            other => panic!("expected missing binary error, got {other:?}"),
        };

        let expected = (
            "default".to_owned(),
            "definitely-not-a-real-binary-for-arky-tests".to_owned(),
        );

        assert_eq!(actual, expected);
    }
}
