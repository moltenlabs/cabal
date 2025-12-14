//! Goblin error types

use thiserror::Error;
use warhorn::AgentId;

/// Errors that can occur in the goblin system
#[derive(Debug, Error)]
pub enum GoblinError {
    /// No active session
    #[error("No active session")]
    NoActiveSession,

    /// No orchestrator agent
    #[error("No orchestrator agent in session")]
    NoOrchestrator,

    /// Agent not found
    #[error("Agent not found: {0}")]
    AgentNotFound(AgentId),

    /// Agent spawn denied
    #[error("Spawn denied: {0}")]
    SpawnDenied(String),

    /// Task error
    #[error("Task error: {0}")]
    TaskError(String),

    /// Tool error
    #[error("Tool error: {0}")]
    ToolError(#[from] trinkets::ToolError),

    /// Sandbox error
    #[error("Sandbox error: {0}")]
    SandboxError(#[from] wardstone::SandboxError),

    /// Protocol error
    #[error("Protocol error: {0}")]
    ProtocolError(#[from] warhorn::ProtocolError),

    /// Channel error
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
}
