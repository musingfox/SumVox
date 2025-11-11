const fs = require('fs');
const path = require('path');
const yaml = require('yaml');

class AgentTask {
  constructor(taskId) {
    this.taskId = taskId;
    this.jsonPath = `.agents/tasks/${taskId}.json`;
    this.dirPath = `.agents/tasks/${taskId}`;
  }

  // Load task
  load() {
    if (!fs.existsSync(this.jsonPath)) {
      throw new Error(`Task ${this.taskId} not found`);
    }
    return JSON.parse(fs.readFileSync(this.jsonPath, 'utf8'));
  }

  // Save task
  save(task) {
    fs.writeFileSync(this.jsonPath, JSON.stringify(task, null, 2));
  }

  // Create new task
  static create(taskId, title, complexity = 5) {
    const states = yaml.parse(fs.readFileSync('.agents/states.yml', 'utf8'));
    const estimatedTokens = states.complexity_scale.token_estimates[complexity];

    const task = {
      task_id: taskId,
      title: title,
      status: 'pending',
      current_agent: null,
      complexity: {
        estimated: complexity,
        estimated_tokens: estimatedTokens,
        actual: null,
        actual_tokens: null
      },
      agents: {},
      metadata: {
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString()
      }
    };

    const jsonPath = `.agents/tasks/${taskId}.json`;
    const dirPath = `.agents/tasks/${taskId}`;

    fs.writeFileSync(jsonPath, JSON.stringify(task, null, 2));
    fs.mkdirSync(dirPath, { recursive: true });

    return new AgentTask(taskId);
  }

  // Set complexity
  setComplexity(complexity, estimatedTokens = null) {
    const task = this.load();
    task.complexity.estimated = complexity;

    if (estimatedTokens) {
      task.complexity.estimated_tokens = estimatedTokens;
    } else {
      const states = yaml.parse(fs.readFileSync('.agents/states.yml', 'utf8'));
      task.complexity.estimated_tokens = states.complexity_scale.token_estimates[complexity];
    }

    task.metadata.updated_at = new Date().toISOString();
    this.save(task);
  }

  // Update agent status
  updateAgent(agentName, data) {
    const task = this.load();

    if (!task.agents[agentName]) {
      task.agents[agentName] = {};
    }

    Object.assign(task.agents[agentName], data);

    if (data.status === 'working' && !task.agents[agentName].started_at) {
      task.agents[agentName].started_at = new Date().toISOString();
    }

    if (data.status === 'completed' && !task.agents[agentName].completed_at) {
      task.agents[agentName].completed_at = new Date().toISOString();
    }

    if (data.handoff_to) {
      task.current_agent = data.handoff_to;
    }

    task.metadata.updated_at = new Date().toISOString();
    this.save(task);
  }

  // Write agent output to markdown
  writeAgentOutput(agentName, content) {
    fs.mkdirSync(this.dirPath, { recursive: true });
    const outputPath = path.join(this.dirPath, `${agentName}.md`);
    fs.writeFileSync(outputPath, content);

    const task = this.load();
    if (!task.agents[agentName]) {
      task.agents[agentName] = {};
    }
    task.agents[agentName].output_file = `${agentName}.md`;
    task.metadata.updated_at = new Date().toISOString();
    this.save(task);
  }

  // Append to agent output
  appendAgentOutput(agentName, content) {
    const outputPath = path.join(this.dirPath, `${agentName}.md`);
    fs.appendFileSync(outputPath, '\n' + content);
  }

  // Read agent output
  readAgentOutput(agentName) {
    const outputPath = path.join(this.dirPath, `${agentName}.md`);
    if (!fs.existsSync(outputPath)) return null;
    return fs.readFileSync(outputPath, 'utf8');
  }

  // Mark task as completed
  complete() {
    const task = this.load();
    task.status = 'completed';
    task.current_agent = null;
    task.metadata.updated_at = new Date().toISOString();

    // Calculate actual complexity
    let totalTokens = 0;
    Object.values(task.agents).forEach(agent => {
      if (agent.tokens_used) {
        totalTokens += agent.tokens_used;
      }
    });

    task.complexity.actual_tokens = totalTokens;
    task.complexity.actual = this.mapToFibonacci(totalTokens);

    this.save(task);
  }

  // Map tokens to Fibonacci scale
  mapToFibonacci(tokens) {
    const states = yaml.parse(fs.readFileSync('.agents/states.yml', 'utf8'));
    const scale = states.complexity_scale.values;
    const estimates = states.complexity_scale.token_estimates;

    for (let i = scale.length - 1; i >= 0; i--) {
      if (tokens >= estimates[scale[i]]) {
        return scale[i];
      }
    }
    return scale[0];
  }

  // Find tasks for specific agent
  static findMyTasks(agentName) {
    const tasksDir = '.agents/tasks';
    if (!fs.existsSync(tasksDir)) return [];

    return fs.readdirSync(tasksDir)
      .filter(f => f.endsWith('.json'))
      .map(f => {
        const task = JSON.parse(fs.readFileSync(path.join(tasksDir, f), 'utf8'));
        return task;
      })
      .filter(t => t.current_agent === agentName && t.status === 'in_progress');
  }

  // Cleanup old tasks
  static cleanup(daysOld = 90) {
    const tasksDir = '.agents/tasks';
    const now = Date.now();
    const cutoff = daysOld * 24 * 60 * 60 * 1000;
    let cleaned = 0;

    fs.readdirSync(tasksDir).forEach(file => {
      if (!file.endsWith('.json')) return;

      const filePath = path.join(tasksDir, file);
      const task = JSON.parse(fs.readFileSync(filePath, 'utf8'));

      if (!['completed', 'cancelled'].includes(task.status)) return;

      const stats = fs.statSync(filePath);
      const age = now - stats.mtimeMs;

      if (age > cutoff) {
        fs.unlinkSync(filePath);
        const taskDir = path.join(tasksDir, task.task_id);
        if (fs.existsSync(taskDir)) {
          fs.rmSync(taskDir, { recursive: true });
        }
        cleaned++;
      }
    });

    return cleaned;
  }
}

module.exports = { AgentTask };
