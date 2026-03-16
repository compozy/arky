//! Provider descriptor and capability metadata.

use arky_protocol::ProviderId;
use serde::{
    Deserialize,
    Serialize,
};

/// Stable provider family classification used for routing and UX.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFamily {
    /// Anthropic Claude Code CLI wrapper.
    ClaudeCode,
    /// `OpenAI` Codex App Server wrapper.
    Codex,
    /// User-defined or third-party family label.
    Custom(String),
}

/// Capability flags exposed by a provider implementation.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the techspec requires these discrete capability flags on the public contract"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    /// Whether streaming is supported.
    pub streaming: bool,
    /// Whether direct generation is supported.
    pub generate: bool,
    /// Whether tool calls are supported.
    pub tool_calls: bool,
    /// Whether MCP servers can be passed through directly.
    pub mcp_passthrough: bool,
    /// Whether provider-native session resume is supported.
    pub session_resume: bool,
    /// Whether mid-turn steering is supported.
    pub steering: bool,
    /// Whether follow-up turns can be issued natively.
    pub follow_up: bool,
}

impl ProviderCapabilities {
    /// Creates an empty capability set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            streaming: false,
            generate: false,
            tool_calls: false,
            mcp_passthrough: false,
            session_resume: false,
            steering: false,
            follow_up: false,
        }
    }

    /// Sets the streaming flag.
    #[must_use]
    pub const fn with_streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Sets the generate flag.
    #[must_use]
    pub const fn with_generate(mut self, generate: bool) -> Self {
        self.generate = generate;
        self
    }

    /// Sets the tool-calls flag.
    #[must_use]
    pub const fn with_tool_calls(mut self, tool_calls: bool) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    /// Sets the MCP passthrough flag.
    #[must_use]
    pub const fn with_mcp_passthrough(mut self, mcp_passthrough: bool) -> Self {
        self.mcp_passthrough = mcp_passthrough;
        self
    }

    /// Sets the session-resume flag.
    #[must_use]
    pub const fn with_session_resume(mut self, session_resume: bool) -> Self {
        self.session_resume = session_resume;
        self
    }

    /// Sets the steering flag.
    #[must_use]
    pub const fn with_steering(mut self, steering: bool) -> Self {
        self.steering = steering;
        self
    }

    /// Sets the follow-up flag.
    #[must_use]
    pub const fn with_follow_up(mut self, follow_up: bool) -> Self {
        self.follow_up = follow_up;
        self
    }
}

/// Immutable provider metadata exposed through the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDescriptor {
    /// Stable provider identifier.
    pub id: ProviderId,
    /// Provider family classification.
    pub family: ProviderFamily,
    /// Supported capabilities.
    pub capabilities: ProviderCapabilities,
}

impl ProviderDescriptor {
    /// Creates a provider descriptor.
    #[must_use]
    pub const fn new(
        id: ProviderId,
        family: ProviderFamily,
        capabilities: ProviderCapabilities,
    ) -> Self {
        Self {
            id,
            family,
            capabilities,
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{
        ProviderCapabilities,
        ProviderDescriptor,
        ProviderFamily,
    };
    use arky_protocol::ProviderId;

    #[test]
    fn provider_descriptor_should_preserve_construction_inputs() {
        let capabilities = ProviderCapabilities::new()
            .with_streaming(true)
            .with_generate(true)
            .with_tool_calls(true)
            .with_session_resume(true);
        let descriptor = ProviderDescriptor::new(
            ProviderId::new("claude-code"),
            ProviderFamily::ClaudeCode,
            capabilities,
        );

        assert_eq!(descriptor.id.as_str(), "claude-code");
        assert_eq!(descriptor.family, ProviderFamily::ClaudeCode);
        assert_eq!(descriptor.capabilities, capabilities);
    }

    #[test]
    fn provider_capabilities_should_toggle_individual_flags() {
        let capabilities = ProviderCapabilities::new()
            .with_streaming(true)
            .with_generate(true)
            .with_tool_calls(true)
            .with_mcp_passthrough(true)
            .with_session_resume(true)
            .with_steering(true)
            .with_follow_up(true);

        assert_eq!(
            capabilities,
            ProviderCapabilities {
                streaming: true,
                generate: true,
                tool_calls: true,
                mcp_passthrough: true,
                session_resume: true,
                steering: true,
                follow_up: true,
            }
        );
    }
}
