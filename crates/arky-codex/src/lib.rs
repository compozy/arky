//! Codex App Server provider implementation for Arky.
//!
//! This crate speaks newline-delimited JSON-RPC with the Codex app server and
//! exposes the transport, notification routing, approval, and thread/session
//! primitives needed by the higher-level Arky runtime.

mod accumulator;
mod approval;
mod notification;
mod provider;
mod rpc;
mod scheduler;
mod thread;

pub use crate::{
    accumulator::{
        TextAccumulator,
        ToolRuntimeState,
        ToolTracker,
    },
    approval::{
        ApprovalDecision,
        ApprovalHandler,
        ApprovalMode,
        ApprovalRequest,
    },
    notification::{
        CodexNotification,
        NotificationRouter,
        extract_scope_id_from_value,
        extract_thread_id_from_value,
    },
    provider::{
        CodexProvider,
        CodexProviderConfig,
    },
    rpc::{
        CodexServerRequest,
        InitializeCapabilities,
        InitializeClientInfo,
        InitializeParams,
        InitializeResponse,
        JsonRpcErrorObject,
        JsonRpcId,
        RpcTransport,
        RpcTransportConfig,
    },
    scheduler::{
        Scheduler,
        SchedulerPermit,
    },
    thread::{
        ThreadManager,
        ThreadOpenParams,
        ThreadStartResult,
        TurnNotificationStream,
        TurnStartParams,
    },
};
