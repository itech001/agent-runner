# agent-runner

[English](README.md) | 中文

**轻量级、通用型、非交互式 —— 一个 Rust 二进制文件，零运行时依赖，agent 完全由文件夹定义。**

一个面向服务器和容器的极简 AI Agent Runner。给它一个包含 `AGENTS.md` 和 skills 的文件夹，再加上一条 prompt：它会规划、调用工具、MCP server 和 skills，自主迭代直到任务完成，写出结果，然后退出。没有 TUI，没有聊天会话，无需安装任何运行时 —— 你不写一行代码就能定义属于自己的 agent。

![agent-runner 架构图](agent-runner-architecture.png)

## 快速开始

```sh
cd agent-runner
cargo build --release
cp .env.example .env   # 填入你的 API key

./target/release/agent-runner --agent-dir ./my-agent --prompt "重构 auth 模块"
```

### Docker

```sh
cd agent-runner
docker build -t agent-runner .
docker run --env-file .env agent-runner \
  --agent-dir /agents/my-agent --prompt "修复测试"
```

## 为什么选择 agent-runner

三个核心属性定义了它，其他一切设计都由此展开：

**轻量级。** 单个约 3 MB 的 Rust 二进制文件，零运行时依赖。没有 Node.js、没有 Python、没有包管理器、没有守护进程。毫秒级启动，可以直接拷进任何容器 —— 二进制文件本身就是全部运行时。

**通用型。** 它不专门做编码助手。Agent 文件夹就是配置面，同一个二进制可以运行代码重构 agent、文档处理 agent、数据迁移 agent、研究总结 agent —— 每个都只是一个文件夹，区别仅在 `AGENTS.md`、skills 和 MCP server。你不写任何源码就能构建领域 agent。

**非交互式。** 没有 REPL，也没有聊天窗口。你给它一条 prompt，它规划、执行、带着退出码退出。这使得它可以接入 cron、CI 步骤、队列消费者、webhook handler 或 Kubernetes Job —— 任何没有人盯着看、也没人能回答"可以吗"的地方。

## 使用场景

**定时维护。** 跑一个每晚执行的任务：更新依赖、检查许可证、核对文档与代码是否一致。`agent-runner` 以 `--run-limit` 作为硬上限无人值守运行，失败时以非零退出码结束，调度系统可以直接告警。

**CI/CD 流水线。** 在流水线门禁之前加一个 agent 步骤：修复 lint 错误、更新 changelog、迁移 API 调用点、生成 release notes。退出码（`0` 完成、`1` 失败、`2` 超限、`3` 配置错误）可以直接映射到 CI 语义。

**面向大量输入的无头批处理。** 用循环把二进制跑在一个目录的输入上：总结 500 个 PDF、分类 10,000 张工单、翻译一整套文档、为每个模块生成测试。每次调用相互隔离，第 47 条失败不会污染其余任务，而且所有运行都写出相同的结构化输出。

**自定义领域 agent。** 把团队内部知识包装成 agent 文件夹：带 runbook 和 shell 工具的 SRE agent、带政策文档的支持 agent、带数仓 MCP server 的数据 agent。这个文件夹可版本化、可评审，作为一个部署单元直接上生产。

**服务器上的受控自动化。** 由于除非路径位于 `writable_paths` 否则写入会被拒绝，你可以在线上机器上运行自主 agent，把影响范围限制得又窄又可审计：它能读取理解系统所需的一切，但只能写到你允许的位置。

**嵌入式 / 气隙环境。** 把静态二进制文件丢进离线环境或其他团队的镜像里。无需安装运行时、无需访问包仓库，部署时没有任何东西需要联网拉取。

## Agent = agent-runner + 文件夹

一个 agent 所需的全部内容都放在一个文件夹里：

```
my-agent/
├── AGENTS.md              # 系统提示词 —— agent 是谁、如何行为
├── agent-runner.json      # MCP 配置、超时、权限
└── skills/                # 可选：额外技能
    └── search/
        ├── SKILL.md       # 注入系统提示词的技能说明
        ├── references/    # 技能参考文档
        └── scripts/       # 可执行脚本（暴露为 agent 工具）
```

就这么简单。没有数据库、没有服务器、除了这个文件夹之外没有任何配置。

> agent 只从 `--agent-dir` 读取内容，**不会**扫描你的 home 目录（不会读取 `~/.agents`、`~/.skills` 或任何用户级配置）。

## agent-runner.json

MCP server、超时、权限和 agent 行为。LLM 设置来自环境变量或 `.env`：

```json
{
  "mcp_servers": {},
  "timeouts": {
    "tool_timeout_secs": 120,
    "run_limit_secs": 3600
  },
  "agent": {
    "max_iterations": 50,
    "plan_required": true,
    "execute_enabled": false
  },
  "writable_paths": ["./src/*", "/tmp/out/*"],
  "permissions": []
}
```

默认情况下所有路径都是**只读**的。只有 `writable_paths` 中列出的路径才能写入（通过 `write_file`、`edit_file`、`execute`）。模式使用 glob 语法（`**`、`*`、`?`）。`permissions` 数组保留用于高级 allow/deny 规则，在 `writable_paths` 未匹配时作为兜底检查。

### 超时设置

超时可以在 `agent-runner.json` 中设置，并被 CLI 参数覆盖：

| 设置项 | 配置键 | CLI 参数 | 默认值 |
|---------|-----------|----------|---------|
| 单工具超时 | `timeouts.tool_timeout_secs` | `--tool-timeout` | 120s |
| 整体运行上限 | `timeouts.run_limit_secs` | `--run-limit` | 3600s |

### MCP Server

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

### 环境变量

通过环境变量或 `.env` 文件设置 LLM 提供商、模型和 API key：

```
LLM_PROVIDER=anthropic
LLM_MODEL=claude-sonnet-4-20250514
LLM_BASE_URL=https://api.anthropic.com
ANTHROPIC_API_KEY=sk-ant-...
```

| 变量 | 是否必需 | 说明 |
|----------|----------|-------------|
| `LLM_PROVIDER` | 是 | `anthropic` 或 `openai` |
| `LLM_MODEL` | 是 | 模型名称（如 `claude-sonnet-4-20250514`、`gpt-4o`） |
| `LLM_BASE_URL` | 否 | 覆盖 OpenAI 兼容 API 的 base URL |
| `LLM_API_KEY` | 是 | API key（或使用下方提供商专属变量名） |
| `ANTHROPIC_API_KEY` | provider=anthropic 时 | Anthropic API key |
| `OPENAI_API_KEY` | provider=openai 时 | OpenAI API key |

### API Key

API key 可以通过以下方式提供：

- **`.env` 文件** —— 放在工作目录中（自动加载）
- **环境变量** —— `export ANTHROPIC_API_KEY=sk-ant-...`

### 完整配置参考

```json
{
  "mcp_servers": {},
  "timeouts": {
    "tool_timeout_secs": 120,
    "run_limit_secs": 3600
  },
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
  "writable_paths": ["./src/*"],
  "permissions": [
    {
      "operations": ["write"],
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

| 参数 | 默认值 | 说明 |
|--------|---------|-------------|
| `--agent-dir` | （必需） | Agent 文件夹路径 |
| `--prompt` | （必需） | 任务 prompt，或一个文本文件路径 |
| `--plan-only` | `false` | 只生成计划（`plan.json`）后退出，不执行 |
| `--max-iterations` | `50` | Agent 循环最大迭代次数 |
| `--output-dir` | `./agent-output` | 报告和 trace 的输出目录 |
| `--working-dir` | `.` | 文件系统/执行工具的工作目录 |
| `--writable-paths` | （来自配置） | 逗号分隔的可写路径 glob 模式（如 `./src/*,/tmp/*`） |
| `--tool-timeout` | `120` | 每次工具调用的超时秒数 |
| `--run-limit` | `3600` | 单次运行的最长总时长（秒） |
| `--verbose` | `false` | 向 stderr 打印迭代详情 |
| `--sandbox` | `false` | 无论配置如何都启用 shell 执行 |

### 退出码

| 退出码 | 含义 |
|------|---------|
| `0` | 任务完成 |
| `1` | 任务失败 |
| `2` | 达到最大迭代次数或运行时长上限 |
| `3` | 配置错误 |

## 内置工具

Agent 默认可以使用以下工具：

| 工具 | 说明 |
|------|-------------|
| `ls` | 列出目录条目 |
| `read_file` | 按行分页读取文件内容 |
| `write_file` | 写入文件（自动创建父目录） |
| `edit_file` | 在文件中查找并替换字符串 |
| `glob` | 按 glob 模式查找文件（遵循 `.gitignore`） |
| `grep` | 用正则搜索文件内容（遵循 `.gitignore`） |
| `execute` | 执行 shell 命令（启用时） |
| `read_plan` | 读取结构化执行计划（`plan.json`） |
| `update_plan` | 更新计划步骤状态（`pending`/`in_progress`/`done`/`skipped`） |
| `task_done` | 通知任务完成 |
| `write_todos` | 更新内部 todo 列表 |
| `compact_conversation` | 触发对话压缩 |

### 执行计划

当 `plan_required` 为 `true`（默认）时，agent 会先生成一份结构化执行计划并保存为 `plan.json`：

```json
{
  "task": "重构 auth 模块",
  "created_at": "2026-08-10T12:00:00Z",
  "steps": [
    { "id": 1, "description": "阅读 auth 模块", "status": "done" },
    { "id": 2, "description": "补充测试", "status": "in_progress" },
    { "id": 3, "description": "运行测试", "status": "pending" }
  ]
}
```

执行过程中，agent 通过 `read_plan` 查看计划与各步骤状态，通过 `update_plan` 标记进度，因此 `plan.json` 始终反映最新的执行状态。

### 权限

默认情况下**所有路径只读** —— `ls`、`read_file`、`glob`、`grep` 在任何位置都可用。写操作（`write_file`、`edit_file`、`execute`）只允许作用于匹配 `writable_paths` 的路径：

```json
{
  "writable_paths": ["./src/*", "./tests/*", "/tmp/out/*"]
}
```

模式使用 glob 语法（`**` 递归、`*` 单层、`?` 单字符）。如需高级 allow/deny 规则，仍支持旧的 `permissions` 数组作为兜底：

```json
{
  "writable_paths": ["./src/*"],
  "permissions": [
    { "operations": ["write"], "paths": ["./secrets/*"], "mode": "deny" }
  ]
}
```

`writable_paths` 也可以通过 `--writable-paths` CLI 参数设置（逗号分隔）；CLI 的值会追加在配置值之上。

## 输出

执行完成后，输出目录包含：

| 文件 | 说明 |
|------|-------------|
| `run.json` | 详细运行日志，含每次迭代和每个工具的 TAT、错误与异常 |
| `plan.json` | 结构化执行计划（含各步骤状态，agent 可读可写） |
| `report.json` | 状态、token 用量、迭代次数、耗时、todos |
| `transcript.json` | 完整消息历史 |
| `trace.jsonl` | 结构化事件日志（每行一个 JSON 对象） |

### run.json

每次运行都会生成一份包含完整调试信息的 `run.json`：

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
      "llm_tat_ms": 3200,
      "llm_input_tokens": 1200,
      "llm_output_tokens": 340,
      "tool_calls": [
        {
          "tool": "read_file",
          "arguments": {"file_path": "src/main.rs"},
          "tat_ms": 12,
          "is_error": false,
          "timed_out": false
        }
      ]
    }
  ],
  "errors": []
}
```

## 与其他工具的对比

这些工具都运行 LLM agent 循环，但它们服务于工作中不同的时刻。区别不在于谁"更好"，而在于**人在不在回路里**，以及 **agent 为一次会话而存在，还是为一个任务而存在**。

| | agent-runner | Claude Code | Codex CLI | OpenClaw | Hermes Agent |
|---|---|---|---|---|---|
| **交互模型** | 非交互，跑完即退 | 交互式 TUI（另有 `-p` 无头模式） | 交互式 TUI（另有 `codex exec`） | 常驻守护进程，聊天驱动 | 交互式 TUI + gateway 守护进程 |
| **运行时** | Rust，单个静态二进制 | 原生二进制 / npm（Node ≥ 22） | Rust（`codex-rs`） | TypeScript / Node.js | Python |
| **生命周期** | 一个任务，然后退出 | 一个工作会话 | 一个工作会话 | 永久运行 | 永久运行 |
| **主要定位** | 自动化与流水线 | 终端前的开发者 | 终端前的开发者 / 云端委派 | 个人助理 | 个人/网关助理 + 研究 |
| **agent 的定义方式** | 一个文件夹（`AGENTS.md` + skills + MCP） | 仓库上下文 + CLAUDE.md | 仓库上下文 + AGENTS.md | 开箱即用的助理，在对话中配置 | 逐步积累的 skills/记忆 |
| **沙箱姿态** | 默认只读；仅 `writable_paths` 可写 | 默认权限询问 | 审批模式 / 沙箱参数 | 拥有用户设备权限 | 运行在你提供的 VPS/容器上 |

### 一句话总结

**agent-runner vs. Claude Code / Codex CLI。** Claude Code 和 Codex CLI 是给坐在终端前的开发者使用的交互式产品：你指挥，它响应，会话是工作的基本单位。两者确实都提供无头模式（`claude -p`、`codex exec`），但那些只是交互式工具的次级入口。agent-runner 把优先级倒转过来 —— 批处理调用是**唯一**模式。如果你打算在 cron 任务里跑 `claude -p`，你正是我们的目标用户；如果你想找一个下午一起重构的协作者，那你需要的是 Claude Code 或 Codex。

**agent-runner vs. OpenClaw。** OpenClaw 是一个常驻的个人助理：一个 gateway 进程活在你自己的硬件上，通过 WhatsApp、Telegram、Slack 等 20 多个渠道与你对话。它有对话、有持久化、有状态 —— 你发消息，它记得。agent-runner 没有消息渠道、运行之间没有记忆、没有守护进程。两者互补而非竞争：OpenClaw 回答的是"助理，我们边聊边把这件事办了"；agent-runner 回答的是"现在无人值守地跑完这个任务，然后退出"。

**agent-runner vs. Hermes Agent。** Hermes Agent（Nous Research）是一个 Python agent，把交互式 TUI 与消息 gateway、cron 自动化、subagent 和自我改进记忆结合在一起。和 OpenClaw 一样，它生来就是要做一个长期陪伴、与你共同成长的伙伴。agent-runner 的设计里没有记忆、也没有成长叙事：每次运行都是一个干净、可复现的任务，身份由文件夹定义，退出码确定。

**agent-runner 适合的场景：** 服务器、CI 或容器中的无人值守执行；面向大量输入的批处理；把一个领域专属 agent 打包成可版本化的文件夹；以及任何"没有人能回答权限询问"是硬性需求而非限制的工作负载。

**agent-runner 不适合的场景：** 结对编程、探索性工作、任何你想中途改变 agent 方向的场景，以及任何需要跨运行对话记忆的场景。

### 体积对比

| | agent-runner | Claude Code | Codex CLI | OpenClaw | Hermes Agent |
|---|---|---|---|---|---|
| 安装方式 | 拷贝一个二进制文件 | 安装脚本 / npm | npm / brew / 预编译包 | npm（Node 24+） | pip / git checkout |
| 运行时依赖 | 无 | Node.js（npm 路径） | 无（Rust 二进制） | Node.js | Python |
| 约二进制体积 | ~3 MB | ~80 MB（社区估计） | ~65–100 MB 压缩包 | ~389 MB 解包后（npm） | Python 包 |
| 成本模型 | 自己的 API key，按 token 付费 | 订阅或 API | 订阅或 API | 免费（MIT），自带模型访问 | 免费（MIT），自带模型访问 |

体积和安装占用均为近似值且经常变化 —— 引用前请以各项目当前 release 为准。agent-runner 自身的数字基于优化后的 `cargo build --release`。

## 工作原理

1. 加载 agent 文件夹（AGENTS.md、agent-runner.json、skills）—— 只从 `--agent-dir` 读取，绝不读取你的 home 目录
2. 当 `plan_required` 为 true 时生成结构化执行计划（`plan.json`）；agent 通过 `read_plan` / `update_plan` 跟踪进度
3. 运行自主循环：LLM 调用 → 权限检查 → 工具执行 → 重复
4. 上下文变长时自动压缩对话历史
5. 当 agent 调用 `task_done`，或达到最大迭代次数 / 运行时长上限时退出
6. 向输出目录写入 run.json、plan.json、report、transcript 和 trace 日志

## 许可证

MIT
