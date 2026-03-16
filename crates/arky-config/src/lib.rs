//! Configuration loading, merging, and validation for Arky.
//!
//! Use this crate to load provider/runtime settings from disk, environment
//! variables, and explicit overrides before handing a finalized configuration
//! to the rest of the SDK.

mod error;
mod loader;
mod merge;
mod validate;

pub use crate::{
    error::{
        ConfigError,
        ValidationIssue,
    },
    loader::{
        AgentConfig,
        AgentConfigBuilder,
        ArkyConfig,
        ArkyConfigBuilder,
        ConfigFormat,
        ConfigLoader,
        ProviderConfig,
        ProviderConfigBuilder,
        WorkspaceConfig,
        WorkspaceConfigBuilder,
    },
    validate::find_binary_on_path,
};
