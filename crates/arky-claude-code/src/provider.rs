//! Claude Code provider orchestration over the shared subprocess layer.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::Arc,
    time::{
        SystemTime,
        UNIX_EPOCH,
    },
};

use arky_protocol::{
    AgentEvent,
    ContentBlock,
    EventMetadata,
    Message,
    ProviderId,
    Role,
    StreamDelta,
    ToolCall,
    ToolContent,
    ToolResult,
};
use arky_provider::{
    ManagedProcess,
    ProcessConfig,
    ProcessManager,
    Provider,
    ProviderCapabilities,
    ProviderDescriptor,
    ProviderError,
    ProviderEventStream,
    ProviderFamily,
    ProviderRequest,
    StdioTransport,
    StdioTransportConfig,
};
use arky_tools::{
    ToolIdCodec,
    create_claude_code_tool_id_codec,
};
use async_stream::try_stream;
use serde_json::{
    Value,
    json,
};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use crate::{
    cooldown::{
        SpawnFailurePolicy,
        SpawnFailureTracker,
    },
    dedup::{
        TextDeduplicator,
        TextSource,
    },
    nested::NestedToolTracker,
    parser::{
        ClaudeEventParser,
        ClaudeEventSource,
        ClaudeNormalizedEvent,
    },
    session::SessionManager,
    tool_fsm::{
        ToolLifecycleState,
        ToolLifecycleTracker,
    },
};

/// Runtime configuration for the Claude Code provider.
#[derive(Debug, Clone)]
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
    /// Whether `--verbose` should be added to Claude invocations.
    pub verbose: bool,
    /// Maximum line length accepted from Claude stdout.
    pub max_frame_len: usize,
    /// Spawn-failure cooldown policy.
    pub spawn_failure_policy: SpawnFailurePolicy,
}

impl Default for ClaudeCodeProviderConfig {
    fn default() -> Self {
        Self {
            binary: "claude".to_owned(),
            cwd: None,
            extra_args: Vec::new(),
            env: BTreeMap::new(),
            version_args: vec!["--version".to_owned()],
            verbose: true,
            max_frame_len: 256 * 1024,
            spawn_failure_policy: SpawnFailurePolicy::default(),
        }
    }
}

/// Concrete `Provider` implementation wrapping the Claude CLI.
#[derive(Clone)]
pub struct ClaudeCodeProvider {
    descriptor: ProviderDescriptor,
    config: ClaudeCodeProviderConfig,
    sessions: SessionManager,
    cooldown: SpawnFailureTracker,
    codec: Arc<dyn ToolIdCodec>,
    validated_version: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl ClaudeCodeProvider {
    /// Creates a provider with the default Claude configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ClaudeCodeProviderConfig::default())
    }

    /// Creates a provider with an explicit runtime configuration.
    #[must_use]
    pub fn with_config(config: ClaudeCodeProviderConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::new(
                ProviderId::new("claude-code"),
                ProviderFamily::ClaudeCode,
                ProviderCapabilities::new()
                    .with_streaming(true)
                    .with_generate(true)
                    .with_tool_calls(true)
                    .with_mcp_passthrough(true)
                    .with_session_resume(true),
            ),
            cooldown: SpawnFailureTracker::new(config.spawn_failure_policy),
            config,
            sessions: SessionManager::new(),
            codec: Arc::new(create_claude_code_tool_id_codec()),
            validated_version: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    async fn ensure_binary_validated(&self) -> Result<String, ProviderError> {
        let cached_version = self.validated_version.lock().await.clone();
        if let Some(version) = cached_version {
            return Ok(version);
        }

        let manager = ProcessManager::new(self.version_process_config());
        let mut process = manager.spawn()?;
        let mut stdout = process.take_stdout()?;
        let mut stderr = process.take_stderr()?;
        let mut stdout_buffer = String::new();
        let mut stderr_buffer = String::new();
        tokio::try_join!(
            stdout.read_to_string(&mut stdout_buffer),
            stderr.read_to_string(&mut stderr_buffer),
        )
        .map_err(|error| {
            ProviderError::stream_interrupted(format!(
                "failed to read Claude version output: {error}"
            ))
        })?;
        process.wait().await?;

        let version = stdout_buffer.trim();
        if version.is_empty() {
            return Err(ProviderError::protocol_violation(
                "Claude version command returned empty stdout",
                Some(json!({
                    "stderr": stderr_buffer.trim(),
                })),
            ));
        }

        *self.validated_version.lock().await = Some(version.to_owned());
        Ok(version.to_owned())
    }

    fn version_process_config(&self) -> ProcessConfig {
        let mut config = ProcessConfig::new(&self.config.binary)
            .with_args(self.config.version_args.clone())
            .with_kill_on_drop(true);
        if let Some(cwd) = &self.config.cwd {
            config = config.with_cwd(cwd.clone());
        }
        for (key, value) in &self.config.env {
            config = config.with_env(key.clone(), value.clone());
        }
        config
    }

    async fn build_process_config(
        &self,
        request: &ProviderRequest,
    ) -> Result<ProcessConfig, ProviderError> {
        let prompt = render_prompt(request);
        let mut args = self.config.extra_args.clone();
        // Claude's `--output-format stream-json` currently requires `--verbose`.
        if self.config.verbose || !args.iter().any(|arg| arg == "--verbose") {
            args.push("--verbose".to_owned());
        }
        args.push("--print".to_owned());
        args.push(prompt);
        args.push("--output-format".to_owned());
        args.push("stream-json".to_owned());
        args.push("--model".to_owned());
        args.push(
            request
                .model
                .provider_model_id
                .clone()
                .unwrap_or_else(|| request.model.model_id.clone()),
        );

        if let Some(session_id) = self.sessions.resolve(&request.session).await {
            args.push("--session-id".to_owned());
            args.push(session_id);
        }

        let mut config = ProcessConfig::new(&self.config.binary)
            .with_args(args)
            .with_kill_on_drop(true);
        if let Some(cwd) = &self.config.cwd {
            config = config.with_cwd(cwd.clone());
        }
        for (key, value) in &self.config.env {
            config = config.with_env(key.clone(), value.clone());
        }
        Ok(config)
    }

    fn canonical_tool_name(&self, tool_name: &str) -> String {
        match self.codec.decode(tool_name) {
            Ok(parsed) => parsed.canonical_name,
            Err(_) => tool_name.to_owned(),
        }
    }

    fn build_stream(
        &self,
        request: ProviderRequest,
        mut process: ManagedProcess,
        transport: StdioTransport,
    ) -> Result<ProviderEventStream, ProviderError> {
        let stderr = process.take_stderr()?;
        let provider = self.clone();
        let descriptor = self.descriptor.clone();
        let sessions = self.sessions.clone();

        let stream: ProviderEventStream = Box::pin(try_stream! {
            let stderr_task = tokio::spawn(async move {
                let mut stderr = stderr;
                let mut buffer = String::new();
                let _ = stderr.read_to_string(&mut buffer).await;
                buffer
            });
            let mut transport = transport;
            let cancel = CancellationToken::new();
            let mut parser = ClaudeEventParser::new();
            let mut runtime =
                StreamRuntime::new(&request, descriptor.id.clone(), provider, sessions);

            yield runtime.turn_start();

            while let Some(line) = transport.recv_frame(cancel.child_token()).await? {
                let normalized_events = parser.parse_line(&line)?;
                for event in normalized_events {
                    for emitted in runtime.handle_event(event).await? {
                        yield emitted;
                    }
                }
            }

            drop(transport);
            let stderr_excerpt = stderr_task.await.unwrap_or_default();
            let terminal_error = runtime.terminal_error();
            match process.wait().await {
                Ok(_) if runtime.finished() && terminal_error.is_none() => {}
                Ok(_) if terminal_error.is_some() => Err(terminal_error.expect("checked is_some"))?,
                Ok(_) => Err(ProviderError::protocol_violation(
                    "Claude stream ended without a terminal finish event",
                    Some(json!({
                        "stderr": stderr_excerpt.trim(),
                    })),
                ))?,
                Err(ProviderError::ProcessCrashed { .. }) if terminal_error.is_some() => {
                    Err(terminal_error.expect("checked is_some"))?
                }
                Err(ProviderError::ProcessCrashed { command, exit_code, .. }) => {
                    Err(ProviderError::process_crashed(
                        command,
                        exit_code,
                        (!stderr_excerpt.trim().is_empty())
                            .then(|| stderr_excerpt.trim().to_owned()),
                    ))?
                }
                Err(error) => Err(error)?,
            }
        });

        Ok(stream)
    }
}

#[async_trait::async_trait]
impl Provider for ClaudeCodeProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn stream(
        &self,
        request: ProviderRequest,
    ) -> Result<ProviderEventStream, ProviderError> {
        let _version = self.ensure_binary_validated().await?;
        self.cooldown.wait_until_ready().await;

        let process_config = self.build_process_config(&request).await?;
        let manager = ProcessManager::new(process_config);
        let mut process = match manager.spawn() {
            Ok(process) => process,
            Err(error) => {
                let _ = self.cooldown.record_failure().await;
                return Err(error);
            }
        };
        self.cooldown.record_success().await;

        let stdin = process.take_stdin()?;
        drop(stdin);
        let stdout = process.take_stdout()?;
        let transport = StdioTransport::new(
            stdout,
            tokio::io::sink(),
            StdioTransportConfig {
                max_frame_len: self.config.max_frame_len,
                ..StdioTransportConfig::default()
            },
        );

        self.build_stream(request, process, transport)
    }
}

impl Default for ClaudeCodeProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn render_prompt(request: &ProviderRequest) -> String {
    let mut prompt = String::new();

    for message in &request.messages {
        let role = match message.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
            Role::Tool => "Tool",
        };
        let content = message
            .content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text } => text.clone(),
                ContentBlock::ToolUse { call } => {
                    format!("ToolUse {} {} {}", call.id, call.name, call.input)
                }
                ContentBlock::ToolResult { result } => format!(
                    "ToolResult {} {} {}",
                    result.id,
                    result.name,
                    tool_contents_to_text(&result.content)
                ),
                ContentBlock::Image { media_type, .. } => format!("Image<{media_type}>"),
            })
            .collect::<Vec<_>>()
            .join("\n");

        if !prompt.is_empty() {
            prompt.push_str("\n\n");
        }
        prompt.push_str(role);
        prompt.push_str(": ");
        prompt.push_str(&content);
    }

    if !request.tools.definitions.is_empty() {
        prompt.push_str("\n\nAvailable tools:\n");
        prompt.push_str(
            &serde_json::to_string(&request.tools.definitions)
                .unwrap_or_else(|_| "[]".to_owned()),
        );
    }

    prompt
}

fn tool_contents_to_text(content: &[ToolContent]) -> String {
    content
        .iter()
        .map(|item| match item {
            ToolContent::Text { text } => text.clone(),
            ToolContent::Json { value } => value.to_string(),
            ToolContent::Image { media_type, .. } => format!("image:{media_type}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn append_text_block(message: &mut Message, text: &str) {
    if let Some(ContentBlock::Text { text: current }) = message.content.last_mut() {
        current.push_str(text);
        return;
    }

    message.content.push(ContentBlock::text(text.to_owned()));
}

fn append_tool_use_block(message: &mut Message, tool_call: &ToolCall) {
    if message.content.iter().any(|block| {
        matches!(block, ContentBlock::ToolUse { call } if call.id == tool_call.id)
    }) {
        return;
    }

    message
        .content
        .push(ContentBlock::tool_use(tool_call.clone()));
}

fn parse_json_or_string(input: &str) -> Value {
    serde_json::from_str::<Value>(input)
        .unwrap_or_else(|_| Value::String(input.to_owned()))
}

fn current_tool_name(tool_fsm: &ToolLifecycleTracker, tool_call_id: &str) -> String {
    match tool_fsm.state(tool_call_id) {
        ToolLifecycleState::Started { tool_name }
        | ToolLifecycleState::InputReceiving { tool_name, .. }
        | ToolLifecycleState::Executing { tool_name, .. }
        | ToolLifecycleState::Completed { tool_name, .. } => tool_name,
        ToolLifecycleState::Idle => "unknown".to_owned(),
    }
}

struct StreamRuntime {
    provider: ClaudeCodeProvider,
    sessions: SessionManager,
    tool_fsm: ToolLifecycleTracker,
    deduplicator: TextDeduplicator,
    nested_tools: NestedToolTracker,
    tool_results: Vec<ToolResult>,
    emitter: EventEmitter,
    current_message: Message,
    message_started: bool,
    finished: bool,
    terminal_error: Option<ProviderError>,
}

impl StreamRuntime {
    fn new(
        request: &ProviderRequest,
        provider_id: ProviderId,
        provider: ClaudeCodeProvider,
        sessions: SessionManager,
    ) -> Self {
        Self {
            provider,
            sessions,
            tool_fsm: ToolLifecycleTracker::new(),
            deduplicator: TextDeduplicator::new(),
            nested_tools: NestedToolTracker::new(),
            tool_results: Vec::new(),
            emitter: EventEmitter::new(request, provider_id),
            current_message: Message::new(Role::Assistant, Vec::new()),
            message_started: false,
            finished: false,
            terminal_error: None,
        }
    }

    fn turn_start(&mut self) -> AgentEvent {
        AgentEvent::TurnStart {
            meta: self.emitter.next(),
        }
    }

    const fn finished(&self) -> bool {
        self.finished
    }

    fn terminal_error(&self) -> Option<ProviderError> {
        self.terminal_error.clone()
    }

    async fn handle_event(
        &mut self,
        event: ClaudeNormalizedEvent,
    ) -> Result<Vec<AgentEvent>, ProviderError> {
        match event {
            ClaudeNormalizedEvent::Metadata(metadata) => {
                self.handle_metadata(metadata).await
            }
            ClaudeNormalizedEvent::RateLimit(rate_limit) => {
                self.handle_rate_limit(rate_limit).await
            }
            ClaudeNormalizedEvent::TextDelta(text) => Ok(self.handle_text_delta(&text)),
            ClaudeNormalizedEvent::ToolUseStart(tool) => self.handle_tool_use_start(tool),
            ClaudeNormalizedEvent::ToolUseInputDelta(input) => {
                self.handle_tool_use_input_delta(input)
            }
            ClaudeNormalizedEvent::ToolUseComplete(tool) => {
                self.handle_tool_use_complete(&tool)
            }
            ClaudeNormalizedEvent::ToolProgress(progress) => {
                Ok(self.handle_tool_progress(progress))
            }
            ClaudeNormalizedEvent::ToolResult(tool) => self.handle_tool_result(tool),
            ClaudeNormalizedEvent::Finish(finish) => Ok(self.handle_finish(&finish)),
        }
    }

    fn ensure_message_started(&mut self, emitted: &mut Vec<AgentEvent>) {
        if self.message_started {
            return;
        }

        self.message_started = true;
        emitted.push(AgentEvent::MessageStart {
            meta: self.emitter.next(),
            message: self.current_message.clone(),
        });
    }

    async fn handle_metadata(
        &mut self,
        metadata: crate::parser::ClaudeMetadataEvent,
    ) -> Result<Vec<AgentEvent>, ProviderError> {
        if let Some(session_id) = metadata.session_id.clone()
            && let Some(request_session_id) = self.emitter.session_id.clone()
        {
            self.sessions
                .record(&request_session_id, session_id.clone())
                .await;
        }
        if metadata.session_id.is_none() && metadata.model_id.is_none() {
            return Ok(Vec::new());
        }

        Ok(vec![AgentEvent::Custom {
            meta: self.emitter.next(),
            event_type: "claude_code.metadata".to_owned(),
            payload: json!({
                "session_id": metadata.session_id,
                "model_id": metadata.model_id,
            }),
        }])
    }

    async fn handle_rate_limit(
        &mut self,
        rate_limit: crate::parser::ClaudeRateLimitEvent,
    ) -> Result<Vec<AgentEvent>, ProviderError> {
        if let Some(session_id) = rate_limit.session_id.clone()
            && let Some(request_session_id) = self.emitter.session_id.clone()
        {
            self.sessions.record(&request_session_id, session_id).await;
        }

        Ok(vec![AgentEvent::Custom {
            meta: self.emitter.next(),
            event_type: "claude_code.rate_limit".to_owned(),
            payload: json!({
                "session_id": rate_limit.session_id,
                "rate_limit_info": rate_limit.rate_limit_info,
            }),
        }])
    }

    fn handle_text_delta(
        &mut self,
        text: &crate::parser::ClaudeTextDeltaEvent,
    ) -> Vec<AgentEvent> {
        let mut emitted = Vec::new();
        self.ensure_message_started(&mut emitted);
        let deduplicated = self.deduplicator.process(
            match text.source {
                ClaudeEventSource::StreamEvent => TextSource::StreamEvent,
                _ => TextSource::Assistant,
            },
            &text.text,
        );
        if deduplicated.is_empty() {
            return emitted;
        }

        append_text_block(&mut self.current_message, &deduplicated);
        emitted.push(AgentEvent::MessageUpdate {
            meta: self.emitter.next(),
            message: self.current_message.clone(),
            delta: StreamDelta::text(deduplicated),
        });
        emitted
    }

    fn handle_tool_use_start(
        &mut self,
        tool: crate::parser::ClaudeToolUseStartEvent,
    ) -> Result<Vec<AgentEvent>, ProviderError> {
        let mut emitted = Vec::new();
        self.ensure_message_started(&mut emitted);
        self.deduplicator.reset();
        self.tool_fsm.start(&tool.tool_call_id, &tool.tool_name)?;
        if let Some(parent_tool_call_id) = &tool.parent_tool_call_id {
            self.nested_tools.register_start(
                parent_tool_call_id,
                &tool.tool_call_id,
                &tool.tool_name,
                tool.input.clone(),
            );
        }

        emitted.push(AgentEvent::ToolExecutionStart {
            meta: self.emitter.next(),
            tool_call_id: tool.tool_call_id,
            tool_name: self.provider.canonical_tool_name(&tool.tool_name),
            args: tool.input,
        });
        Ok(emitted)
    }

    fn handle_tool_use_input_delta(
        &mut self,
        input: crate::parser::ClaudeToolUseInputDeltaEvent,
    ) -> Result<Vec<AgentEvent>, ProviderError> {
        self.tool_fsm
            .input_delta(&input.tool_call_id, &input.delta)?;
        Ok(vec![
            AgentEvent::ToolExecutionUpdate {
                meta: self.emitter.next(),
                tool_call_id: input.tool_call_id.clone(),
                tool_name: current_tool_name(&self.tool_fsm, &input.tool_call_id),
                partial_result: json!({
                    "input_delta": input.delta.clone(),
                }),
            },
            AgentEvent::MessageUpdate {
                meta: self.emitter.next(),
                message: self.current_message.clone(),
                delta: StreamDelta::tool_use_input(input.tool_call_id, input.delta),
            },
        ])
    }

    fn handle_tool_use_complete(
        &mut self,
        tool: &crate::parser::ClaudeToolUseCompleteEvent,
    ) -> Result<Vec<AgentEvent>, ProviderError> {
        self.deduplicator.reset();
        self.tool_fsm.complete_input(
            &tool.tool_call_id,
            &tool.tool_name,
            &tool.final_input,
        )?;
        let mut tool_call = ToolCall::new(
            tool.tool_call_id.clone(),
            self.provider.canonical_tool_name(&tool.tool_name),
            parse_json_or_string(&tool.final_input),
        );
        if let Some(parent_tool_call_id) = &tool.parent_tool_call_id {
            tool_call = tool_call.with_parent_id(parent_tool_call_id.clone());
        }
        append_tool_use_block(&mut self.current_message, &tool_call);

        Ok(vec![AgentEvent::MessageUpdate {
            meta: self.emitter.next(),
            message: self.current_message.clone(),
            delta: StreamDelta::tool_use(tool_call),
        }])
    }

    fn handle_tool_progress(
        &mut self,
        progress: crate::parser::ClaudeToolProgressEvent,
    ) -> Vec<AgentEvent> {
        vec![AgentEvent::ToolExecutionUpdate {
            meta: self.emitter.next(),
            tool_call_id: progress.tool_call_id,
            tool_name: self.provider.canonical_tool_name(&progress.tool_name),
            partial_result: json!({
                "progress": progress.progress_text,
            }),
        }]
    }

    fn handle_tool_result(
        &mut self,
        tool: crate::parser::ClaudeToolResultEvent,
    ) -> Result<Vec<AgentEvent>, ProviderError> {
        self.deduplicator.reset();
        self.tool_fsm.result(&tool.tool_call_id, !tool.is_error)?;
        let canonical_tool_name = self.provider.canonical_tool_name(&tool.tool_name);
        let result_json = if let Some(parent_tool_call_id) = &tool.parent_tool_call_id {
            self.nested_tools
                .register_result(parent_tool_call_id, &tool);
            tool.result_json.clone()
        } else {
            self.nested_tools
                .merge_into_parent_result(&tool.tool_call_id, tool.result_json.clone())
        };

        let result = if tool.is_error {
            ToolResult::failure(
                tool.tool_call_id.clone(),
                canonical_tool_name.clone(),
                tool.content.clone(),
            )
        } else {
            ToolResult::success(
                tool.tool_call_id.clone(),
                canonical_tool_name.clone(),
                tool.content.clone(),
            )
        };

        if tool.parent_tool_call_id.is_none() {
            self.tool_results.push(result);
        }

        Ok(vec![AgentEvent::ToolExecutionEnd {
            meta: self.emitter.next(),
            tool_call_id: tool.tool_call_id,
            tool_name: canonical_tool_name,
            result: result_json,
            is_error: tool.is_error,
        }])
    }

    fn handle_finish(
        &mut self,
        finish: &crate::parser::ClaudeFinishEvent,
    ) -> Vec<AgentEvent> {
        let mut emitted = Vec::new();
        self.deduplicator.reset();
        self.finished = true;
        self.terminal_error = classify_terminal_error(finish, &self.current_message);
        self.ensure_message_started(&mut emitted);
        emitted.push(AgentEvent::MessageEnd {
            meta: self.emitter.next(),
            message: self.current_message.clone(),
        });
        if self.terminal_error.is_none() {
            emitted.push(AgentEvent::TurnEnd {
                meta: self.emitter.next(),
                message: self.current_message.clone(),
                tool_results: self.tool_results.clone(),
            });
        }
        emitted.push(AgentEvent::Custom {
            meta: self.emitter.next(),
            event_type: "claude_code.finish".to_owned(),
            payload: json!({
                "finish_reason": finish.finish_reason,
                "usage": finish.usage,
                "session_id": finish.session_id,
                "is_error": finish.is_error,
                "result": finish.result,
                "error_code": finish.error_code,
            }),
        });
        emitted
    }
}

fn classify_terminal_error(
    finish: &crate::parser::ClaudeFinishEvent,
    message: &Message,
) -> Option<ProviderError> {
    if !finish.is_error {
        return None;
    }

    let detail = finish
        .result
        .clone()
        .unwrap_or_else(|| assistant_message_text(message));

    match finish.error_code.as_deref() {
        Some("authentication_failed") => Some(ProviderError::auth_failed(detail)),
        Some(error_code) => Some(ProviderError::protocol_violation(
            detail,
            Some(json!({
                "error_code": error_code,
            })),
        )),
        None => Some(ProviderError::protocol_violation(detail, None)),
    }
}

fn assistant_message_text(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::Text { text } = block {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug)]
struct EventEmitter {
    session_id: Option<arky_protocol::SessionId>,
    turn_id: arky_protocol::TurnId,
    provider_id: ProviderId,
    sequence: u64,
}

impl EventEmitter {
    fn new(request: &ProviderRequest, provider_id: ProviderId) -> Self {
        Self {
            session_id: request.session.id.clone(),
            turn_id: request.turn.id.clone(),
            provider_id,
            sequence: 0,
        }
    }

    fn next(&mut self) -> EventMetadata {
        self.sequence = self.sequence.saturating_add(1);
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        let mut metadata = EventMetadata::new(timestamp_ms, self.sequence)
            .with_turn_id(self.turn_id.clone())
            .with_provider_id(self.provider_id.clone());
        if let Some(session_id) = &self.session_id {
            metadata = metadata.with_session_id(session_id.clone());
        }
        metadata
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{
        ClaudeCodeProvider,
        ClaudeCodeProviderConfig,
    };
    use arky_protocol::{
        Message,
        ModelRef,
        SessionRef,
        TurnContext,
        TurnId,
    };
    use arky_provider::ProviderRequest;

    #[tokio::test]
    async fn provider_should_pass_session_id_to_cli_arguments() {
        let provider = ClaudeCodeProvider::with_config(ClaudeCodeProviderConfig {
            binary: "claude".to_owned(),
            ..ClaudeCodeProviderConfig::default()
        });
        let request = ProviderRequest::new(
            SessionRef::new(None).with_provider_session_id("session-123"),
            TurnContext::new(TurnId::new(), 1),
            ModelRef::new("sonnet"),
            vec![Message::user("hello")],
        );

        let config = provider
            .build_process_config(&request)
            .await
            .expect("process config should build");

        assert_eq!(
            config.args.windows(2).any(|window| {
                window == ["--session-id".to_owned(), "session-123".to_owned()]
            }),
            true
        );
    }
}
