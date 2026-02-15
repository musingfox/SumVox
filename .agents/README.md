# Agent Workspace

This directory contains the Agent-First workflow workspace for this project.

## Structure

- `package.json` - **Dependencies** (yaml package)
- `node_modules/` - **Installed packages** (gitignored)
- `config.yml` - **Dynamic configuration** (initialization time, runtime stats)
- `states.yml` - **State definitions** (task states, agent states, complexity scale)
- `lib.js` - **Agent helper library** (AgentTask class)
- `tasks/` - **Active tasks** (JSON + markdown details, gitignored)
  - `{task-id}.json` - Task state and metadata
  - `{task-id}/` - Detailed agent outputs
- `retro/` - **Retrospective analysis reports** (gitignored)
- `outputs/` - **Agent output reports** (gitignored)

## Setup

After cloning this repo:

```bash
cd .agents
bun install
```

## Task Lifecycle

1. **Create**: `AgentTask.create(taskId, title, complexity)`
2. **Work**: Agents update status and write outputs
3. **Complete**: Mark as done, calculate actual complexity
4. **Cleanup**: Auto-delete after 90 days
