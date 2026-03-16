//! Public agent API and actor-backed orchestration runtime.

use std::{
    pin::Pin,
    sync::Arc,
};

use arky_protocol::{
    AgentEvent,
    AgentResponse,
    SessionId,
};
use futures::{
    Stream,
    stream::poll_fn,
};
use tokio::sync::{
    Mutex,
    broadcast,
    mpsc,
    oneshot,
};
use tokio_util::sync::CancellationToken;

use crate::{
    AgentBuilder,
    CoreError,
    queue::CommandQueue,
    replay::{
        SessionState,
        create_session,
        restore_session,
    },
    subscription::EventSubscription,
    turn::{
        TurnControl,
        TurnRunResult,
        TurnRuntime,
        run_turn,
    },
};

/// Stream type returned by [`Agent::stream`].
pub type AgentEventStream =
    Pin<Box<dyn Stream<Item = Result<AgentEvent, CoreError>> + Send>>;

/// High-level orchestration surface for provider-backed agent execution.
#[derive(Clone)]
pub struct Agent {
    inner: Arc<AgentInner>,
}

struct AgentInner {
    queue: CommandQueue<AgentCommand>,
    events: broadcast::Sender<AgentEvent>,
}

struct ActiveTurn {
    cancel: CancellationToken,
    steering_tx: mpsc::UnboundedSender<String>,
    follow_up_tx: mpsc::UnboundedSender<String>,
}

struct ActorState {
    session: Option<SessionState>,
    bootstrap_resume: Option<SessionId>,
    active_turn: Option<ActiveTurn>,
}

enum AgentCommand {
    Prompt {
        input: String,
        response_tx: oneshot::Sender<
            Result<oneshot::Receiver<Result<AgentResponse, CoreError>>, CoreError>,
        >,
    },
    Stream {
        input: String,
        response_tx: oneshot::Sender<Result<AgentEventStream, CoreError>>,
    },
    Steer {
        message: String,
        response_tx: oneshot::Sender<Result<(), CoreError>>,
    },
    FollowUp {
        message: String,
        response_tx: oneshot::Sender<Result<(), CoreError>>,
    },
    NewSession {
        response_tx: oneshot::Sender<Result<SessionId, CoreError>>,
    },
    Resume {
        session_id: SessionId,
        response_tx: oneshot::Sender<Result<(), CoreError>>,
    },
    Abort {
        response_tx: oneshot::Sender<Result<(), CoreError>>,
    },
    CurrentSession {
        response_tx: oneshot::Sender<Option<SessionId>>,
    },
    ClearSession {
        response_tx: oneshot::Sender<Result<(), CoreError>>,
    },
    TurnFinished {
        session: Box<SessionState>,
    },
}

impl Agent {
    pub(crate) fn new(runtime: TurnRuntime, bootstrap_resume: Option<SessionId>) -> Self {
        let events = runtime.events.clone();
        let runtime = Arc::new(runtime);
        let state = Arc::new(Mutex::new(ActorState {
            session: None,
            bootstrap_resume,
            active_turn: None,
        }));
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let queue = CommandQueue::from_sender(sender);
        let worker_queue = queue.clone();

        tokio::spawn({
            let runtime = Arc::clone(&runtime);
            let state = Arc::clone(&state);
            async move {
                while let Some(command) = receiver.recv().await {
                    let mut state = state.lock().await;
                    handle_command(&runtime, &worker_queue, &mut state, command).await;
                }
            }
        });

        Self {
            inner: Arc::new(AgentInner { queue, events }),
        }
    }

    /// Starts building an [`Agent`].
    #[must_use]
    pub fn builder() -> AgentBuilder {
        AgentBuilder::new()
    }

    /// Runs a prompt to completion and returns the aggregated response.
    pub async fn prompt(
        &self,
        input: impl Into<String>,
    ) -> Result<AgentResponse, CoreError> {
        let (tx, rx) = oneshot::channel();
        self.inner.queue.send(AgentCommand::Prompt {
            input: input.into(),
            response_tx: tx,
        })?;
        let response_rx = rx.await.map_err(|_| {
            CoreError::invalid_state("prompt command handler dropped", None)
        })??;

        response_rx.await.map_err(|_| {
            CoreError::invalid_state("prompt completion channel dropped", None)
        })?
    }

    /// Runs a prompt and returns a streaming event view for the run.
    pub async fn stream(
        &self,
        input: impl Into<String>,
    ) -> Result<AgentEventStream, CoreError> {
        let (tx, rx) = oneshot::channel();
        self.inner.queue.send(AgentCommand::Stream {
            input: input.into(),
            response_tx: tx,
        })?;
        rx.await.map_err(|_| {
            CoreError::invalid_state("stream command handler dropped", None)
        })?
    }

    /// Queues mid-run steering for the active turn.
    pub async fn steer(&self, message: impl Into<String>) -> Result<(), CoreError> {
        let (tx, rx) = oneshot::channel();
        self.inner.queue.send(AgentCommand::Steer {
            message: message.into(),
            response_tx: tx,
        })?;
        rx.await.map_err(|_| {
            CoreError::invalid_state("steer command handler dropped", None)
        })?
    }

    /// Queues a follow-up message for the active run.
    pub async fn follow_up(&self, message: impl Into<String>) -> Result<(), CoreError> {
        let (tx, rx) = oneshot::channel();
        self.inner.queue.send(AgentCommand::FollowUp {
            message: message.into(),
            response_tx: tx,
        })?;
        rx.await.map_err(|_| {
            CoreError::invalid_state("follow_up command handler dropped", None)
        })?
    }

    /// Creates a typed event subscription for broadcast agent events.
    #[must_use]
    pub fn subscribe(&self) -> EventSubscription {
        EventSubscription::new(self.inner.events.subscribe())
    }

    /// Creates and activates a fresh session.
    pub async fn new_session(&self) -> Result<SessionId, CoreError> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .queue
            .send(AgentCommand::NewSession { response_tx: tx })?;
        rx.await.map_err(|_| {
            CoreError::invalid_state("new_session command handler dropped", None)
        })?
    }

    /// Resumes the provided persisted session.
    pub async fn resume(&self, session_id: SessionId) -> Result<(), CoreError> {
        let (tx, rx) = oneshot::channel();
        self.inner.queue.send(AgentCommand::Resume {
            session_id,
            response_tx: tx,
        })?;
        rx.await.map_err(|_| {
            CoreError::invalid_state("resume command handler dropped", None)
        })?
    }

    /// Cancels the active turn.
    pub async fn abort(&self) -> Result<(), CoreError> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .queue
            .send(AgentCommand::Abort { response_tx: tx })?;
        rx.await.map_err(|_| {
            CoreError::invalid_state("abort command handler dropped", None)
        })?
    }

    /// Returns the current session identifier when one is active.
    pub async fn current_session_id(&self) -> Option<SessionId> {
        let (tx, rx) = oneshot::channel();
        if self
            .inner
            .queue
            .send(AgentCommand::CurrentSession { response_tx: tx })
            .is_err()
        {
            return None;
        }

        rx.await.ok().flatten()
    }

    /// Clears the active session when the agent is idle.
    pub async fn clear_session(&self) -> Result<(), CoreError> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .queue
            .send(AgentCommand::ClearSession { response_tx: tx })?;
        rx.await.map_err(|_| {
            CoreError::invalid_state("clear_session command handler dropped", None)
        })?
    }
}

async fn handle_command(
    runtime: &Arc<TurnRuntime>,
    queue: &CommandQueue<AgentCommand>,
    state: &mut ActorState,
    command: AgentCommand,
) {
    match command {
        AgentCommand::Prompt { input, response_tx } => {
            handle_prompt(runtime, queue, state, input, response_tx).await;
        }
        AgentCommand::Stream { input, response_tx } => {
            handle_stream(runtime, queue, state, input, response_tx).await;
        }
        AgentCommand::Steer {
            message,
            response_tx,
        } => handle_steer(state, message, response_tx),
        AgentCommand::FollowUp {
            message,
            response_tx,
        } => handle_follow_up(runtime, queue, state, message, response_tx).await,
        AgentCommand::NewSession { response_tx } => {
            handle_new_session(runtime, state, response_tx).await;
        }
        AgentCommand::Resume {
            session_id,
            response_tx,
        } => handle_resume(runtime, state, session_id, response_tx).await,
        AgentCommand::Abort { response_tx } => handle_abort(state, response_tx),
        AgentCommand::CurrentSession { response_tx } => {
            let _ = response_tx
                .send(state.session.as_ref().map(|session| session.id.clone()));
        }
        AgentCommand::ClearSession { response_tx } => {
            handle_clear_session(state, response_tx);
        }
        AgentCommand::TurnFinished { session } => {
            state.session = Some(*session);
            state.active_turn = None;
        }
    }
}

async fn handle_prompt(
    runtime: &Arc<TurnRuntime>,
    queue: &CommandQueue<AgentCommand>,
    state: &mut ActorState,
    input: String,
    response_tx: oneshot::Sender<
        Result<oneshot::Receiver<Result<AgentResponse, CoreError>>, CoreError>,
    >,
) {
    let result = start_turn(runtime, queue, state, input, false, "prompt")
        .await
        .map(|(_, response_rx)| response_rx);
    let _ = response_tx.send(result);
}

async fn handle_stream(
    runtime: &Arc<TurnRuntime>,
    queue: &CommandQueue<AgentCommand>,
    state: &mut ActorState,
    input: String,
    response_tx: oneshot::Sender<Result<AgentEventStream, CoreError>>,
) {
    let result = start_turn(runtime, queue, state, input, true, "stream")
        .await
        .map(|(stream, _)| stream);
    let _ = response_tx.send(result);
}

fn handle_steer(
    state: &ActorState,
    message: String,
    response_tx: oneshot::Sender<Result<(), CoreError>>,
) {
    let result = state
        .active_turn
        .as_ref()
        .ok_or_else(|| {
            CoreError::invalid_state("cannot steer without an active turn", None)
        })
        .and_then(|active| {
            active.steering_tx.send(message).map_err(|_| {
                CoreError::invalid_state("failed to queue steering message", None)
            })
        });
    let _ = response_tx.send(result);
}

async fn handle_follow_up(
    runtime: &Arc<TurnRuntime>,
    queue: &CommandQueue<AgentCommand>,
    state: &mut ActorState,
    message: String,
    response_tx: oneshot::Sender<Result<(), CoreError>>,
) {
    let result = if let Some(active) = state.active_turn.as_ref() {
        active.follow_up_tx.send(message).map_err(|_| {
            CoreError::invalid_state("failed to queue follow_up message", None)
        })
    } else if state.session.is_some() || state.bootstrap_resume.is_some() {
        start_turn(runtime, queue, state, message, false, "follow_up")
            .await
            .map(|_| ())
    } else {
        Err(CoreError::invalid_state(
            "cannot queue follow_up without an initialized session",
            None,
        ))
    };
    let _ = response_tx.send(result);
}

async fn handle_new_session(
    runtime: &Arc<TurnRuntime>,
    state: &mut ActorState,
    response_tx: oneshot::Sender<Result<SessionId, CoreError>>,
) {
    let result = if state.active_turn.is_some() {
        current_busy_error(state, "new_session")
    } else {
        create_session(
            runtime.session_store.as_ref(),
            runtime.hooks.as_ref(),
            &runtime.model,
            &runtime.provider_settings,
        )
        .await
        .map(|session| {
            let session_id = session.id.clone();
            state.session = Some(session);
            state.bootstrap_resume = None;
            session_id
        })
    };
    let _ = response_tx.send(result);
}

async fn handle_resume(
    runtime: &Arc<TurnRuntime>,
    state: &mut ActorState,
    session_id: SessionId,
    response_tx: oneshot::Sender<Result<(), CoreError>>,
) {
    let result = if state.active_turn.is_some() {
        current_busy_error(state, "resume")
    } else {
        restore_session(
            runtime.session_store.as_ref(),
            runtime.hooks.as_ref(),
            &session_id,
            &runtime.model,
            &runtime.provider_settings,
        )
        .await
        .map(|session| {
            state.session = Some(session);
            state.bootstrap_resume = None;
        })
    };
    let _ = response_tx.send(result);
}

fn handle_abort(state: &ActorState, response_tx: oneshot::Sender<Result<(), CoreError>>) {
    let result = state
        .active_turn
        .as_ref()
        .ok_or_else(|| {
            CoreError::invalid_state("cannot abort without an active turn", None)
        })
        .map(|active| active.cancel.cancel());
    let _ = response_tx.send(result);
}

fn handle_clear_session(
    state: &mut ActorState,
    response_tx: oneshot::Sender<Result<(), CoreError>>,
) {
    let result = if state.active_turn.is_some() {
        current_busy_error(state, "clear_session")
    } else {
        state.session = None;
        state.bootstrap_resume = None;
        Ok(())
    };
    let _ = response_tx.send(result);
}

async fn start_turn(
    runtime: &Arc<TurnRuntime>,
    queue: &CommandQueue<AgentCommand>,
    state: &mut ActorState,
    input: String,
    expose_stream: bool,
    operation: &'static str,
) -> Result<
    (
        AgentEventStream,
        oneshot::Receiver<Result<AgentResponse, CoreError>>,
    ),
    CoreError,
> {
    if state.active_turn.is_some() {
        return current_busy_error(state, operation);
    }

    ensure_session(runtime, state).await?;
    let session = state
        .session
        .clone()
        .ok_or_else(|| CoreError::invalid_state("session state is unavailable", None))?;
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (response_tx, response_rx) = oneshot::channel();
    let (steering_tx, steering_rx) = mpsc::unbounded_channel();
    let (follow_up_tx, follow_up_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    state.active_turn = Some(ActiveTurn {
        cancel: cancel.clone(),
        steering_tx,
        follow_up_tx,
    });

    let stream = receiver_stream(if expose_stream { Some(event_rx) } else { None });
    let runtime = Arc::clone(runtime);
    let queue = queue.clone();
    tokio::spawn(async move {
        let TurnRunResult { session, response } = run_turn(
            runtime,
            session,
            input,
            Some(event_tx),
            TurnControl {
                cancel,
                steering_rx,
                follow_up_rx,
            },
        )
        .await;
        let _ = response_tx.send(response);
        let _ = queue.send(AgentCommand::TurnFinished {
            session: Box::new(session),
        });
    });

    Ok((stream, response_rx))
}

async fn ensure_session(
    runtime: &Arc<TurnRuntime>,
    state: &mut ActorState,
) -> Result<(), CoreError> {
    if state.session.is_some() {
        return Ok(());
    }

    if let Some(session_id) = state.bootstrap_resume.take() {
        state.session = Some(
            restore_session(
                runtime.session_store.as_ref(),
                runtime.hooks.as_ref(),
                &session_id,
                &runtime.model,
                &runtime.provider_settings,
            )
            .await?,
        );
        return Ok(());
    }

    state.session = Some(
        create_session(
            runtime.session_store.as_ref(),
            runtime.hooks.as_ref(),
            &runtime.model,
            &runtime.provider_settings,
        )
        .await?,
    );
    Ok(())
}

fn current_busy_error<T>(
    state: &ActorState,
    operation: &'static str,
) -> Result<T, CoreError> {
    let session_id = state
        .session
        .as_ref()
        .map(|session| session.id.clone())
        .unwrap_or_default();
    Err(CoreError::busy_session(session_id, operation))
}

fn receiver_stream(
    receiver: Option<mpsc::UnboundedReceiver<Result<AgentEvent, CoreError>>>,
) -> AgentEventStream {
    match receiver {
        Some(mut receiver) => Box::pin(poll_fn(move |cx| receiver.poll_recv(cx))),
        None => Box::pin(futures::stream::empty()),
    }
}
