#![allow(dead_code)]

use std::{
    env,
    error::Error,
    fs,
    io,
    net::{
        IpAddr,
        Ipv4Addr,
        SocketAddr,
    },
    path::Path,
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
};

use arky::{
    AgentEvent,
    AgentEventStream,
    ClaudeCodeProvider,
    ClaudeCodeProviderConfig,
    CodexProvider,
    CodexProviderConfig,
    ContentBlock,
    EventSubscription,
    McpServerHandle,
    McpServerTransport,
    Message,
    ModelRef,
    ProviderEventStream,
    ProviderRequest,
    ProviderSettings,
    SessionId,
    SessionRef,
    ToolCall,
    ToolContent,
    ToolDescriptor,
    ToolOrigin,
    TurnContext,
    TurnId,
    prelude::*,
};
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{
    Value,
    json,
};
use tempfile::{
    TempDir,
    tempdir,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

pub type ExampleError = Box<dyn Error + Send + Sync>;
const STREAM_EVENT_TIMEOUT_SECS: u64 = 60;

pub const CLAUDE_TOOL_NAME: &str = "mcp/local/reveal_token";
pub const CODEX_TOOL_NAME: &str = "mcp/local/reveal_token";
pub const CODEX_BLOCKING_TOOL_NAME: &str = "mcp/local/wait_for_release";

pub fn print_section(title: &str) {
    println!("\n== {title} ==");
}

pub fn pass(message: &str) {
    println!("PASS: {message}");
}

pub fn text_from_message(message: &Message) -> String {
    let mut text = String::new();

    for block in &message.content {
        match block {
            ContentBlock::Text { text: fragment } => text.push_str(fragment),
            ContentBlock::ToolResult { result } => {
                for item in &result.content {
                    match item {
                        ToolContent::Text { text: fragment } => {
                            if !text.is_empty() {
                                text.push(' ');
                            }
                            text.push_str(fragment);
                        }
                        ToolContent::Json { value } => {
                            if !text.is_empty() {
                                text.push(' ');
                            }
                            text.push_str(&value.to_string());
                        }
                        ToolContent::Image { .. } => {}
                    }
                }
            }
            ContentBlock::ToolUse { .. } | ContentBlock::Image { .. } => {}
        }
    }

    text
}

pub fn require(condition: bool, message: impl Into<String>) -> Result<(), ExampleError> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message.into()).into())
    }
}

pub fn require_contains(
    actual: &str,
    expected_fragment: &str,
    context: &str,
) -> Result<(), ExampleError> {
    require(
        actual.contains(expected_fragment),
        format!("{context}: expected `{expected_fragment}` in `{actual}`"),
    )
}

pub fn require_event<F>(
    events: &[AgentEvent],
    context: &str,
    predicate: F,
) -> Result<(), ExampleError>
where
    F: Fn(&AgentEvent) -> bool,
{
    require(
        events.iter().any(predicate),
        format!("{context}: required event was not observed"),
    )
}

pub fn require_tool_execution(
    events: &[AgentEvent],
    tool_name: &str,
) -> Result<(), ExampleError> {
    let observed = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolExecutionEnd { tool_name, .. } => Some(tool_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    require(
        observed.contains(&tool_name),
        format!(
            "tool execution for `{tool_name}`: observed completed tools {observed:?}"
        ),
    )
}

pub fn require_any_tool_execution(
    events: &[AgentEvent],
    context: &str,
) -> Result<(), ExampleError> {
    require_event(events, context, |event| {
        matches!(event, AgentEvent::ToolExecutionEnd { .. })
    })
}

pub fn final_turn_text(events: &[AgentEvent]) -> Result<String, ExampleError> {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            AgentEvent::TurnEnd { message, .. } => Some(text_from_message(message)),
            _ => None,
        })
        .ok_or_else(|| io::Error::other("missing terminal TurnEnd event").into())
}

pub async fn collect_agent_stream(
    mut stream: AgentEventStream,
) -> Result<Vec<AgentEvent>, ExampleError> {
    let mut events = Vec::new();

    while let Some(item) =
        next_event_with_timeout("agent stream", &events, stream.next()).await?
    {
        events.push(item?);
    }

    Ok(events)
}

pub async fn collect_provider_stream(
    mut stream: ProviderEventStream,
) -> Result<Vec<AgentEvent>, ExampleError> {
    let mut events = Vec::new();

    while let Some(item) =
        next_event_with_timeout("provider stream", &events, stream.next()).await?
    {
        events.push(item?);
    }

    Ok(events)
}

pub async fn collect_subscription_until_agent_end(
    subscription: &mut EventSubscription,
) -> Result<Vec<AgentEvent>, ExampleError> {
    let mut events = Vec::new();

    loop {
        let event = tokio::time::timeout(
            std::time::Duration::from_secs(STREAM_EVENT_TIMEOUT_SECS),
            subscription.recv(),
        )
        .await
        .map_err(|_| {
            io::Error::other(format!(
                "event subscription timed out after {STREAM_EVENT_TIMEOUT_SECS}s; observed {}",
                summarize_events(&events)
            ))
        })??;
        let should_stop = matches!(event, AgentEvent::AgentEnd { .. });
        events.push(event);
        if should_stop {
            return Ok(events);
        }
    }
}

pub fn temporary_workspace(label: &str) -> Result<TempDir, ExampleError> {
    let workspace = tempdir()?;
    fs::write(
        workspace.path().join("README.md"),
        format!("# {label}\n\nTemporary workspace for Arky live examples.\n"),
    )?;
    fs::create_dir_all(workspace.path().join("scratch"))?;

    Ok(workspace)
}

pub fn claude_model() -> String {
    env::var("ARKY_CLAUDE_MODEL").unwrap_or_else(|_| "sonnet".to_owned())
}

pub fn codex_model() -> String {
    env::var("ARKY_CODEX_MODEL").unwrap_or_else(|_| "gpt-5".to_owned())
}

pub fn claude_provider(cwd: &Path) -> ClaudeCodeProvider {
    ClaudeCodeProvider::with_config(ClaudeCodeProviderConfig {
        cwd: Some(cwd.to_path_buf()),
        ..ClaudeCodeProviderConfig::default()
    })
}

pub fn codex_provider(cwd: &Path) -> CodexProvider {
    CodexProvider::with_config(CodexProviderConfig {
        cwd: Some(cwd.to_path_buf()),
        approval_policy: Some("never".to_owned()),
        ..CodexProviderConfig::default()
    })
}

pub fn request(
    model: &str,
    prompt: impl Into<String>,
    turn_index: u64,
) -> ProviderRequest {
    request_with_messages(
        SessionId::new(),
        model,
        turn_index,
        vec![Message::user(prompt.into())],
    )
}

pub fn request_with_session(
    session_id: SessionId,
    model: &str,
    prompt: impl Into<String>,
    turn_index: u64,
) -> ProviderRequest {
    request_with_messages(
        session_id,
        model,
        turn_index,
        vec![Message::user(prompt.into())],
    )
}

pub fn request_with_messages(
    session_id: SessionId,
    model: &str,
    turn_index: u64,
    messages: Vec<Message>,
) -> ProviderRequest {
    ProviderRequest::new(
        SessionRef::new(Some(session_id)),
        TurnContext::new(TurnId::new(), turn_index),
        ModelRef::new(model),
        messages,
    )
}

pub fn http_transport(path: &str) -> McpServerTransport {
    McpServerTransport::StreamableHttp {
        bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        path: path.to_owned(),
    }
}

pub fn handle_url(handle: &McpServerHandle) -> Result<String, ExampleError> {
    handle.url().map(ToOwned::to_owned).ok_or_else(|| {
        io::Error::other("expected MCP server to expose an HTTP URL").into()
    })
}

pub fn codex_mcp_settings(url: &str) -> ProviderSettings {
    let mut settings = ProviderSettings::new();
    settings.extra.insert(
        "mcp_servers".to_owned(),
        json!({
            "runtime": {
                "url": url,
            },
        }),
    );
    settings
        .extra
        .insert("rmcp_client".to_owned(), Value::Bool(true));
    settings.extra.insert(
        "developer_instructions".to_owned(),
        Value::String(
            "Use runtime MCP tools whenever the user explicitly asks for them."
                .to_owned(),
        ),
    );
    settings
}

pub struct RevealTokenTool {
    descriptor: ToolDescriptor,
    token: String,
}

impl RevealTokenTool {
    pub fn new(
        canonical_name: &str,
        token: impl Into<String>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            descriptor: ToolDescriptor::new(
                canonical_name,
                "Reveal Token",
                "Return the exact verification token for the live example.",
                json!({
                    "type": "object",
                    "properties": {},
                }),
                ToolOrigin::Local,
            )?,
            token: token.into(),
        })
    }
}

#[async_trait]
impl Tool for RevealTokenTool {
    fn descriptor(&self) -> ToolDescriptor {
        self.descriptor.clone()
    }

    async fn execute(
        &self,
        call: ToolCall,
        _cancel: CancellationToken,
    ) -> Result<ToolResult, ToolError> {
        Ok(ToolResult::success(
            call.id,
            call.name,
            vec![ToolContent::text(self.token.clone())],
        ))
    }
}

pub struct BlockingTokenTool {
    descriptor: ToolDescriptor,
    token: String,
    started: Arc<Notify>,
    release: Arc<Notify>,
    started_flag: Arc<AtomicBool>,
}

impl BlockingTokenTool {
    pub fn new(
        canonical_name: &str,
        token: impl Into<String>,
        started: Arc<Notify>,
        release: Arc<Notify>,
        started_flag: Arc<AtomicBool>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            descriptor: ToolDescriptor::new(
                canonical_name,
                "Wait For Release",
                "Block until the live example releases the tool, then return the verification token.",
                json!({
                    "type": "object",
                    "properties": {},
                }),
                ToolOrigin::Local,
            )?,
            token: token.into(),
            started,
            release,
            started_flag,
        })
    }
}

#[async_trait]
impl Tool for BlockingTokenTool {
    fn descriptor(&self) -> ToolDescriptor {
        self.descriptor.clone()
    }

    async fn execute(
        &self,
        call: ToolCall,
        cancel: CancellationToken,
    ) -> Result<ToolResult, ToolError> {
        self.started_flag.store(true, Ordering::SeqCst);
        self.started.notify_waiters();

        tokio::select! {
            () = cancel.cancelled() => Err(ToolError::cancelled(
                "blocking tool cancelled before release",
                Some(call.name.clone()),
            )),
            () = self.release.notified() => Ok(ToolResult::success(
                call.id,
                call.name,
                vec![ToolContent::text(self.token.clone())],
            )),
        }
    }
}

pub fn describe_event(event: &AgentEvent) -> String {
    match event {
        AgentEvent::AgentStart { .. } => "agent_start".to_owned(),
        AgentEvent::AgentEnd { .. } => "agent_end".to_owned(),
        AgentEvent::TurnStart { .. } => "turn_start".to_owned(),
        AgentEvent::TurnEnd { .. } => "turn_end".to_owned(),
        AgentEvent::MessageStart { .. } => "message_start".to_owned(),
        AgentEvent::MessageUpdate { delta, .. } => format!("message_update::{delta:?}"),
        AgentEvent::MessageEnd { .. } => "message_end".to_owned(),
        AgentEvent::ToolExecutionStart { tool_name, .. } => {
            format!("tool_start::{tool_name}")
        }
        AgentEvent::ToolExecutionUpdate { tool_name, .. } => {
            format!("tool_update::{tool_name}")
        }
        AgentEvent::ToolExecutionEnd { tool_name, .. } => {
            format!("tool_end::{tool_name}")
        }
        AgentEvent::Custom { event_type, .. } => format!("custom::{event_type}"),
        _ => "other".to_owned(),
    }
}

fn summarize_events(events: &[AgentEvent]) -> String {
    if events.is_empty() {
        return "no events".to_owned();
    }

    events
        .iter()
        .map(describe_event)
        .collect::<Vec<_>>()
        .join(", ")
}

async fn next_event_with_timeout<T>(
    stream_name: &str,
    observed_events: &[AgentEvent],
    future: impl std::future::Future<Output = Option<T>>,
) -> Result<Option<T>, ExampleError> {
    tokio::time::timeout(
        std::time::Duration::from_secs(STREAM_EVENT_TIMEOUT_SECS),
        future,
    )
    .await
    .map_err(|_| {
        io::Error::other(format!(
            "{stream_name} timed out after {STREAM_EVENT_TIMEOUT_SECS}s; observed {}",
            summarize_events(observed_events)
        ))
    })
    .map_err(Into::into)
}
