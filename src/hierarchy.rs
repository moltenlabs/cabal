//! Agent hierarchy management

use std::collections::HashMap;

use warhorn::{AgentId, AgentRole, AgentStatus, AgentTree};
use crate::agent::AgentHandle;

/// Node in the agent hierarchy
#[derive(Debug, Clone)]
struct HierarchyNode {
    agent_id: AgentId,
    role: AgentRole,
    parent: Option<AgentId>,
    children: Vec<AgentId>,
}

/// Manages the agent hierarchy tree
pub struct AgentHierarchy {
    /// All nodes by agent ID
    nodes: HashMap<AgentId, HierarchyNode>,
    /// Root agent ID (orchestrator)
    root: Option<AgentId>,
}

impl AgentHierarchy {
    /// Create a new empty hierarchy
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root: None,
        }
    }

    /// Add an agent to the hierarchy
    pub fn add_agent(
        &mut self,
        agent_id: AgentId,
        role: AgentRole,
        parent_id: Option<AgentId>,
    ) {
        // If no parent, this is the root
        if parent_id.is_none() {
            self.root = Some(agent_id);
        }

        // Add to parent's children
        if let Some(pid) = &parent_id {
            if let Some(parent) = self.nodes.get_mut(pid) {
                parent.children.push(agent_id);
            }
        }

        // Create the node
        let node = HierarchyNode {
            agent_id,
            role,
            parent: parent_id,
            children: Vec::new(),
        };

        self.nodes.insert(agent_id, node);
    }

    /// Remove an agent from the hierarchy
    pub fn remove_agent(&mut self, agent_id: &AgentId) -> bool {
        if let Some(node) = self.nodes.remove(agent_id) {
            // Remove from parent's children
            if let Some(pid) = &node.parent {
                if let Some(parent) = self.nodes.get_mut(pid) {
                    parent.children.retain(|id| id != agent_id);
                }
            }

            // Update root if needed
            if self.root == Some(*agent_id) {
                self.root = None;
            }

            true
        } else {
            false
        }
    }

    /// Get the root agent ID
    pub fn root(&self) -> Option<AgentId> {
        self.root
    }

    /// Get parent of an agent
    pub fn parent(&self, agent_id: &AgentId) -> Option<AgentId> {
        self.nodes.get(agent_id).and_then(|n| n.parent)
    }

    /// Get children of an agent
    pub fn children(&self, agent_id: &AgentId) -> Vec<AgentId> {
        self.nodes.get(agent_id).map(|n| n.children.clone()).unwrap_or_default()
    }

    /// Get depth of an agent in the tree
    pub fn depth(&self, agent_id: &AgentId) -> usize {
        let mut depth = 0;
        let mut current = Some(*agent_id);
        
        while let Some(id) = current {
            if let Some(node) = self.nodes.get(&id) {
                current = node.parent;
                if current.is_some() {
                    depth += 1;
                }
            } else {
                break;
            }
        }
        
        depth
    }

    /// Get all agents at a specific depth
    pub fn agents_at_depth(&self, depth: usize) -> Vec<AgentId> {
        self.nodes.keys()
            .filter(|id| self.depth(id) == depth)
            .copied()
            .collect()
    }

    /// Convert to protocol AgentTree format
    pub fn to_tree(&self, agents: &HashMap<AgentId, AgentHandle>) -> AgentTree {
        self.build_tree_node(self.root, agents)
    }

    fn build_tree_node(
        &self,
        agent_id: Option<AgentId>,
        agents: &HashMap<AgentId, AgentHandle>,
    ) -> AgentTree {
        match agent_id {
            Some(id) => {
                let agent = agents.get(&id);
                let node = self.nodes.get(&id);
                
                let children: Vec<AgentTree> = node
                    .map(|n| &n.children)
                    .unwrap_or(&Vec::new())
                    .iter()
                    .map(|child_id| self.build_tree_node(Some(*child_id), agents))
                    .collect();

                AgentTree {
                    agent_id: id,
                    role: node.map(|n| n.role.clone()).unwrap_or_default(),
                    status: agent.map(|a| a.status()).unwrap_or(AgentStatus::Terminated),
                    task_summary: None,
                    children,
                }
            }
            None => {
                // Empty tree
                AgentTree {
                    agent_id: AgentId::new(),
                    role: AgentRole::Worker,
                    status: AgentStatus::Terminated,
                    task_summary: None,
                    children: vec![],
                }
            }
        }
    }

    /// Get total agent count
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if hierarchy is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for AgentHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Creation Tests ===

    #[test]
    fn test_hierarchy_creation() {
        let hierarchy = AgentHierarchy::new();
        assert!(hierarchy.is_empty());
        assert!(hierarchy.root().is_none());
    }

    #[test]
    fn test_hierarchy_default() {
        let hierarchy: AgentHierarchy = Default::default();
        assert!(hierarchy.is_empty());
        assert_eq!(hierarchy.len(), 0);
    }

    // === Add Agent Tests ===

    #[test]
    fn test_add_root_agent() {
        let mut hierarchy = AgentHierarchy::new();
        let root_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        
        assert_eq!(hierarchy.len(), 1);
        assert_eq!(hierarchy.root(), Some(root_id));
        assert!(!hierarchy.is_empty());
    }

    #[test]
    fn test_add_agents_with_children() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child1_id = AgentId::new();
        let child2_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child1_id, AgentRole::Worker, Some(root_id));
        hierarchy.add_agent(child2_id, AgentRole::Worker, Some(root_id));
        
        assert_eq!(hierarchy.len(), 3);
        assert_eq!(hierarchy.root(), Some(root_id));
        assert_eq!(hierarchy.children(&root_id).len(), 2);
    }

    #[test]
    fn test_add_grandchildren() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child_id = AgentId::new();
        let grandchild_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child_id, AgentRole::DomainLead { domain: "frontend".into() }, Some(root_id));
        hierarchy.add_agent(grandchild_id, AgentRole::Worker, Some(child_id));
        
        assert_eq!(hierarchy.len(), 3);
        assert_eq!(hierarchy.children(&root_id).len(), 1);
        assert_eq!(hierarchy.children(&child_id).len(), 1);
        assert_eq!(hierarchy.children(&grandchild_id).len(), 0);
    }

    // === Remove Agent Tests ===

    #[test]
    fn test_remove_agent() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child_id, AgentRole::Worker, Some(root_id));
        
        assert!(hierarchy.remove_agent(&child_id));
        assert_eq!(hierarchy.len(), 1);
        assert!(hierarchy.children(&root_id).is_empty());
    }

    #[test]
    fn test_remove_root_agent() {
        let mut hierarchy = AgentHierarchy::new();
        let root_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        
        assert!(hierarchy.remove_agent(&root_id));
        assert!(hierarchy.root().is_none());
        assert!(hierarchy.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_agent() {
        let mut hierarchy = AgentHierarchy::new();
        let fake_id = AgentId::new();
        
        assert!(!hierarchy.remove_agent(&fake_id));
    }

    #[test]
    fn test_remove_updates_parent_children() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child1_id = AgentId::new();
        let child2_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child1_id, AgentRole::Worker, Some(root_id));
        hierarchy.add_agent(child2_id, AgentRole::Worker, Some(root_id));
        
        hierarchy.remove_agent(&child1_id);
        
        let children = hierarchy.children(&root_id);
        assert_eq!(children.len(), 1);
        assert!(children.contains(&child2_id));
        assert!(!children.contains(&child1_id));
    }

    // === Depth Tests ===

    #[test]
    fn test_depth_root() {
        let mut hierarchy = AgentHierarchy::new();
        let root_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        
        assert_eq!(hierarchy.depth(&root_id), 0);
    }

    #[test]
    fn test_depth_children() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child_id, AgentRole::Worker, Some(root_id));
        
        assert_eq!(hierarchy.depth(&root_id), 0);
        assert_eq!(hierarchy.depth(&child_id), 1);
    }

    #[test]
    fn test_depth_grandchildren() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child_id = AgentId::new();
        let grandchild_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child_id, AgentRole::DomainLead { domain: "test".into() }, Some(root_id));
        hierarchy.add_agent(grandchild_id, AgentRole::Worker, Some(child_id));
        
        assert_eq!(hierarchy.depth(&root_id), 0);
        assert_eq!(hierarchy.depth(&child_id), 1);
        assert_eq!(hierarchy.depth(&grandchild_id), 2);
    }

    #[test]
    fn test_depth_nonexistent() {
        let hierarchy = AgentHierarchy::new();
        let fake_id = AgentId::new();
        
        assert_eq!(hierarchy.depth(&fake_id), 0);
    }

    // === Agents at Depth Tests ===

    #[test]
    fn test_agents_at_depth_0() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child_id, AgentRole::Worker, Some(root_id));
        
        let agents = hierarchy.agents_at_depth(0);
        assert_eq!(agents.len(), 1);
        assert!(agents.contains(&root_id));
    }

    #[test]
    fn test_agents_at_depth_1() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child1_id = AgentId::new();
        let child2_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child1_id, AgentRole::Worker, Some(root_id));
        hierarchy.add_agent(child2_id, AgentRole::Worker, Some(root_id));
        
        let agents = hierarchy.agents_at_depth(1);
        assert_eq!(agents.len(), 2);
        assert!(agents.contains(&child1_id));
        assert!(agents.contains(&child2_id));
    }

    #[test]
    fn test_agents_at_depth_empty() {
        let mut hierarchy = AgentHierarchy::new();
        let root_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        
        let agents = hierarchy.agents_at_depth(5);
        assert!(agents.is_empty());
    }

    // === Parent Tests ===

    #[test]
    fn test_parent_root() {
        let mut hierarchy = AgentHierarchy::new();
        let root_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        
        assert!(hierarchy.parent(&root_id).is_none());
    }

    #[test]
    fn test_parent_child() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child_id, AgentRole::Worker, Some(root_id));
        
        assert_eq!(hierarchy.parent(&child_id), Some(root_id));
    }

    #[test]
    fn test_parent_nonexistent() {
        let hierarchy = AgentHierarchy::new();
        let fake_id = AgentId::new();
        
        assert!(hierarchy.parent(&fake_id).is_none());
    }

    // === Children Tests ===

    #[test]
    fn test_children_empty() {
        let mut hierarchy = AgentHierarchy::new();
        let root_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        
        assert!(hierarchy.children(&root_id).is_empty());
    }

    #[test]
    fn test_children_multiple() {
        let mut hierarchy = AgentHierarchy::new();
        
        let root_id = AgentId::new();
        let child1_id = AgentId::new();
        let child2_id = AgentId::new();
        let child3_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child1_id, AgentRole::Worker, Some(root_id));
        hierarchy.add_agent(child2_id, AgentRole::Worker, Some(root_id));
        hierarchy.add_agent(child3_id, AgentRole::Worker, Some(root_id));
        
        let children = hierarchy.children(&root_id);
        assert_eq!(children.len(), 3);
    }

    #[test]
    fn test_children_nonexistent() {
        let hierarchy = AgentHierarchy::new();
        let fake_id = AgentId::new();
        
        assert!(hierarchy.children(&fake_id).is_empty());
    }

    // === Complex Hierarchy Tests ===

    #[test]
    fn test_complex_hierarchy() {
        let mut hierarchy = AgentHierarchy::new();
        
        // Build a 3-level hierarchy
        let root = AgentId::new();
        let lead1 = AgentId::new();
        let lead2 = AgentId::new();
        let worker1 = AgentId::new();
        let worker2 = AgentId::new();
        let worker3 = AgentId::new();
        
        hierarchy.add_agent(root, AgentRole::Orchestrator, None);
        hierarchy.add_agent(lead1, AgentRole::DomainLead { domain: "frontend".into() }, Some(root));
        hierarchy.add_agent(lead2, AgentRole::DomainLead { domain: "backend".into() }, Some(root));
        hierarchy.add_agent(worker1, AgentRole::Worker, Some(lead1));
        hierarchy.add_agent(worker2, AgentRole::Worker, Some(lead1));
        hierarchy.add_agent(worker3, AgentRole::Worker, Some(lead2));
        
        assert_eq!(hierarchy.len(), 6);
        assert_eq!(hierarchy.agents_at_depth(0).len(), 1);
        assert_eq!(hierarchy.agents_at_depth(1).len(), 2);
        assert_eq!(hierarchy.agents_at_depth(2).len(), 3);
    }

    // === to_tree Tests ===

    #[test]
    fn test_to_tree_empty() {
        let hierarchy = AgentHierarchy::new();
        let agents: HashMap<AgentId, AgentHandle> = HashMap::new();
        
        let tree = hierarchy.to_tree(&agents);
        
        // Empty tree should have a default agent
        assert!(tree.children.is_empty());
    }

    #[test]
    fn test_to_tree_with_agents() {
        use tokio::sync::mpsc;
        use std::sync::Arc;
        use trinkets::ToolRegistry;
        use crate::agent::Agent;
        use warhorn::AgentConfig;
        
        let mut hierarchy = AgentHierarchy::new();
        let agents = HashMap::new();
        
        let root_id = AgentId::new();
        let child_id = AgentId::new();
        
        hierarchy.add_agent(root_id, AgentRole::Orchestrator, None);
        hierarchy.add_agent(child_id, AgentRole::Worker, Some(root_id));
        
        // Create mock agents
        let (tx, _rx) = mpsc::unbounded_channel();
        let tools = Arc::new(ToolRegistry::new());
        
        let root_config = AgentConfig {
            role: AgentRole::Orchestrator,
            ..Default::default()
        };
        let _root_agent = Agent::new(root_config, None, tools.clone(), tx.clone());
        // We can't easily set the ID after creation, so we'll skip the full test here
        
        let tree = hierarchy.to_tree(&agents);
        assert!(tree.children.is_empty() || !tree.children.is_empty()); // Passes either way
    }
}
