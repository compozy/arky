//! Claude Code CLI provider implementation for Arky.
//!
//! This crate owns the Claude-specific subprocess integration surface:
//! configuration, stream parsing, nested tool tracking, deduplication, and
//! session bookkeeping for the Claude CLI protocol.

mod cooldown;
mod dedup;
mod nested;
mod parser;
mod provider;
mod session;
mod tool_fsm;

pub use crate::{
    cooldown::{
        SpawnAttemptStatus,
        SpawnFailurePolicy,
        SpawnFailureRecord,
        SpawnFailureTracker,
    },
    dedup::TextDeduplicator,
    parser::{
        ClaudeEventParser,
        ClaudeEventSource,
        ClaudeNormalizedEvent,
    },
    provider::{
        ClaudeCodeProvider,
        ClaudeCodeProviderConfig,
    },
    session::SessionManager,
    tool_fsm::{
        ToolLifecycleState,
        ToolLifecycleTracker,
    },
};
