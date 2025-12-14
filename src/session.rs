//! Session management for goblin orchestration

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use warhorn::{
    AgentId, SessionId, TaskId, AgentConfig,
    SessionConfig, Event, SubmissionId,
};
use trinkets::ToolRegistry;

use crate::agent::{Agent, AgentHandle};
use crate::hierarchy::AgentHierarchy;
use crate::error::GoblinError;

/// A goblin orchestration session
pub struct Session {
    /// Session ID
    pub id: SessionId,
    /// Session configuration
    pub config: SessionConfig,
    /// All agents in this session
    agents: RwLock<HashMap<AgentId, AgentHandle>>,
    /// Agent hierarchy
    hierarchy: RwLock<AgentHierarchy>,
    /// Shared tool registry
    tools: Arc<ToolRegistry>,
    /// Event sender
    event_tx: mpsc::UnboundedSender<Event>,
    /// Current active task
    current_task: RwLock<Option<TaskId>>,
}

impl Session {
    /// Create a new session
    pub fn new(
        config: SessionConfig,
        tools: Arc<ToolRegistry>,
        event_tx: mpsc::UnboundedSender<Event>,
    ) -> Self {
        let id = SessionId::new();
        
        info!(session_id = %id, "Creating new session");
        
        Self {
            id,
            config,
            agents: RwLock::new(HashMap::new()),
            hierarchy: RwLock::new(AgentHierarchy::new()),
            tools,
            event_tx,
            current_task: RwLock::new(None),
        }
    }

    /// Spawn a new agent in this session
    pub fn spawn_agent(
        &self,
        config: AgentConfig,
        parent_id: Option<AgentId>,
        sub_id: &SubmissionId,
    ) -> Result<AgentHandle, GoblinError> {
        // Verify parent exists if specified
        if let Some(pid) = &parent_id {
            let agents = self.agents.read();
            let parent = agents.get(pid).ok_or_else(|| {
                GoblinError::AgentNotFound(*pid)
            })?;
            
            // Check if parent can spawn
            if !parent.can_spawn() {
                return Err(GoblinError::SpawnDenied(
                    "Parent agent cannot spawn more children".into()
                ));
            }
        }

        // Create the agent
        let agent = Agent::new(
            config.clone(),
            parent_id,
            Arc::clone(&self.tools),
            self.event_tx.clone(),
        );
        let agent_id = agent.id;
        let handle = AgentHandle::new(agent);

        // Add to registry
        self.agents.write().insert(agent_id, handle.clone());

        // Update hierarchy
        {
            let mut hierarchy = self.hierarchy.write();
            hierarchy.add_agent(agent_id, config.role.clone(), parent_id);
        }

        // Update parent's children list
        if let Some(pid) = &parent_id {
            if let Some(parent) = self.agents.read().get(pid) {
                parent.add_child(agent_id);
            }
        }

        // Emit event
        let _ = self.event_tx.send(Event::AgentSpawned {
            sub_id: sub_id.clone(),
            agent_id,
            parent_id,
            role: config.role.clone(),
            config,
        });

        info!(
            session_id = %self.id,
            agent_id = %agent_id,
            parent = ?parent_id,
            "Spawned agent"
        );

        Ok(handle)
    }

    /// Get an agent by ID
    pub fn get_agent(&self, id: &AgentId) -> Option<AgentHandle> {
        self.agents.read().get(id).cloned()
    }

    /// Get all agent IDs
    pub fn agent_ids(&self) -> Vec<AgentId> {
        self.agents.read().keys().copied().collect()
    }

    /// Get agent count
    pub fn agent_count(&self) -> usize {
        self.agents.read().len()
    }

    /// Terminate an agent
    pub fn terminate_agent(
        &self,
        agent_id: &AgentId,
        reason: String,
        sub_id: &SubmissionId,
    ) -> Result<(), GoblinError> {
        let agent = self.agents.write().remove(agent_id).ok_or_else(|| {
            GoblinError::AgentNotFound(*agent_id)
        })?;

        // Remove from parent's children
        if let Some(pid) = agent.parent_id {
            if let Some(parent) = self.agents.read().get(&pid) {
                parent.remove_child(agent_id);
            }
        }

        // Terminate children recursively
        for child_id in agent.children() {
            let _ = self.terminate_agent(&child_id, "Parent terminated".into(), sub_id);
        }

        // Update hierarchy
        self.hierarchy.write().remove_agent(agent_id);

        // Terminate the agent
        agent.terminate(sub_id, reason);

        info!(
            session_id = %self.id,
            agent_id = %agent_id,
            "Terminated agent"
        );

        Ok(())
    }

    /// Get the hierarchy tree
    pub fn hierarchy(&self) -> warhorn::AgentTree {
        self.hierarchy.read().to_tree(&self.agents.read())
    }

    /// Set current task
    pub fn set_current_task(&self, task_id: Option<TaskId>) {
        *self.current_task.write() = task_id;
    }

    /// Get current task
    pub fn current_task(&self) -> Option<TaskId> {
        *self.current_task.read()
    }

    /// Get the root orchestrator agent (if exists)
    pub fn orchestrator(&self) -> Option<AgentHandle> {
        self.hierarchy.read().root().and_then(|id| self.get_agent(&id))
    }
}

/// Handle to a session for external interaction
#[derive(Clone)]
pub struct SessionHandle {
    inner: Arc<Session>,
}

impl SessionHandle {
    pub fn new(session: Session) -> Self {
        Self {
            inner: Arc::new(session),
        }
    }

    pub fn id(&self) -> SessionId {
        self.inner.id
    }
}

impl std::ops::Deref for SessionHandle {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warhorn::AgentRole;

    fn create_test_session() -> (Session, mpsc::UnboundedReceiver<Event>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let tools = Arc::new(ToolRegistry::new());
        let config = SessionConfig::default();
        (Session::new(config, tools, tx), rx)
    }

    #[test]
    fn test_session_creation() {
        let (session, _rx) = create_test_session();
        assert_eq!(session.agent_count(), 0);
    }

    #[test]
    fn test_spawn_agent() {
        let (session, mut rx) = create_test_session();
        let sub_id = SubmissionId::new();
        
        let config = AgentConfig {
            role: AgentRole::Orchestrator,
            ..Default::default()
        };
        
        let result = session.spawn_agent(config, None, &sub_id);
        assert!(result.is_ok());
        assert_eq!(session.agent_count(), 1);
        
        // Check event was emitted
        let event = rx.try_recv();
        assert!(matches!(event, Ok(Event::AgentSpawned { .. })));
    }
}
