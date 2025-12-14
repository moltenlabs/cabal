# 🎭 Cabal

Hierarchical agent orchestration system - the scheming group.

[![Crates.io](https://img.shields.io/crates/v/cabal.svg)](https://crates.io/crates/cabal)
[![Documentation](https://docs.rs/cabal/badge.svg)](https://docs.rs/cabal)
[![License](https://img.shields.io/crates/l/cabal.svg)](LICENSE)

## Overview

Cabal is the orchestration layer for hierarchical AI agent systems. It manages agent lifecycles, coordinates task execution, and handles communication between agents.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          ORCHESTRATOR (Level 0)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │ Task Planner │  │ Agent Factory│  │ Result Merger│               │
│  └──────────────┘  └──────────────┘  └──────────────┘               │
└────────────────────────────┬────────────────────────────────────────┘
                             │
         ┌───────────────────┼───────────────────┐
         ▼                   ▼                   ▼
  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
  │ Domain Lead │     │ Domain Lead │     │ Domain Lead │
  │  (Level 1)  │     │  (Level 1)  │     │  (Level 1)  │
  └──────┬──────┘     └──────┬──────┘     └──────┬──────┘
         │                   │                   │
    ┌────┴────┐         ┌────┴────┐         ┌────┴────┐
    ▼         ▼         ▼         ▼         ▼         ▼
 ┌─────┐  ┌─────┐    ┌─────┐  ┌─────┐    ┌─────┐  ┌─────┐
 │ W1  │  │ W2  │    │ W3  │  │ W4  │    │ W5  │  │ W6  │
 └─────┘  └─────┘    └─────┘  └─────┘    └─────┘  └─────┘
```

## Features

- 🏗️ Hierarchical agent spawning
- 📨 Op/Event communication protocol
- 🔄 Session management
- 👥 Agent lifecycle management
- 📊 Token usage tracking

## Installation

```toml
[dependencies]
cabal = "0.1"
```

## Usage

```rust
use cabal::{Orchestrator, GoblinChannel, Op, Event};
use trinkets::ToolRegistry;

#[tokio::main]
async fn main() {
    // Create orchestrator with tool registry
    let registry = ToolRegistry::new();
    let (orchestrator, channel) = Orchestrator::with_channel(registry);

    // Spawn orchestrator in background
    tokio::spawn(orchestrator.run());

    // Send operations
    channel.send(Op::user_input("Build a REST API")).unwrap();

    // Handle events
    while let Some(event) = channel.recv().await {
        match event {
            Event::AgentSpawned { agent_id, role, .. } => {
                println!("Agent {} spawned as {:?}", agent_id, role);
            }
            Event::TaskComplete { result, .. } => {
                println!("Done: {}", result.summary);
                break;
            }
            _ => {}
        }
    }
}
```

## Agent Roles

```rust
use warhorn::AgentRole;

// Orchestrator - top-level coordinator
let orchestrator = AgentRole::Orchestrator;

// Domain leads - coordinate specific areas
let frontend_lead = AgentRole::DomainLead { 
    domain: "frontend".into() 
};

// Workers - execute specific tasks
let worker = AgentRole::Worker;

// Specialists - experts in specific areas
let security = AgentRole::Specialist { 
    specialty: "security".into() 
};
```

## Part of the Goblin Family

- [warhorn](https://crates.io/crates/warhorn) - Protocol types
- [trinkets](https://crates.io/crates/trinkets) - Tool registry
- [wardstone](https://crates.io/crates/wardstone) - Sandboxing
- [skulk](https://crates.io/crates/skulk) - MCP connections
- [hutch](https://crates.io/crates/hutch) - Checkpoints
- [ambush](https://crates.io/crates/ambush) - Task planning
- **cabal** - Orchestration (you are here)

## License

MIT OR Apache-2.0
