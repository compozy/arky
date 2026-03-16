//! Codex provider orchestration over the shared subprocess layer.

use std::{
    collections::{
        BTreeMap,
        HashMap,
    },
    path::PathBuf,
    sync::Arc,
    time::{
        Duration,
        SystemTime,
        UNIX_EPOCH,
    },
};

use arky_protocol::{
    AgentEvent,
    ContentBlock,
    EventMetadata,
    ProviderId,
    Role,
    StreamDelta,
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
};
use arky_tools::{
    ToolIdCodec,
    create_codex_tool_id_codec,
};
use async_stream::try_stream;
use futures::StreamExt;
use serde_json::{
    Map,
    Value,
    json,
};
use tokio::{
    io::AsyncReadExt,
    sync::Mutex,
};

use crate::{
    ApprovalHandler,
    ApprovalMode,
    NotificationRouter,
    RpcTransport,
    RpcTransportConfig,
    Scheduler,
    TextAccumulator,
    ThreadManager,
    ThreadOpenParams,
    TurnStartParams,
    accumulator::ToolTracker,
    rpc::{
        InitializeCapabilities,
        InitializeParams,
    },
};

#[derive(Debug, Clone)]
struct ResolvedCommand {
    program: String,
    prefix_args: Vec<String>,
}

impl ResolvedCommand {
    fn args_with(&self, extra: &[String]) -> Vec<String> {
        let mut args = self.prefix_args.clone();
        args.extend(extra.iter().cloned());
        args
    }
}

/// Runtime configuration for the Codex provider.
#[derive(Debug, Clone)]
pub struct CodexProviderConfig {
    /// Preferred binary path or command name.
    pub binary: String,
    /// Whether `npx -y @openai/codex` may be used as a fallback.
    pub allow_npx: bool,
    /// Optional working directory for spawned processes.
    pub cwd: Option<PathBuf>,
    /// Environment overrides applied to every subprocess.
    pub env: BTreeMap<String, String>,
    /// Arguments used when validating the binary.
    pub version_args: Vec<String>,
    /// Arguments used to launch the app-server.
    pub app_server_args: Vec<String>,
    /// Per-request JSON-RPC timeout.
    pub request_timeout: Duration,
    /// Scheduler acquire timeout.
    pub scheduler_timeout: Duration,
    /// Approval behavior for server-initiated requests.
    pub approval_mode: ApprovalMode,
    /// Approval policy sent with `turn/start`.
    pub approval_policy: Option<String>,
    /// Client identity used in the initialize handshake.
    pub client_name: String,
    /// Client version used in the initialize handshake.
    pub client_version: String,
    /// Whether to enable experimental app-server APIs.
    pub experimental_api: bool,
}

impl Default for CodexProviderConfig {
    fn default() -> Self {
        Self {
            binary: "codex".to_owned(),
            allow_npx: true,
            cwd: None,
            env: BTreeMap::new(),
            version_args: vec!["--version".to_owned()],
            app_server_args: vec![
                "app-server".to_owned(),
                "--listen".to_owned(),
                "stdio://".to_owned(),
            ],
            request_timeout: Duration::from_secs(30),
            scheduler_timeout: Duration::from_secs(300),
            approval_mode: ApprovalMode::AutoApprove,
            approval_policy: Some("never".to_owned()),
            client_name: "arky-codex".to_owned(),
            client_version: env!("CARGO_PKG_VERSION").to_owned(),
            experimental_api: false,
        }
    }
}

/// Concrete `Provider` implementation backed by the Codex app-server.
#[derive(Clone)]
pub struct CodexProvider {
    descriptor: ProviderDescriptor,
    config: CodexProviderConfig,
    scheduler: Scheduler,
    sessions: Arc<Mutex<HashMap<String, String>>>,
    codec: Arc<dyn ToolIdCodec>,
    resolved_command: Arc<Mutex<Option<ResolvedCommand>>>,
}

impl CodexProvider {
    /// Creates a provider with the default runtime configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(CodexProviderConfig::default())
    }

    /// Creates a provider with an explicit configuration.
    #[must_use]
    pub fn with_config(config: CodexProviderConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::new(
                ProviderId::new("codex"),
                ProviderFamily::Codex,
                ProviderCapabilities::new()
                    .with_streaming(true)
                    .with_generate(true)
                    .with_tool_calls(true)
                    .with_mcp_passthrough(true)
                    .with_session_resume(true)
                    .with_steering(true)
                    .with_follow_up(true),
            ),
            scheduler: Scheduler::new(config.scheduler_timeout),
            config,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            codec: Arc::new(create_codex_tool_id_codec()),
            resolved_command: Arc::new(Mutex::new(None)),
        }
    }

    async fn ensure_binary_validated(&self) -> Result<ResolvedCommand, ProviderError> {
        let cached = self.resolved_command.lock().await.clone();
        if let Some(resolved) = cached {
            return Ok(resolved);
        }

        let candidates = command_candidates(&self.config);
        let mut last_error = None;

        for candidate in candidates {
            match self.validate_candidate(&candidate).await {
                Ok(version) => {
                    let resolved = ResolvedCommand {
                        program: candidate.program,
                        prefix_args: candidate.prefix_args,
                    };
                    tracing::debug!(version = %version, program = %resolved.program, "validated codex command");
                    *self.resolved_command.lock().await = Some(resolved.clone());
                    return Ok(resolved);
                }
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ProviderError::binary_not_found(self.config.binary.clone())
        }))
    }

    async fn validate_candidate(
        &self,
        candidate: &CommandCandidate,
    ) -> Result<String, ProviderError> {
        let manager = ProcessManager::new(self.version_process_config(candidate));
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
                "failed to read codex version output: {error}"
            ))
        })?;
        process.wait().await?;

        let version = if stdout_buffer.trim().is_empty() {
            stderr_buffer.trim()
        } else {
            stdout_buffer.trim()
        };
        if version.is_empty() {
            return Err(ProviderError::protocol_violation(
                "codex version command returned no output",
                Some(json!({
                    "stderr": stderr_buffer.trim(),
                })),
            ));
        }

        Ok(version.to_owned())
    }

    fn version_process_config(&self, candidate: &CommandCandidate) -> ProcessConfig {
        let mut config = ProcessConfig::new(&candidate.program)
            .with_args(candidate.args_with(&self.config.version_args))
            .with_kill_on_drop(true);
        config = apply_process_overrides(config, &self.config);
        config
    }

    fn app_server_process_config(&self, resolved: &ResolvedCommand) -> ProcessConfig {
        let mut config = ProcessConfig::new(&resolved.program)
            .with_args(resolved.args_with(&self.config.app_server_args))
            .with_kill_on_drop(true);
        config = apply_process_overrides(config, &self.config);
        config
    }

    fn app_server_command_label(&self, resolved: &ResolvedCommand) -> String {
        let args = resolved.args_with(&self.config.app_server_args);
        if args.is_empty() {
            return resolved.program.clone();
        }

        format!("{} {}", resolved.program, args.join(" "))
    }

    async fn resolve_thread_id(&self, request: &ProviderRequest) -> Option<String> {
        if let Some(provider_session_id) = &request.session.provider_session_id {
            return Some(provider_session_id.clone());
        }

        let session_key = request.session.id.as_ref()?.to_string();
        self.sessions.lock().await.get(&session_key).cloned()
    }

    async fn remember_thread_id(&self, request: &ProviderRequest, thread_id: &str) {
        if let Some(session_id) = &request.session.id {
            self.sessions
                .lock()
                .await
                .insert(session_id.to_string(), thread_id.to_owned());
        }
    }

    fn initialize_params(&self) -> InitializeParams {
        InitializeParams {
            protocol_version: Some(1),
            client_info: Some(crate::rpc::InitializeClientInfo {
                name: self.config.client_name.clone(),
                title: Some("Arky Codex Provider".to_owned()),
                version: self.config.client_version.clone(),
            }),
            capabilities: Some(InitializeCapabilities {
                experimental_api: Some(self.config.experimental_api),
                opt_out_notification_methods: Vec::new(),
            }),
        }
    }

    fn build_turn_params(
        &self,
        request: &ProviderRequest,
        config_overrides: Option<Map<String, Value>>,
    ) -> TurnStartParams {
        TurnStartParams {
            scope_id: Some(request.turn.id.to_string()),
            prompt: Some(render_prompt(request)),
            input: None,
            model: Some(
                request
                    .model
                    .provider_model_id
                    .clone()
                    .unwrap_or_else(|| request.model.model_id.clone()),
            ),
            config_overrides,
            output_schema: request.settings.extra.get("output_schema").cloned(),
            approval_policy: self.config.approval_policy.clone(),
        }
    }

    fn build_config_overrides(request: &ProviderRequest) -> Option<Map<String, Value>> {
        let mut overrides = Map::new();
        copy_extra_override(
            &request.settings.extra,
            "reasoning",
            "model_reasoning_effort",
            &mut overrides,
        );
        copy_extra_override(
            &request.settings.extra,
            "reasoning_summary",
            "model_reasoning_summary",
            &mut overrides,
        );
        copy_extra_override(
            &request.settings.extra,
            "model_verbosity",
            "model_verbosity",
            &mut overrides,
        );

        if let Some(developer_instructions) = request
            .settings
            .extra
            .get("developer_instructions")
            .cloned()
            .or_else(|| developer_instructions_from_messages(&request.messages))
        {
            overrides.insert("developer_instructions".to_owned(), developer_instructions);
        }
        if !request.tools.definitions.is_empty() {
            overrides.insert(
                "tools.available".to_owned(),
                Value::Array(
                    request
                        .tools
                        .definitions
                        .iter()
                        .map(tool_definition_to_value)
                        .collect(),
                ),
            );
        }
        if let Some(tool_choice) = request
            .settings
            .extra
            .get("tools.choice")
            .cloned()
            .or_else(|| request.settings.extra.get("tool_choice").cloned())
        {
            overrides.insert("tools.choice".to_owned(), tool_choice);
        }
        if let Some(config_override_object) = request
            .settings
            .extra
            .get("config_overrides")
            .and_then(Value::as_object)
        {
            for (key, value) in config_override_object {
                overrides.insert(key.clone(), value.clone());
            }
        }
        if let Some(mcp_servers) = request
            .settings
            .extra
            .get("mcp_servers")
            .and_then(Value::as_object)
        {
            flatten_override_object("mcp_servers", mcp_servers, &mut overrides);
        }
        if let Some(rmcp_client) = request
            .settings
            .extra
            .get("rmcp_client")
            .cloned()
            .or_else(|| request.settings.extra.get("rmcpClient").cloned())
        {
            overrides.insert("features.rmcp_client".to_owned(), rmcp_client);
        }

        (!overrides.is_empty()).then_some(overrides)
    }

    fn build_stream(
        &self,
        request: ProviderRequest,
        mut process: ManagedProcess,
        rpc: Arc<RpcTransport>,
        resolved: &ResolvedCommand,
        _permit: crate::SchedulerPermit,
    ) -> Result<ProviderEventStream, ProviderError> {
        let stderr = process.take_stderr()?;
        let provider = self.clone();
        let descriptor = self.descriptor.clone();
        let command_label = self.app_server_command_label(resolved);

        let stream: ProviderEventStream = Box::pin(try_stream! {
            let mut stderr = stderr;
            let mut stderr_task = Some(tokio::spawn(async move {
                let mut buffer = String::new();
                let _ = stderr.read_to_string(&mut buffer).await;
                buffer
            }));

            let router = NotificationRouter::new();
            let notifications = rpc.take_notifications().ok_or_else(|| {
                ProviderError::protocol_violation(
                    "rpc notification receiver has already been taken",
                    None,
                )
            })?;
            let server_requests = rpc.take_server_requests().ok_or_else(|| {
                ProviderError::protocol_violation(
                    "rpc server-request receiver has already been taken",
                    None,
                )
            })?;

            rpc.initialize(provider.initialize_params()).await?;
            let approval_handler = ApprovalHandler::new(provider.config.approval_mode.clone());
            let notifications_worker =
                spawn_notification_worker(router.clone(), notifications);
            let approval_worker = spawn_approval_worker(
                router.clone(),
                approval_handler,
                rpc.clone(),
                server_requests,
            );
            let config_overrides = Self::build_config_overrides(&request);
            let turn_params =
                provider.build_turn_params(&request, config_overrides.clone());
            let (threads, mut turn_stream) = open_turn_stream(
                &provider,
                &request,
                rpc.clone(),
                router.clone(),
                config_overrides,
                turn_params,
            )
            .await?;
            let mut runtime =
                StreamRuntime::new(&request, descriptor.id.clone(), provider.codec.clone());

            yield runtime.turn_start();

            while let Some(item) = turn_stream.next().await {
                let notification = match item {
                    Ok(notification) => notification,
                    Err(error) => {
                        let mapped_error = map_stream_error(
                            error,
                            &mut stderr_task,
                            &mut process,
                            &command_label,
                        )
                        .await?;
                        Err(mapped_error)?;
                        unreachable!("error branch should have returned from the stream")
                    }
                };
                let events = runtime.handle_notification(&notification)?;
                for event in events {
                    yield event;
                }
                if runtime.finished() {
                    break;
                }
            }

            for event in runtime.finalize_open_state() {
                yield event;
            }

            notifications_worker.abort();
            approval_worker.abort();
            let _ = notifications_worker.await;
            let _ = approval_worker.await;
            drop(turn_stream);
            drop(threads);
            drop(rpc);
            finalize_process(
                &mut process,
                &mut stderr_task,
                &command_label,
                runtime.finished(),
            )
            .await?;
        });

        Ok(stream)
    }
}

#[async_trait::async_trait]
impl Provider for CodexProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn stream(
        &self,
        request: ProviderRequest,
    ) -> Result<ProviderEventStream, ProviderError> {
        let resolved = self.ensure_binary_validated().await?;
        let permit = self.scheduler.acquire("codex stream").await?;
        let manager = ProcessManager::new(self.app_server_process_config(&resolved));
        let mut process = manager.spawn()?;
        let stdout = process.take_stdout()?;
        let stdin = process.take_stdin()?;
        let rpc = Arc::new(RpcTransport::new(
            stdout,
            stdin,
            RpcTransportConfig {
                request_timeout: self.config.request_timeout,
                ..RpcTransportConfig::default()
            },
        ));

        self.build_stream(request, process, rpc, &resolved, permit)
    }
}

impl Default for CodexProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct CommandCandidate {
    program: String,
    prefix_args: Vec<String>,
}

impl CommandCandidate {
    fn args_with(&self, extra: &[String]) -> Vec<String> {
        let mut args = self.prefix_args.clone();
        args.extend(extra.iter().cloned());
        args
    }
}

fn command_candidates(config: &CodexProviderConfig) -> Vec<CommandCandidate> {
    let mut candidates = vec![resolve_command_candidate(&config.binary)];
    if config.allow_npx && config.binary != "npx" {
        candidates.push(CommandCandidate {
            program: "npx".to_owned(),
            prefix_args: vec!["-y".to_owned(), "@openai/codex".to_owned()],
        });
    }
    candidates
}

fn resolve_command_candidate(binary: &str) -> CommandCandidate {
    if has_javascript_extension(binary) {
        return CommandCandidate {
            program: "node".to_owned(),
            prefix_args: vec![binary.to_owned()],
        };
    }

    CommandCandidate {
        program: binary.to_owned(),
        prefix_args: Vec::new(),
    }
}

fn has_javascript_extension(binary: &str) -> bool {
    std::path::Path::new(binary)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "js" | "mjs" | "cjs"
            )
        })
}

fn apply_process_overrides(
    mut config: ProcessConfig,
    provider_config: &CodexProviderConfig,
) -> ProcessConfig {
    if let Some(cwd) = &provider_config.cwd {
        config = config.with_cwd(cwd.clone());
    }

    if !provider_config.env.contains_key("RUST_LOG") {
        config = config.with_env("RUST_LOG", "error");
    }

    for (key, value) in &provider_config.env {
        config = config.with_env(key.clone(), value.clone());
    }

    config
}

fn copy_extra_override(
    extra: &std::collections::BTreeMap<String, Value>,
    source_key: &str,
    target_key: &str,
    overrides: &mut Map<String, Value>,
) {
    if let Some(value) = extra.get(source_key).cloned() {
        overrides.insert(target_key.to_owned(), value);
    }
}

fn developer_instructions_from_messages(
    messages: &[arky_protocol::Message],
) -> Option<Value> {
    let instructions = messages
        .iter()
        .filter(|message| matches!(message.role, Role::System))
        .map(message_text)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if instructions.is_empty() {
        return None;
    }

    Some(Value::String(instructions))
}

fn flatten_override_object(
    prefix: &str,
    object: &Map<String, Value>,
    overrides: &mut Map<String, Value>,
) {
    for (key, value) in object {
        let dotted_key = format!("{prefix}.{key}");
        match value {
            Value::Object(nested) => {
                flatten_override_object(&dotted_key, nested, overrides);
            }
            other => {
                overrides.insert(dotted_key, other.clone());
            }
        }
    }
}

fn tool_definition_to_value(definition: &arky_protocol::ToolDefinition) -> Value {
    json!({
        "name": definition.name,
        "description": definition.description,
        "input_schema": definition.input_schema,
    })
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
            .map(content_block_text)
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

fn message_text(message: &arky_protocol::Message) -> String {
    message
        .content
        .iter()
        .map(content_block_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn content_block_text(block: &ContentBlock) -> String {
    match block {
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
    }
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

fn tool_result_to_value(result: &ToolResult) -> Value {
    let content = result
        .content
        .iter()
        .map(|item| match item {
            ToolContent::Text { text } => Value::String(text.clone()),
            ToolContent::Json { value } => value.clone(),
            ToolContent::Image { media_type, .. } => {
                Value::String(format!("image:{media_type}"))
            }
        })
        .collect::<Vec<_>>();

    json!({
        "id": result.id,
        "name": result.name,
        "content": content,
        "isError": result.is_error,
    })
}

fn spawn_notification_worker(
    notification_router: NotificationRouter,
    mut notifications: tokio::sync::mpsc::UnboundedReceiver<
        Result<crate::CodexNotification, ProviderError>,
    >,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(item) = notifications.recv().await {
            match item {
                Ok(notification) => {
                    if let Err(error) = notification_router.dispatch(notification).await {
                        notification_router.error_all(error).await;
                        break;
                    }
                }
                Err(error) => {
                    notification_router.error_all(error).await;
                    break;
                }
            }
        }
    })
}

fn spawn_approval_worker(
    approval_router: NotificationRouter,
    approval_handler: ApprovalHandler,
    approval_transport: Arc<RpcTransport>,
    mut server_requests: tokio::sync::mpsc::UnboundedReceiver<
        Result<crate::CodexServerRequest, ProviderError>,
    >,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(item) = server_requests.recv().await {
            match item {
                Ok(request) => {
                    if let Err(error) = approval_handler
                        .handle(request, approval_transport.as_ref())
                        .await
                    {
                        approval_router.error_all(error).await;
                        break;
                    }
                }
                Err(error) => {
                    approval_router.error_all(error).await;
                    break;
                }
            }
        }
    })
}

async fn open_turn_stream(
    provider: &CodexProvider,
    request: &ProviderRequest,
    rpc: Arc<RpcTransport>,
    router: NotificationRouter,
    config_overrides: Option<Map<String, Value>>,
    turn_params: TurnStartParams,
) -> Result<(ThreadManager<RpcTransport>, crate::TurnNotificationStream), ProviderError> {
    let threads = ThreadManager::new(rpc, router);
    let model = request
        .model
        .provider_model_id
        .clone()
        .unwrap_or_else(|| request.model.model_id.clone());
    let thread_id = if let Some(thread_id) = provider.resolve_thread_id(request).await {
        threads
            .resume_thread(
                &thread_id,
                ThreadOpenParams {
                    model: Some(model.clone()),
                    config_overrides: config_overrides.clone(),
                },
            )
            .await?
            .thread_id
    } else {
        threads
            .start_thread(ThreadOpenParams {
                model: Some(model),
                config_overrides,
            })
            .await?
            .thread_id
    };

    provider.remember_thread_id(request, &thread_id).await;
    let turn_stream = threads.start_turn(&thread_id, turn_params).await?;

    Ok((threads, turn_stream))
}

async fn collect_stderr(
    stderr_task: &mut Option<tokio::task::JoinHandle<String>>,
) -> String {
    if let Some(task) = stderr_task.take() {
        task.await.unwrap_or_default()
    } else {
        String::new()
    }
}

async fn map_stream_error(
    error: ProviderError,
    stderr_task: &mut Option<tokio::task::JoinHandle<String>>,
    process: &mut ManagedProcess,
    command_label: &str,
) -> Result<ProviderError, ProviderError> {
    if matches!(error, ProviderError::StreamInterrupted { .. })
        && !process.is_running()?
    {
        let stderr_excerpt = collect_stderr(stderr_task).await;
        return Ok(ProviderError::process_crashed(
            command_label.to_owned(),
            None,
            (!stderr_excerpt.trim().is_empty())
                .then_some(stderr_excerpt.trim().to_owned()),
        ));
    }

    Ok(error)
}

async fn finalize_process(
    process: &mut ManagedProcess,
    stderr_task: &mut Option<tokio::task::JoinHandle<String>>,
    command_label: &str,
    finished: bool,
) -> Result<(), ProviderError> {
    let process_running = process.is_running()?;
    process.graceful_shutdown().await?;

    if finished {
        return Ok(());
    }

    let stderr_excerpt = collect_stderr(stderr_task).await;
    if !process_running {
        return Err(ProviderError::process_crashed(
            command_label.to_owned(),
            None,
            (!stderr_excerpt.trim().is_empty())
                .then_some(stderr_excerpt.trim().to_owned()),
        ));
    }

    Err(ProviderError::protocol_violation(
        "codex stream ended without a terminal turn notification",
        Some(json!({
            "stderr": stderr_excerpt.trim(),
        })),
    ))
}

struct StreamRuntime {
    emitter: EventEmitter,
    text: TextAccumulator,
    tools: ToolTracker,
    tool_results: Vec<ToolResult>,
    message_started: bool,
    message_finished: bool,
    finished: bool,
    codec: Arc<dyn ToolIdCodec>,
}

impl std::fmt::Debug for StreamRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamRuntime")
            .field("emitter", &self.emitter)
            .field("text", &self.text)
            .field("tools", &self.tools)
            .field("tool_results", &self.tool_results)
            .field("message_started", &self.message_started)
            .field("message_finished", &self.message_finished)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl StreamRuntime {
    fn new(
        request: &ProviderRequest,
        provider_id: ProviderId,
        codec: Arc<dyn ToolIdCodec>,
    ) -> Self {
        Self {
            emitter: EventEmitter::new(request, provider_id),
            text: TextAccumulator::new(),
            tools: ToolTracker::new(),
            tool_results: Vec::new(),
            message_started: false,
            message_finished: false,
            finished: false,
            codec,
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

    fn finalize_open_state(&mut self) -> Vec<AgentEvent> {
        if self.finished {
            return Vec::new();
        }

        let mut events = Vec::new();
        for result in self.tools.fail_open_tools() {
            events.push(AgentEvent::ToolExecutionEnd {
                meta: self.emitter.next(),
                tool_call_id: result.id.clone(),
                tool_name: result.name.clone(),
                result: tool_result_to_value(&result),
                is_error: true,
            });
            self.tool_results.push(result);
        }
        if self.message_started && !self.message_finished {
            events.push(AgentEvent::MessageEnd {
                meta: self.emitter.next(),
                message: self.text.message(),
            });
            self.message_finished = true;
        }
        events
    }

    fn handle_notification(
        &mut self,
        notification: &crate::CodexNotification,
    ) -> Result<Vec<AgentEvent>, ProviderError> {
        match normalize_notification(notification, self.codec.as_ref()) {
            NormalizedNotification::Ignored => Ok(Vec::new()),
            NormalizedNotification::MessageStart { snapshot } => {
                Ok(self.handle_message_start(snapshot))
            }
            NormalizedNotification::MessageDelta { delta } => {
                Ok(self.handle_message_delta(delta))
            }
            NormalizedNotification::MessageComplete { snapshot } => {
                Ok(self.handle_message_complete(snapshot))
            }
            NormalizedNotification::ToolStart {
                call_id,
                tool_name,
                input,
                parent_id,
            } => Ok(vec![
                self.handle_tool_start(call_id, tool_name, input, parent_id),
            ]),
            NormalizedNotification::ToolUpdate {
                call_id,
                tool_name,
                partial_result,
            } => Ok(vec![self.handle_tool_update(
                &call_id,
                tool_name,
                partial_result,
            )]),
            NormalizedNotification::ToolComplete {
                call_id,
                tool_name,
                result,
                is_error,
            } => {
                Ok(vec![self.handle_tool_complete(
                    &call_id, tool_name, result, is_error,
                )])
            }
            NormalizedNotification::TurnCompleted => Ok(self.handle_turn_completed()),
            NormalizedNotification::TurnFailed { message } => {
                Err(ProviderError::protocol_violation(message, None))
            }
        }
    }

    fn handle_message_start(&mut self, snapshot: Option<String>) -> Vec<AgentEvent> {
        if let Some(snapshot) = snapshot {
            self.text.apply_snapshot(&snapshot);
        }
        if self.message_started {
            return Vec::new();
        }

        self.message_started = true;
        vec![AgentEvent::MessageStart {
            meta: self.emitter.next(),
            message: self.text.message(),
        }]
    }

    fn handle_message_delta(&mut self, delta: String) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        if !self.message_started {
            self.message_started = true;
            events.push(AgentEvent::MessageStart {
                meta: self.emitter.next(),
                message: self.text.message(),
            });
        }

        self.text.push_delta(&delta);
        events.push(AgentEvent::MessageUpdate {
            meta: self.emitter.next(),
            message: self.text.message(),
            delta: StreamDelta::text(delta),
        });
        events
    }

    fn handle_message_complete(&mut self, snapshot: Option<String>) -> Vec<AgentEvent> {
        if let Some(snapshot) = snapshot {
            self.text.apply_snapshot(&snapshot);
        }

        let mut events = Vec::new();
        if !self.message_started {
            self.message_started = true;
            events.push(AgentEvent::MessageStart {
                meta: self.emitter.next(),
                message: self.text.message(),
            });
        }
        if !self.message_finished {
            self.message_finished = true;
            events.push(AgentEvent::MessageEnd {
                meta: self.emitter.next(),
                message: self.text.message(),
            });
        }
        events
    }

    fn handle_tool_start(
        &mut self,
        call_id: String,
        tool_name: String,
        input: Value,
        parent_id: Option<String>,
    ) -> AgentEvent {
        let call = self.tools.start(call_id, tool_name, input, parent_id);
        AgentEvent::ToolExecutionStart {
            meta: self.emitter.next(),
            tool_call_id: call.id,
            tool_name: call.name,
            args: call.input,
        }
    }

    fn handle_tool_update(
        &mut self,
        call_id: &str,
        tool_name: String,
        partial_result: Value,
    ) -> AgentEvent {
        self.tools.push_output(
            call_id,
            tool_name.clone(),
            &stringify_value(&partial_result),
        );
        AgentEvent::ToolExecutionUpdate {
            meta: self.emitter.next(),
            tool_call_id: call_id.to_owned(),
            tool_name,
            partial_result,
        }
    }

    fn handle_tool_complete(
        &mut self,
        call_id: &str,
        tool_name: String,
        result: Option<Value>,
        is_error: bool,
    ) -> AgentEvent {
        let completed = self.tools.complete(call_id, tool_name, result, is_error);
        self.tool_results.push(completed.clone());
        AgentEvent::ToolExecutionEnd {
            meta: self.emitter.next(),
            tool_call_id: completed.id.clone(),
            tool_name: completed.name.clone(),
            result: tool_result_to_value(&completed),
            is_error: completed.is_error,
        }
    }

    fn handle_turn_completed(&mut self) -> Vec<AgentEvent> {
        let mut events = self.finalize_open_state();
        events.push(AgentEvent::TurnEnd {
            meta: self.emitter.next(),
            message: self.text.message(),
            tool_results: self.tool_results.clone(),
        });
        self.finished = true;
        events
    }
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
        let mut meta = EventMetadata::new(now_ms(), self.sequence)
            .with_turn_id(self.turn_id.clone())
            .with_provider_id(self.provider_id.clone());
        if let Some(session_id) = &self.session_id {
            meta = meta.with_session_id(session_id.clone());
        }
        meta
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(u64::MAX)
}

#[derive(Debug)]
enum NormalizedNotification {
    Ignored,
    MessageStart {
        snapshot: Option<String>,
    },
    MessageDelta {
        delta: String,
    },
    MessageComplete {
        snapshot: Option<String>,
    },
    ToolStart {
        call_id: String,
        tool_name: String,
        input: Value,
        parent_id: Option<String>,
    },
    ToolUpdate {
        call_id: String,
        tool_name: String,
        partial_result: Value,
    },
    ToolComplete {
        call_id: String,
        tool_name: String,
        result: Option<Value>,
        is_error: bool,
    },
    TurnCompleted,
    TurnFailed {
        message: String,
    },
}

fn normalize_notification(
    notification: &crate::CodexNotification,
    codec: &dyn ToolIdCodec,
) -> NormalizedNotification {
    let method = canonical_method(&notification.method);
    let params = notification.params.as_object();
    let item = params
        .and_then(|params| params.get("item"))
        .and_then(Value::as_object);
    let item_type = item
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
        .map(canonical_item_type);

    match method.as_str() {
        "turn/completed" => NormalizedNotification::TurnCompleted,
        "turn/failed" | "error" => NormalizedNotification::TurnFailed {
            message: extract_error_message(params)
                .unwrap_or_else(|| "Codex turn failed".to_owned()),
        },
        "item/agentmessage/delta" => {
            let Some(delta) = extract_text_delta(params) else {
                return NormalizedNotification::Ignored;
            };
            NormalizedNotification::MessageDelta { delta }
        }
        "item/commandexecution/outputdelta"
        | "item/filechange/outputdelta"
        | "item/mcptoolcall/outputdelta"
        | "item/collabtoolcall/outputdelta" => normalize_tool_update(params, item, codec),
        "item/started" => match item_type.as_deref() {
            Some("agentmessage") => NormalizedNotification::MessageStart {
                snapshot: extract_text_snapshot(item),
            },
            Some(
                "commandexecution" | "filechange" | "mcptoolcall" | "collabtoolcall",
            ) => normalize_tool_start(item, codec),
            _ => NormalizedNotification::Ignored,
        },
        "item/completed" => match item_type.as_deref() {
            Some("agentmessage") => NormalizedNotification::MessageComplete {
                snapshot: extract_text_snapshot(item),
            },
            Some(
                "commandexecution" | "filechange" | "mcptoolcall" | "collabtoolcall",
            ) => normalize_tool_complete(item, codec),
            _ => NormalizedNotification::Ignored,
        },
        _ => NormalizedNotification::Ignored,
    }
}

fn normalize_tool_start(
    item: Option<&Map<String, Value>>,
    codec: &dyn ToolIdCodec,
) -> NormalizedNotification {
    let Some(item) = item else {
        return NormalizedNotification::Ignored;
    };
    let call_id = item
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("tool")
        .to_owned();
    let tool_name = canonical_tool_name(item, codec);
    let input = tool_input(item);
    let parent_id = item
        .get("parentId")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    NormalizedNotification::ToolStart {
        call_id,
        tool_name,
        input,
        parent_id,
    }
}

fn normalize_tool_update(
    params: Option<&Map<String, Value>>,
    item: Option<&Map<String, Value>>,
    codec: &dyn ToolIdCodec,
) -> NormalizedNotification {
    let call_id = params
        .and_then(|params| params.get("itemId"))
        .and_then(Value::as_str)
        .or_else(|| item.and_then(|item| item.get("id")).and_then(Value::as_str))
        .unwrap_or("tool")
        .to_owned();
    let tool_name = tool_name_or_default(item, codec);
    let partial = params
        .and_then(|params| params.get("delta").cloned())
        .or_else(|| params.and_then(|params| params.get("output").cloned()))
        .unwrap_or_else(|| Value::String(String::new()));

    NormalizedNotification::ToolUpdate {
        call_id,
        tool_name,
        partial_result: partial,
    }
}

fn normalize_tool_complete(
    item: Option<&Map<String, Value>>,
    codec: &dyn ToolIdCodec,
) -> NormalizedNotification {
    let Some(item) = item else {
        return NormalizedNotification::Ignored;
    };
    let call_id = item
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("tool")
        .to_owned();
    let tool_name = canonical_tool_name(item, codec);
    let status = item
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let is_error = matches!(status, "failed" | "declined") || item.get("error").is_some();
    let result = item
        .get("result")
        .cloned()
        .or_else(|| item.get("aggregatedOutput").cloned())
        .or_else(|| item.get("changes").cloned());

    NormalizedNotification::ToolComplete {
        call_id,
        tool_name,
        result,
        is_error,
    }
}

fn canonical_method(method: &str) -> String {
    method.to_ascii_lowercase().replace('.', "/")
}

fn canonical_item_type(item_type: &str) -> String {
    item_type
        .chars()
        .filter(|character| *character != '_' && *character != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

fn extract_text_delta(params: Option<&Map<String, Value>>) -> Option<String> {
    params
        .and_then(|params| params.get("delta"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn extract_text_snapshot(item: Option<&Map<String, Value>>) -> Option<String> {
    item.and_then(|item| item.get("text"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn extract_error_message(params: Option<&Map<String, Value>>) -> Option<String> {
    params
        .and_then(|params| params.get("message"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            params
                .and_then(|params| params.get("error"))
                .and_then(Value::as_object)
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            params
                .and_then(|params| params.get("turn"))
                .and_then(Value::as_object)
                .and_then(|turn| turn.get("error"))
                .and_then(Value::as_object)
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

fn canonical_tool_name(item: &Map<String, Value>, codec: &dyn ToolIdCodec) -> String {
    let item_type = item
        .get("type")
        .and_then(Value::as_str)
        .map(canonical_item_type);

    let direct_name = item
        .get("tool")
        .and_then(Value::as_str)
        .or_else(|| item.get("name").and_then(Value::as_str))
        .or_else(|| item.get("command").and_then(Value::as_str));

    if let Some(name) = direct_name {
        if let Ok(decoded) = codec.decode(name) {
            return decoded.canonical_name;
        }
        return name.to_owned();
    }

    match item_type.as_deref() {
        Some("commandexecution") => "command_execution".to_owned(),
        Some("filechange") => "file_change".to_owned(),
        Some("mcptoolcall") => {
            let server = item
                .get("server")
                .and_then(Value::as_str)
                .unwrap_or("server");
            let tool = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
            format!("mcp/{server}/{tool}")
        }
        Some("collabtoolcall") => item
            .get("tool")
            .and_then(Value::as_str)
            .unwrap_or("collab_tool")
            .to_owned(),
        _ => "tool".to_owned(),
    }
}

fn tool_name_or_default(
    item: Option<&Map<String, Value>>,
    codec: &dyn ToolIdCodec,
) -> String {
    let Some(item) = item else {
        return "tool".to_owned();
    };

    canonical_tool_name(item, codec)
}

fn tool_input(item: &Map<String, Value>) -> Value {
    item.get("arguments")
        .cloned()
        .or_else(|| item.get("input").cloned())
        .or_else(|| item.get("command").cloned())
        .unwrap_or_else(|| Value::Object(item.clone()))
}

fn stringify_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use arky_protocol::{
        Message,
        ModelRef,
        SessionRef,
        ToolContext,
        ToolDefinition,
        TurnContext,
        TurnId,
    };
    use arky_provider::Provider;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::{
        CodexProvider,
        NormalizedNotification,
        normalize_notification,
    };

    #[test]
    fn normalize_notification_should_detect_message_and_tool_events() {
        let provider = CodexProvider::new();

        let message_delta = normalize_notification(
            &crate::CodexNotification {
                method: "item/agentMessage/delta".to_owned(),
                params: json!({
                    "delta": "hello",
                }),
            },
            provider.codec.as_ref(),
        );
        let tool_complete = normalize_notification(
            &crate::CodexNotification {
                method: "item/completed".to_owned(),
                params: json!({
                    "item": {
                        "id": "tool-1",
                        "type": "commandExecution",
                        "status": "completed",
                        "aggregatedOutput": "done",
                    },
                }),
            },
            provider.codec.as_ref(),
        );

        assert!(matches!(
            message_delta,
            NormalizedNotification::MessageDelta { delta } if delta == "hello"
        ));
        assert!(matches!(
            tool_complete,
            NormalizedNotification::ToolComplete { call_id, .. } if call_id == "tool-1"
        ));
    }

    #[test]
    fn codex_descriptor_should_expose_expected_capabilities() {
        let provider = CodexProvider::new();
        let descriptor = provider.descriptor();

        assert_eq!(descriptor.id.as_str(), "codex");
        assert_eq!(descriptor.capabilities.streaming, true);
        assert_eq!(descriptor.capabilities.generate, true);
        assert_eq!(descriptor.capabilities.tool_calls, true);
        assert_eq!(descriptor.capabilities.session_resume, true);
    }

    #[test]
    fn build_config_overrides_should_include_tools_developer_text_and_mcp_settings() {
        let mut extra = BTreeMap::new();
        extra.insert("reasoning".to_owned(), json!("high"));
        extra.insert("reasoning_summary".to_owned(), json!("none"));
        extra.insert("tool_choice".to_owned(), json!("auto"));
        extra.insert(
            "mcp_servers".to_owned(),
            json!({
                "runtime": {
                    "url": "http://127.0.0.1:7777/mcp",
                },
            }),
        );

        let mut settings = arky_provider::ProviderSettings::new();
        settings.extra = extra;
        let request = arky_provider::ProviderRequest::new(
            SessionRef::new(None),
            TurnContext::new(TurnId::new(), 1),
            ModelRef::new("gpt-5"),
            vec![Message::system("Use the contract"), Message::user("hello")],
        )
        .with_tools(
            ToolContext::new().with_definitions(vec![ToolDefinition::new(
                "search_docs",
                "Search docs",
                json!({
                    "type": "object",
                }),
            )]),
        )
        .with_settings(settings);

        let overrides = CodexProvider::build_config_overrides(&request)
            .expect("config overrides should exist");

        assert_eq!(overrides["model_reasoning_effort"], "high");
        assert_eq!(overrides["model_reasoning_summary"], "none");
        assert_eq!(overrides["tools.choice"], "auto");
        assert_eq!(overrides["developer_instructions"], "Use the contract");
        assert_eq!(
            overrides["mcp_servers.runtime.url"],
            "http://127.0.0.1:7777/mcp"
        );
        assert_eq!(overrides["tools.available"][0]["name"], "search_docs");
    }
}
