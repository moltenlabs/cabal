//! Main orchestrator - coordinates agent hierarchy

use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn, error, instrument};

use warhorn::{
    AgentId, TaskId, AgentConfig, AgentRole, SessionId,
    SessionConfig, Op, Event, SubmissionId, TaskContext,
};
use trinkets::ToolRegistry;

use crate::session::{Session, SessionHandle};
use crate::channel::{GoblinChannel, ChannelPair};
use crate::error::GoblinError;

/// The main goblin orchestrator
///
/// Manages sessions and coordinates the agent hierarchy.
pub struct Orchestrator {
    /// Active sessions
    sessions: parking_lot::RwLock<std::collections::HashMap<SessionId, SessionHandle>>,
    /// Tool registry
    tools: Arc<ToolRegistry>,
    /// Channel for receiving operations
    op_rx: mpsc::UnboundedReceiver<Op>,
    /// Channel for sending events
    event_tx: mpsc::UnboundedSender<Event>,
}

impl Orchestrator {
    /// Create a new orchestrator with the given channel pair
    pub fn new(tools: ToolRegistry, channels: ChannelPair) -> Self {
        Self {
            sessions: parking_lot::RwLock::new(std::collections::HashMap::new()),
            tools: Arc::new(tools),
            op_rx: channels.op_rx,
            event_tx: channels.event_tx,
        }
    }

    /// Create an orchestrator and return a channel for communication
    pub fn with_channel(tools: ToolRegistry) -> (Self, GoblinChannel) {
        let (channel, pair) = GoblinChannel::new();
        (Self::new(tools, pair), channel)
    }

    /// Run the orchestrator event loop
    #[instrument(skip(self))]
    pub async fn run(mut self) -> Result<(), GoblinError> {
        info!("Starting goblin orchestrator");

        while let Some(op) = self.op_rx.recv().await {
            if let Err(e) = self.handle_op(op).await {
                error!(error = %e, "Error handling operation");
            }
        }

        info!("Goblin orchestrator stopped");
        Ok(())
    }

    /// Handle a single operation
    async fn handle_op(&mut self, op: Op) -> Result<(), GoblinError> {
        let sub_id = op.sub_id().clone();
        
        match op {
            Op::ConfigureSession { config, .. } => {
                self.configure_session(config, &sub_id).await?;
            }
            Op::UserInput { prompt, context, .. } => {
                self.handle_user_input(&prompt, context, &sub_id).await?;
            }
            Op::Interrupt { task_id, .. } => {
                self.handle_interrupt(task_id, &sub_id).await?;
            }
            Op::SpawnAgent { config, parent_id, task, .. } => {
                self.spawn_agent(config, parent_id, &sub_id).await?;
            }
            Op::TerminateAgent { agent_id, reason, .. } => {
                self.terminate_agent(&agent_id, reason, &sub_id).await?;
            }
            Op::ExecApproval { call_id, approved, .. } => {
                self.handle_exec_approval(call_id, approved, &sub_id).await?;
            }
            _ => {
                debug!(op = ?op, "Unhandled operation");
            }
        }

        Ok(())
    }

    /// Configure or create a session
    async fn configure_session(
        &mut self,
        config: SessionConfig,
        sub_id: &SubmissionId,
    ) -> Result<SessionHandle, GoblinError> {
        let session = Session::new(
            config.clone(),
            Arc::clone(&self.tools),
            self.event_tx.clone(),
        );
        let session_id = session.id;
        let handle = SessionHandle::new(session);

        self.sessions.write().insert(session_id, handle.clone());

        // Create the root orchestrator agent
        let orchestrator_config = AgentConfig {
            role: AgentRole::Orchestrator,
            model: config.model.clone(),
            cwd: config.cwd.clone(),
            can_spawn: true,
            max_children: Some(config.max_parallel_agents),
            ..Default::default()
        };

        handle.spawn_agent(orchestrator_config, None, sub_id)?;

        // Emit configured event
        let _ = self.event_tx.send(Event::SessionConfigured {
            sub_id: sub_id.clone(),
            session_id,
            config,
        });

        info!(session_id = %session_id, "Session configured");
        Ok(handle)
    }

    /// Handle user input - start a new task
    async fn handle_user_input(
        &mut self,
        prompt: &str,
        context: TaskContext,
        sub_id: &SubmissionId,
    ) -> Result<(), GoblinError> {
        // Get the current session (assumes single session for now)
        let session = self.sessions.read().values().next().cloned()
            .ok_or_else(|| GoblinError::NoActiveSession)?;

        // Create task ID
        let task_id = TaskId::new();
        session.set_current_task(Some(task_id));

        // Emit task started
        let _ = self.event_tx.send(Event::TaskStarted {
            sub_id: sub_id.clone(),
            task_id,
            prompt: prompt.to_string(),
        });

        // Get orchestrator agent
        let orchestrator = session.orchestrator()
            .ok_or_else(|| GoblinError::NoOrchestrator)?;

        // TODO: Send prompt to orchestrator agent
        // For now, emit a placeholder message
        orchestrator.emit_message(sub_id, format!("Received task: {}", prompt), false);

        info!(task_id = %task_id, "Started task");
        Ok(())
    }

    /// Handle interrupt
    async fn handle_interrupt(
        &mut self,
        task_id: Option<TaskId>,
        sub_id: &SubmissionId,
    ) -> Result<(), GoblinError> {
        // Get the current session
        let session = self.sessions.read().values().next().cloned()
            .ok_or_else(|| GoblinError::NoActiveSession)?;

        let current_task = task_id.or_else(|| session.current_task());
        
        if let Some(tid) = current_task {
            let _ = self.event_tx.send(Event::TaskInterrupted {
                sub_id: sub_id.clone(),
                task_id: tid,
            });
            session.set_current_task(None);
            info!(task_id = %tid, "Task interrupted");
        }

        Ok(())
    }

    /// Spawn a new agent
    async fn spawn_agent(
        &mut self,
        config: AgentConfig,
        parent_id: Option<AgentId>,
        sub_id: &SubmissionId,
    ) -> Result<(), GoblinError> {
        let session = self.sessions.read().values().next().cloned()
            .ok_or_else(|| GoblinError::NoActiveSession)?;

        session.spawn_agent(config, parent_id, sub_id)?;
        Ok(())
    }

    /// Terminate an agent
    async fn terminate_agent(
        &mut self,
        agent_id: &AgentId,
        reason: Option<String>,
        sub_id: &SubmissionId,
    ) -> Result<(), GoblinError> {
        let session = self.sessions.read().values().next().cloned()
            .ok_or_else(|| GoblinError::NoActiveSession)?;

        session.terminate_agent(agent_id, reason.unwrap_or_default(), sub_id)?;
        Ok(())
    }

    /// Handle execution approval
    async fn handle_exec_approval(
        &mut self,
        call_id: warhorn::CallId,
        approved: bool,
        sub_id: &SubmissionId,
    ) -> Result<(), GoblinError> {
        // TODO: Route approval to the agent that requested it
        debug!(call_id = %call_id, approved = approved, "Execution approval received");
        Ok(())
    }

    /// Get a session by ID
    pub fn get_session(&self, id: &SessionId) -> Option<SessionHandle> {
        self.sessions.read().get(id).cloned()
    }

    /// Get all session IDs
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.read().keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::GoblinChannel;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let tools = ToolRegistry::new();
        let (orchestrator, _channel) = Orchestrator::with_channel(tools);
        assert!(orchestrator.session_ids().is_empty());
    }
}
