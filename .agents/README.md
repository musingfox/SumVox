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
    - `planner.md` - Planning documents
    - `coder.md` - Implementation logs
    - `reviewer.md` - Review results
    - `retro.md` - Retrospective analysis
- `retro/` - **Retrospective analysis reports** (gitignored)

## Setup

### First Time Setup
After running `/init-agents`, dependencies are automatically installed. If you clone this repo:

```bash
cd .agents
bun install
```

## Task Lifecycle

1. **Create**: `AgentTask.create(taskId, title, complexity)`
2. **Work**: Agents update status and write outputs
3. **Complete**: Mark as done, calculate actual complexity
4. **Cleanup**: Auto-delete after 90 days (based on file mtime)

## Usage Examples

### Create a task
```javascript
const { AgentTask } = require('./.agents/lib');
const task = AgentTask.create('LIN-123', 'Implement auth API', 8);
```

### Agent updates status
```javascript
task.updateAgent('planner', {
  status: 'completed',
  tokens_used: 1200,
  handoff_to: 'coder'
});
```

### Write detailed output
```javascript
task.writeAgentOutput('planner', `
# Planning Document
## Requirements
...
`);
```

### Find my tasks
```javascript
const myTasks = AgentTask.findMyTasks('coder');
```

### Cleanup old tasks
```javascript
const cleaned = AgentTask.cleanup(90); // 90 days
console.log(`Cleaned ${cleaned} old tasks`);
```

## States

Check `states.yml` for:
- Task states (pending, in_progress, blocked, completed, failed, cancelled)
- Agent states (idle, working, completed, blocked, skipped)
- Complexity scale (Fibonacci: 1, 2, 3, 5, 8, 13, 21, 34, 55, 89)

## Maintenance

Tasks are automatically cleaned up 90 days after completion based on file modification time. No archive directory needed.
