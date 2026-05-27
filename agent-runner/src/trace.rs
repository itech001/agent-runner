use chrono::Utc;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub struct TraceLogger {
    writer: Mutex<BufWriter<File>>,
    seq: AtomicU64,
}

impl TraceLogger {
    pub fn new(output_dir: &Path) -> Result<Self, io::Error> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("trace.jsonl");
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
            seq: AtomicU64::new(0),
        })
    }

    pub fn log(&self, event: &str, data: Value) {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let mut entry = match data {
            Value::Object(map) => map,
            _ => {
                let mut map = serde_json::Map::new();
                map.insert("data".into(), data);
                map
            }
        };
        entry.insert("seq".into(), Value::Number(seq.into()));
        entry.insert("timestamp".into(), Value::String(timestamp));
        entry.insert("event".into(), Value::String(event.into()));
        let line = serde_json::Value::Object(entry).to_string();
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writeln!(writer, "{}", line);
            let _ = writer.flush();
        }
    }

    pub fn log_init(&self, agent_dir: &str, model: &str, tools: &[String], skills: &[String]) {
        self.log(
            "init",
            serde_json::json!({
                "agent_dir": agent_dir,
                "model": model,
                "tools": tools,
                "skills": skills,
            }),
        );
    }

    pub fn log_plan(&self, content: &str, todos_created: usize) {
        self.log(
            "plan",
            serde_json::json!({
                "content": content,
                "todos_created": todos_created,
            }),
        );
    }

    pub fn log_llm_request(&self, iteration: u32, input_tokens: u32) {
        self.log(
            "llm_request",
            serde_json::json!({
                "iteration": iteration,
                "input_tokens": input_tokens,
            }),
        );
    }

    pub fn log_llm_response(
        &self,
        iteration: u32,
        output_tokens: u32,
        reasoning: Option<&str>,
        tool_calls: &[String],
    ) {
        self.log(
            "llm_response",
            serde_json::json!({
                "iteration": iteration,
                "output_tokens": output_tokens,
                "reasoning": reasoning,
                "tool_calls": tool_calls,
            }),
        );
    }

    pub fn log_tool_call(&self, iteration: u32, tool: &str, input: &str) {
        self.log(
            "tool_call",
            serde_json::json!({
                "iteration": iteration,
                "tool": tool,
                "input": input,
            }),
        );
    }

    pub fn log_tool_result(
        &self,
        iteration: u32,
        tool: &str,
        output: &str,
        duration_ms: u64,
        truncated: bool,
    ) {
        self.log(
            "tool_result",
            serde_json::json!({
                "iteration": iteration,
                "tool": tool,
                "output": output,
                "duration_ms": duration_ms,
                "truncated": truncated,
            }),
        );
    }

    pub fn log_permission_denied(
        &self,
        iteration: u32,
        tool: &str,
        path: &str,
        operations: &[String],
    ) {
        self.log(
            "permission_denied",
            serde_json::json!({
                "iteration": iteration,
                "tool": tool,
                "path": path,
                "operations": operations,
            }),
        );
    }

    pub fn log_summarization(&self, iteration: u32, tokens_before: u32, tokens_after: u32) {
        self.log(
            "summarization",
            serde_json::json!({
                "iteration": iteration,
                "tokens_before": tokens_before,
                "tokens_after": tokens_after,
            }),
        );
    }

    pub fn log_todo_update(&self, todos: &[Value]) {
        self.log(
            "todo_update",
            serde_json::json!({
                "todos": todos,
            }),
        );
    }

    pub fn log_task_done(&self, summary: &str, success: bool) {
        self.log(
            "task_done",
            serde_json::json!({
                "summary": summary,
                "success": success,
            }),
        );
    }
}
