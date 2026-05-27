# agent-runner

A minimal, non-interactive AI agent runner in your server or container. Give it a folder with AGENTS.md and skills, it will uses tools, MCP and skills and iterates until the task is done.

## Quick Start

```sh
cd agent-runner
cargo build --release
cp .env.example .env   # edit with your API key

./target/release/agent-runner --agent-dir ./my-agent --prompt "Refactor the auth module"
```

### Docker

```sh
cd agent-runner
docker build -t agent-runner .
docker run --env-file .env agent-runner \
  --agent-dir /agents/my-agent --prompt "Fix the tests"
```

## Agent = agent-runner + Folder

Everything an agent needs lives in one folder:

```
my-agent/
├── AGENTS.md              # System prompt — who the agent is and how it behaves
├── agent-runner.json      # MCP configuration
└── skills/                # Optional: extra skills
    └── search/
        ├── SKILL.md       # Skill instructions injected into the system prompt
        ├── references/    # Reference documents for the skill
        └── scripts/       # Executable scripts (exposed as agent tools)
```

That's it. No database, no server, no setup beyond the folder.

## agent-runner.json

Only MCP server configuration. LLM settings come from environment variables or `.env`:

```json
{
  "mcp_servers": {}
}
```

With MCP servers:

```json
{
  "mcp_servers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/data"],
      "env": {}
    }
  }
}
```

### Environment Variables

Set LLM provider, model, and API key via environment variables or a `.env` file:

```
LLM_PROVIDER=anthropic
LLM_MODEL=claude-sonnet-4-20250514
LLM_BASE_URL=https://api.anthropic.com
ANTHROPIC_API_KEY=sk-ant-...
```

| Variable | Required | Description |
|----------|----------|-------------|
| `LLM_PROVIDER` | yes | `anthropic` or `openai` |
| `LLM_MODEL` | yes | Model name (e.g. `claude-sonnet-4-20250514`, `gpt-4o`) |
| `LLM_BASE_URL` | no | Override base URL for OpenAI-compatible APIs |
| `LLM_API_KEY` | yes | API key (or use provider-specific name below) |
| `ANTHROPIC_API_KEY` | if provider=anthropic | Anthropic API key |
| `OPENAI_API_KEY` | if provider=openai | OpenAI API key |

### API Keys

API keys can be provided via:

- **`.env` file** — place in the working directory (loaded automatically)
- **Environment variables** — `export ANTHROPIC_API_KEY=sk-ant-...`

### Full Configuration Reference

agent-runner.json only needs `mcp_servers` (can be empty). All other settings have defaults and can be overridden via environment variables:

```json
{
  "mcp_servers": {},
  "agent": {
    "max_iterations": 50,
    "plan_required": true,
    "tool_output_token_limit": 20000,
    "user_message_token_limit": 50000,
    "execute_timeout_secs": 3600,
    "execute_enabled": false
  },
  "summarization": {
    "enabled": true,
    "trigger_tokens": 80000,
    "keep_tokens": 20000,
    "trim_tokens": 4000
  },
  "permissions": [
    {
      "operations": ["read"],
      "paths": ["./*"],
      "mode": "allow"
    }
  ],
  "subagents": []
}
```

## CLI

```
agent-runner --agent-dir <DIR> --prompt <TEXT|FILE> [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--agent-dir` | (required) | Path to agent folder |
| `--prompt` | (required) | Task prompt or path to a text file |
| `--plan-only` | `false` | Generate plan and exit without executing |
| `--max-iterations` | `50` | Maximum agent loop iterations |
| `--output-dir` | `./agent-output` | Output directory for reports and traces |
| `--working-dir` | `.` | Working directory for filesystem/execute tools |
| `--tool-timeout` | `120` | Timeout in seconds for each tool call |
| `--run-limit` | `3600` | Maximum total run time in seconds |
| `--mail-to` | (none) | Email address for result notification |
| `--verbose` | `false` | Print iteration details to stderr |
| `--sandbox` | `false` | Enable shell execution regardless of config |

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Task completed |
| `1` | Task failed |
| `2` | Max iterations or run limit exceeded |
| `3` | Configuration error |

## Built-in Tools

The agent has these tools available by default:

| Tool | Description |
|------|-------------|
| `ls` | List directory entries |
| `read_file` | Read file contents with line-based pagination |
| `write_file` | Write content to a file (creates parent dirs) |
| `edit_file` | Find-and-replace strings in a file |
| `glob` | Find files matching a glob pattern |
| `grep` | Search file contents with regex |
| `execute` | Run a shell command (when enabled) |
| `task_done` | Signal task completion |
| `write_todos` | Update internal todo list |
| `compact_conversation` | Trigger conversation compaction |

### Permissions

Control which tools can access which paths:

```json
{
  "permissions": [
    { "operations": ["read"], "paths": ["./*"], "mode": "allow" },
    { "operations": ["write"], "paths": ["./src/*"], "mode": "allow" },
    { "operations": ["write"], "paths": ["./secrets/*"], "mode": "deny" }
  ]
}
```

Operations: `"read"` covers `ls`, `read_file`, `glob`, `grep`. `"write"` covers `write_file`, `edit_file`, `execute`. Paths support `/*` for prefix matching.

## Output

After execution, the output directory contains:

| File | Description |
|------|-------------|
| `run.json` | Detailed run log with per-iteration and per-tool TAT, errors, and exceptions |
| `plan.md` | Generated execution plan |
| `report.json` | Status, token usage, iterations, duration, todos |
| `transcript.json` | Full message history |
| `trace.jsonl` | Structured event log (one JSON object per line) |

### run.json

Every run produces a `run.json` with full debugging details:

```json
{
  "status": "completed",
  "exit_code": 0,
  "started_at": "2026-05-26T12:00:00.000Z",
  "finished_at": "2026-05-26T12:01:23.456Z",
  "duration_ms": 83456,
  "iterations": [
    {
      "iteration": 1,
      "started_at": "...",
      "llm_tat_ms": 3200,
      "llm_input_tokens": 1200,
      "llm_output_tokens": 340,
      "llm_error": null,
      "tool_calls": [
        {
          "tool": "read_file",
          "arguments": {"file_path": "src/main.rs"},
          "result": "...",
          "tat_ms": 12,
          "is_error": false,
          "error": null,
          "permission_denied": null,
          "timed_out": false
        }
      ]
    }
  ],
  "errors": []
}
```

## How It Works

```
┌─────────────┐     ┌─────────────┐     ┌──────────────────┐
│  Load agent  │────▶│  Plan task  │────▶│  Agent loop      │
│  folder      │     │  (optional) │     │  ┌─────────────┐ │
└─────────────┘     └─────────────┘     │  │ LLM call    │ │
                                        │  │      ↓      │ │
                                        │  │ Tool calls  │ │
                                        │  │      ↓      │ │
                                        │  │ Summarize   │ │
                                        │  │ (if needed) │ │
                                        │  └─────────────┘ │
                                        │         ↓         │
                                        │  task_done / max  │
                                        └──────────────────┘
                                                 ↓
                                        ┌──────────────────┐
                                        │  Write output     │
                                        │  report + trace   │
                                        └──────────────────┘
```

1. Loads the agent folder (AGENTS.md, agent-runner.json, skills)
2. Optionally generates a step-by-step execution plan
3. Runs an autonomous loop: LLM call → tool execution → repeat
4. Summarizes conversation history when context gets long
5. Exits when the agent calls `task_done` or hits max iterations
6. Writes a JSON report, transcript, and trace log

## License

MIT
