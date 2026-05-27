use crate::provider::Message;
use crate::tools::todos::TodoItem;
use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct Report {
    pub status: String,
    pub exit_code: u32,
    pub prompt: String,
    pub plan: String,
    pub todos_final: Vec<TodoItem>,
    pub subagent_runs: u32,
    pub metrics: Metrics,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct Metrics {
    pub total_iterations: u32,
    pub total_tool_calls: u32,
    pub total_tokens: TokenMetrics,
    pub summarization_runs: u32,
    pub tokens_saved_by_summarization: u32,
    pub permission_denials: u32,
    pub duration_secs: f64,
}

#[derive(Debug, Serialize)]
pub struct TokenMetrics {
    pub input: u32,
    pub output: u32,
}

impl Report {
    pub fn write_to_file(&self, output_dir: &Path) -> Result<(), io::Error> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("report.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)
    }
}

pub fn write_transcript(messages: &[Message], output_dir: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(output_dir)?;
    let path = output_dir.join("transcript.json");
    let json = serde_json::to_string_pretty(messages)?;
    fs::write(path, json)
}

impl Report {
    pub fn new(
        status: String,
        exit_code: u32,
        prompt: String,
        plan: String,
        todos_final: Vec<TodoItem>,
        subagent_runs: u32,
        metrics: Metrics,
    ) -> Self {
        Self {
            status,
            exit_code,
            prompt,
            plan,
            todos_final,
            subagent_runs,
            metrics,
            created_at: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        }
    }
}
