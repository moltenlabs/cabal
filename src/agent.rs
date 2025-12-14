//! Agent implementation - a single AI worker

use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, info, warn, instrument};

use warhorn::{
    AgentId, AgentRole, AgentStatus, AgentConfig, TaskId,
    Event, SubmissionId, TokenUsage,
};
use trinkets::{ToolRegistry, ToolContext};

use crate::error::GoblinError;

/// A single AI agent worker
pub struct Agent {
    /// Unique identifier
    pub id: AgentId,
    /// Agent role in hierarchy
    pub role: AgentRole,
    /// Current status
    status: RwLock<AgentStatus>,
    /// Configuration
    pub config: AgentConfig,
    /// Parent agent (None for orchestrator)
    pub parent_id: Option<AgentId>,
    /// Children agents (if can spawn)
    children: RwLock<Vec<AgentId>>,
    /// Tool registry available to this agent
    tools: Arc<ToolRegistry>,
    /// Current task being worked on
    current_task: RwLock<Option<TaskId>>,
    /// Token usage
    usage: RwLock<TokenUsage>,
    /// Event sender for reporting back
    event_tx: mpsc::UnboundedSender<Event>,
}

impl Agent {
    /// Create a new agent
    pub fn new(
        config: AgentConfig,
        parent_id: Option<AgentId>,
        tools: Arc<ToolRegistry>,
        event_tx: mpsc::UnboundedSender<Event>,
    ) -> Self {
        let id = AgentId::new();
        
        info!(
            agent_id = %id,
            role = ?config.role,
            parent = ?parent_id,
            "Creating new agent"
        );

        Self {
            id,
            role: config.role.clone(),
            status: RwLock::new(AgentStatus::Spawning),
            config,
            parent_id,
            children: RwLock::new(Vec::new()),
            tools,
            current_task: RwLock::new(None),
            usage: RwLock::new(TokenUsage::default()),
            event_tx,
        }
    }

    /// Get current status
    pub fn status(&self) -> AgentStatus {
        self.status.read().clone()
    }

    /// Set status and emit event
    pub fn set_status(&self, status: AgentStatus, sub_id: &SubmissionId) {
        let mut guard = self.status.write();
        *guard = status.clone();
        drop(guard);

        let _ = self.event_tx.send(Event::AgentStatusChanged {
            sub_id: sub_id.clone(),
            agent_id: self.id,
            status,
        });
    }

    /// Initialize the agent (load context, etc.)
    #[instrument(skip(self))]
    pub async fn initialize(&self, sub_id: &SubmissionId) -> Result<(), GoblinError> {
        debug!(agent_id = %self.id, "Initializing agent");
        
        self.set_status(AgentStatus::Initializing, sub_id);
        
        // TODO: Load context from Grimoire
        // TODO: Initialize model connection
        
        self.set_status(AgentStatus::Running, sub_id);
        
        info!(agent_id = %self.id, "Agent initialized");
        Ok(())
    }

    /// Assign a task to this agent
    pub fn assign_task(&self, task_id: TaskId) {
        let mut guard = self.current_task.write();
        *guard = Some(task_id);
    }

    /// Get current task
    pub fn current_task(&self) -> Option<TaskId> {
        self.current_task.read().clone()
    }

    /// Add a child agent
    pub fn add_child(&self, child_id: AgentId) {
        self.children.write().push(child_id);
    }

    /// Remove a child agent
    pub fn remove_child(&self, child_id: &AgentId) -> bool {
        let mut guard = self.children.write();
        if let Some(pos) = guard.iter().position(|id| id == child_id) {
            guard.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get children IDs
    pub fn children(&self) -> Vec<AgentId> {
        self.children.read().clone()
    }

    /// Check if agent can spawn children
    pub fn can_spawn(&self) -> bool {
        if !self.config.can_spawn {
            return false;
        }
        
        if let Some(max) = self.config.max_children {
            return self.children.read().len() < max;
        }
        
        true
    }

    /// Get tool registry
    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    /// Create tool context for this agent
    pub fn tool_context(&self) -> ToolContext {
        let mut ctx = ToolContext::new(
            self.config.cwd.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        );
        
        ctx = ctx.with_agent(self.id);
        
        if let Some(task_id) = self.current_task() {
            ctx = ctx.with_task(task_id);
        }
        
        ctx
    }

    /// Update token usage
    pub fn add_usage(&self, input: u64, output: u64) {
        let mut guard = self.usage.write();
        guard.input_tokens += input;
        guard.output_tokens += output;
        guard.total_tokens = guard.input_tokens + guard.output_tokens;
    }

    /// Get token usage
    pub fn usage(&self) -> TokenUsage {
        self.usage.read().clone()
    }

    /// Emit a message event
    pub fn emit_message(&self, sub_id: &SubmissionId, content: String, streaming: bool) {
        let _ = self.event_tx.send(Event::AgentMessage {
            sub_id: sub_id.clone(),
            agent_id: self.id,
            content,
            streaming,
            message_type: warhorn::MessageType::Text,
        });
    }

    /// Terminate this agent
    pub fn terminate(&self, sub_id: &SubmissionId, reason: String) {
        self.set_status(AgentStatus::Terminated, sub_id);
        
        let _ = self.event_tx.send(Event::AgentTerminated {
            sub_id: sub_id.clone(),
            agent_id: self.id,
            reason,
        });
    }
}

/// Handle to an agent for external interaction
#[derive(Clone)]
pub struct AgentHandle {
    inner: Arc<Agent>,
}

impl AgentHandle {
    pub fn new(agent: Agent) -> Self {
        Self {
            inner: Arc::new(agent),
        }
    }

    pub fn id(&self) -> AgentId {
        self.inner.id
    }

    pub fn status(&self) -> AgentStatus {
        self.inner.status()
    }

    pub fn role(&self) -> &AgentRole {
        &self.inner.role
    }

    pub fn inner(&self) -> &Agent {
        &self.inner
    }
}

impl std::ops::Deref for AgentHandle {
    type Target = Agent;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_agent() -> (Agent, mpsc::UnboundedReceiver<Event>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let tools = Arc::new(ToolRegistry::new());
        let config = AgentConfig {
            role: AgentRole::Worker,
            can_spawn: false,
            ..Default::default()
        };
        (Agent::new(config, None, tools, tx), rx)
    }

    #[test]
    fn test_agent_creation() {
        let (agent, _rx) = create_test_agent();
        assert_eq!(agent.status(), AgentStatus::Spawning);
        assert!(agent.parent_id.is_none());
    }

    #[test]
    fn test_agent_children() {
        let (agent, _rx) = create_test_agent();
        let child_id = AgentId::new();
        
        agent.add_child(child_id);
        assert_eq!(agent.children().len(), 1);
        
        assert!(agent.remove_child(&child_id));
        assert_eq!(agent.children().len(), 0);
    }
}
