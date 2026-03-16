//! Facade crate for the Arky SDK.
//!
//! The facade keeps the skeleton ergonomically usable even before the concrete
//! implementations land by re-exporting the workspace crates from a single
//! entrypoint.

pub use arky_config as config;
pub use arky_core as core;
pub use arky_error as error;
pub use arky_error::ClassifiedError;
pub use arky_hooks as hooks;
pub use arky_mcp as mcp;
pub use arky_protocol as protocol;
pub use arky_provider as provider;
pub use arky_session as session;
pub use arky_tools as tools;
pub use arky_tools_macros as macros;
pub use arky_tools_macros::tool;

#[cfg(feature = "claude-code")]
pub use arky_claude_code as claude_code;

#[cfg(feature = "codex")]
pub use arky_codex as codex;

#[cfg(feature = "server")]
pub use arky_server as server;

/// Common exports for consumers that want one import surface.
pub mod prelude {
    pub use arky_error::ClassifiedError;
    pub use arky_tools_macros::tool;
}
