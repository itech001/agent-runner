use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
pub struct RunLog {
    pub status: String,
    pub exit_code: u32,
    pub prompt: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: u64,
    pub iterations: Vec<IterationLog>,
    pub errors: Vec<ErrorEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IterationLog {
    pub iteration: u32,
    pub started_at: String,
    pub llm_tat_ms: u64,
    pub llm_input_tokens: u32,
    pub llm_output_tokens: u32,
    pub llm_error: Option<String>,
    pub tool_calls: Vec<ToolCallLog>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCallLog {
    pub tool: String,
    pub arguments: serde_json::Value,
    pub result: String,
    pub tat_ms: u64,
    pub is_error: bool,
    pub error: Option<String>,
    pub permission_denied: Option<String>,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorEntry {
    pub iteration: u32,
    pub phase: String,
    pub message: String,
    pub timestamp: String,
}

pub struct RunLogger {
    log: Mutex<RunLog>,
    output_dir: std::path::PathBuf,
}

impl RunLogger {
    pub fn new(prompt: &str, output_dir: &std::path::PathBuf) -> Self {
        Self {
            log: Mutex::new(RunLog {
                status: "running".into(),
                exit_code: 99,
                prompt: prompt.into(),
                started_at: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                finished_at: String::new(),
                duration_ms: 0,
                iterations: Vec::new(),
                errors: Vec::new(),
            }),
            output_dir: output_dir.clone(),
        }
    }

    pub fn add_iteration(&self, iter_log: IterationLog) {
        let mut log = self.log.lock().unwrap();
        log.iterations.push(iter_log);
    }

    pub fn add_error(&self, iteration: u32, phase: &str, message: &str) {
        let mut log = self.log.lock().unwrap();
        log.errors.push(ErrorEntry {
            iteration,
            phase: phase.into(),
            message: message.into(),
            timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        });
    }

    pub fn finish(&self, status: &str, exit_code: u32) {
        let mut log = self.log.lock().unwrap();
        log.status = status.into();
        log.exit_code = exit_code;
        log.finished_at = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    }

    pub fn set_duration(&self, ms: u64) {
        let mut log = self.log.lock().unwrap();
        log.duration_ms = ms;
    }

    pub fn write_to_file(&self) -> Result<(), io::Error> {
        let log = self.log.lock().unwrap();
        fs::create_dir_all(&self.output_dir)?;
        let path = self.output_dir.join("run.json");
        let json = serde_json::to_string_pretty(&*log)?;
        fs::write(path, json)
    }
}
