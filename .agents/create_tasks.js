const { AgentTask } = require('./lib.js');

// Task definitions from PM breakdown
const tasks = [
  {
    id: 'VOICE-001',
    title: '基礎設定與環境準備',
    complexity: 3,
    description: 'Setup Python environment, install dependencies, create project structure'
  },
  {
    id: 'VOICE-002',
    title: '配置文件架構設計',
    complexity: 5,
    description: 'Design voice_config.json structure, API key env vars, model priorities'
  },
  {
    id: 'VOICE-003',
    title: 'Hook 整合層實現',
    complexity: 8,
    description: 'Implement Claude Code Stop Hook integration, read last output, register hook'
  },
  {
    id: 'VOICE-004',
    title: 'LiteLLM 多模型適配層',
    complexity: 8,
    description: 'Implement LiteLLM client, model fallback chain, token tracking, cost control'
  },
  {
    id: 'VOICE-005',
    title: '摘要生成引擎實現',
    complexity: 8,
    description: 'Design prompt templates, operation type recognition, result status parsing'
  },
  {
    id: 'VOICE-006',
    title: 'macOS 語音引擎集成',
    complexity: 5,
    description: 'Implement macOS say command wrapper, Ting-Ting voice, async playback'
  },
  {
    id: 'VOICE-007',
    title: '完整通知流程整合',
    complexity: 8,
    description: 'Integrate Hook → Summarizer → LLM → Voice Engine pipeline'
  },
  {
    id: 'VOICE-008',
    title: '配置驗證與初始化腳本',
    complexity: 5,
    description: 'Config validation, env var checks, setup.sh script, health check'
  },
  {
    id: 'VOICE-009',
    title: '完整文件與使用說明',
    complexity: 5,
    description: 'Write README, configuration guide, API docs, troubleshooting guide'
  },
  {
    id: 'VOICE-010',
    title: '綜合測試與品質保證',
    complexity: 8,
    description: 'Run test suite, code quality checks, performance tests, security audit'
  },
  {
    id: 'VOICE-011',
    title: '最終驗收與發布準備',
    complexity: 3,
    description: 'UAT, integration verification, CHANGELOG, version tagging'
  }
];

// Create all tasks
console.log('Creating tasks in .agents/tasks/...\n');

tasks.forEach(taskDef => {
  try {
    const task = AgentTask.create(taskDef.id, taskDef.title, taskDef.complexity);
    console.log(`✓ Created ${taskDef.id}: ${taskDef.title} (complexity: ${taskDef.complexity})`);

    // Update task with description
    const taskData = task.load();
    taskData.description = taskDef.description;
    taskData.metadata.project = 'Claude Code Voice Notification Hook';
    task.save(taskData);

  } catch (error) {
    console.error(`✗ Failed to create ${taskDef.id}: ${error.message}`);
  }
});

console.log('\n✓ All tasks created successfully!');
console.log('\nNext: Start with VOICE-001 (基礎設定與環境準備)');
