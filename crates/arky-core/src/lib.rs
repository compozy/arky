//! Agent framework and orchestration loop for Arky.
//!
//! `arky-core` owns the command queue, turn runtime, replay-aware session
//! restore, event fanout, and the public [`crate::Agent`] orchestration API.

mod agent;
mod builder;
mod error;
mod queue;
mod replay;
mod subscription;
mod turn;

pub use crate::{
    agent::{
        Agent,
        AgentEventStream,
    },
    builder::AgentBuilder,
    error::CoreError,
    subscription::EventSubscription,
};
