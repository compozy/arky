//! Agent construction helpers.

use std::sync::Arc;

use arky_config::AgentConfig;
use arky_hooks::{
    HookChain,
    Hooks,
};
use arky_protocol::ModelRef;
use arky_provider::Provider;
use arky_session::{
    InMemorySessionStore,
    SessionStore,
};
use arky_tools::{
    Tool,
    ToolRegistry,
};
use tokio::sync::broadcast;

use crate::{
    Agent,
    CoreError,
    turn::TurnRuntime,
};

const DEFAULT_EVENT_BUFFER: usize = 256;

/// Builder for [`crate::Agent`].
pub struct AgentBuilder {
    provider: Option<Arc<dyn Provider>>,
    tools: Vec<Arc<dyn Tool>>,
    temporary_tools: Vec<Arc<dyn Tool>>,
    hooks: Option<Arc<dyn Hooks>>,
    session_store: Option<Arc<dyn SessionStore>>,
    config: Option<AgentConfig>,
    model: Option<String>,
    system_prompt: Option<String>,
    resume_session_id: Option<arky_protocol::SessionId>,
    event_buffer: usize,
}

impl AgentBuilder {
    /// Creates an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            provider: None,
            tools: Vec::new(),
            temporary_tools: Vec::new(),
            hooks: None,
            session_store: None,
            config: None,
            model: None,
            system_prompt: None,
            resume_session_id: None,
            event_buffer: DEFAULT_EVENT_BUFFER,
        }
    }

    /// Registers the provider used by the agent.
    #[must_use]
    pub fn provider<P>(mut self, provider: P) -> Self
    where
        P: Provider + 'static,
    {
        self.provider = Some(Arc::new(provider));
        self
    }

    /// Registers a provider from a shared pointer.
    #[must_use]
    pub fn provider_arc(mut self, provider: Arc<dyn Provider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Registers a long-lived tool.
    #[must_use]
    pub fn tool<T>(mut self, tool: T) -> Self
    where
        T: Tool + 'static,
    {
        self.tools.push(Arc::new(tool));
        self
    }

    /// Registers a temporary tool that is re-bound per run and cleaned up
    /// when the run finishes.
    #[must_use]
    pub fn temporary_tool<T>(mut self, tool: T) -> Self
    where
        T: Tool + 'static,
    {
        self.temporary_tools.push(Arc::new(tool));
        self
    }

    /// Registers hooks used by the agent.
    #[must_use]
    pub fn hooks<H>(mut self, hooks: H) -> Self
    where
        H: Hooks + 'static,
    {
        self.hooks = Some(Arc::new(hooks));
        self
    }

    /// Registers hooks from a shared pointer.
    #[must_use]
    pub fn hooks_arc(mut self, hooks: Arc<dyn Hooks>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Overrides the session store backend.
    #[must_use]
    pub fn session_store<S>(mut self, session_store: S) -> Self
    where
        S: SessionStore + 'static,
    {
        self.session_store = Some(Arc::new(session_store));
        self
    }

    /// Overrides the session store backend with a shared pointer.
    #[must_use]
    pub fn session_store_arc(mut self, session_store: Arc<dyn SessionStore>) -> Self {
        self.session_store = Some(session_store);
        self
    }

    /// Applies a pre-loaded agent config.
    #[must_use]
    pub fn config(mut self, config: AgentConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Sets the model identifier sent to providers.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the system prompt prepended to provider requests.
    #[must_use]
    pub fn system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    /// Preconfigures the builder to resume the provided session on first use.
    #[must_use]
    pub const fn resume(mut self, session_id: arky_protocol::SessionId) -> Self {
        self.resume_session_id = Some(session_id);
        self
    }

    /// Adjusts the broadcast buffer used by `subscribe()`.
    #[must_use]
    pub const fn event_buffer(mut self, event_buffer: usize) -> Self {
        self.event_buffer = event_buffer;
        self
    }

    /// Builds an [`Agent`].
    pub fn build(self) -> Result<Agent, CoreError> {
        let model = resolve_model(&self)?;
        let provider = self.provider.ok_or_else(|| {
            CoreError::invalid_state("AgentBuilder requires a provider", None)
        })?;
        let system_prompt = self.system_prompt.or_else(|| {
            self.config
                .as_ref()
                .and_then(AgentConfig::instructions)
                .map(ToOwned::to_owned)
        });
        if let Some(config) = &self.config {
            let provider_id = provider.descriptor().id.as_str();
            if config.provider() != provider_id {
                return Err(CoreError::invalid_state(
                    format!(
                        "agent config provider `{}` does not match registered provider `{provider_id}`",
                        config.provider()
                    ),
                    None,
                ));
            }
        }

        let tools = ToolRegistry::new();
        for tool in self.tools {
            tools.register_arc(tool).map_err(|error| {
                CoreError::invalid_state(
                    format!("failed to register tool: {error}"),
                    Some(serde_json::json!({
                        "error_code": arky_error::ClassifiedError::error_code(&error),
                    })),
                )
            })?;
        }

        let hooks = self.hooks.unwrap_or_else(|| Arc::new(HookChain::new()));
        let session_store = self
            .session_store
            .unwrap_or_else(|| Arc::new(InMemorySessionStore::default()));
        let (events, _) = broadcast::channel(self.event_buffer.max(1));
        let runtime = TurnRuntime {
            provider,
            tools,
            temporary_tools: self.temporary_tools,
            hooks,
            session_store,
            model: ModelRef::new(model),
            system_prompt,
            provider_settings: arky_protocol::ProviderSettings::new(),
            events,
        };

        Ok(Agent::new(runtime, self.resume_session_id))
    }
}

fn resolve_model(builder: &AgentBuilder) -> Result<String, CoreError> {
    builder
        .model
        .clone()
        .or_else(|| {
            builder
                .config
                .as_ref()
                .and_then(AgentConfig::model)
                .map(ToOwned::to_owned)
        })
        .ok_or_else(|| {
            CoreError::invalid_state(
                "AgentBuilder requires a model via .model(...) or AgentConfig",
                None,
            )
        })
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}
