//! # Cabal
//!
//! Hierarchical agent orchestration system - the scheming group.
//!
//! This crate implements the "Goblins" - AI agents that work in a hierarchy
//! to complete complex tasks through divide-and-conquer approaches.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                          ORCHESTRATOR (Level 0)                      │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
//! │  │ Task Planner │  │ Agent Factory│  │ Result Merger│               │
//! │  └──────────────┘  └──────────────┘  └──────────────┘               │
//! └────────────────────────────┬────────────────────────────────────────┘
//!                              │
//!          ┌───────────────────┼───────────────────┐
//!          ▼                   ▼                   ▼
//!   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//!   │ Domain Lead │     │ Domain Lead │     │ Domain Lead │
//!   │  (Level 1)  │     │  (Level 1)  │     │  (Level 1)  │
//!   └──────┬──────┘     └──────┬──────┘     └──────┬──────┘
//!          │                   │                   │
//!     ┌────┴────┐         ┌────┴────┐         ┌────┴────┐
//!     ▼    ▼    ▼         ▼    ▼    ▼         ▼    ▼    ▼
//!   ┌───┐┌───┐┌───┐     ┌───┐┌───┐┌───┐     ┌───┐┌───┐┌───┐
//!   │W1 ││W2 ││W3 │     │W4 ││W5 ││W6 │     │W7 ││W8 ││W9 │
//!   └───┘└───┘└───┘     └───┘└───┘└───┘     └───┘└───┘└───┘
//! ```
//!
//! ## Key Concepts
//!
//! - **Agent**: A single AI worker with its own context and tools
//! - **Hierarchy**: Tree structure of agents (orchestrator → leads → workers)
//! - **Session**: The runtime context for an orchestration session
//! - **Task**: A unit of work assigned to an agent

pub mod agent;
pub mod session;
pub mod orchestrator;
pub mod hierarchy;
pub mod channel;
pub mod error;

pub use agent::{Agent, AgentHandle};
pub use session::{Session, SessionHandle};
pub use orchestrator::Orchestrator;
pub use hierarchy::AgentHierarchy;
pub use channel::{GoblinChannel, ChannelPair};
pub use error::GoblinError;

// Re-export commonly used protocol types
pub use warhorn::{
    AgentId, TaskId, SessionId, CallId,
    AgentRole, AgentStatus, AgentConfig,
    Op, Event,
};
