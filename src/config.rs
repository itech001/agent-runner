use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub llm: LlmConfig,
    #[serde(default)]
    pub summarization: SummarizationConfig,
    #[serde(default)]
    pub permissions: Vec<FilesystemPermission>,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub subagents: Vec<SubAgentConfig>,
    #[serde(default)]
    pub agent: AgentConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_max_tokens() -> u32 {
    4096
}
fn default_temperature() -> f32 {
    0.7
}

#[derive(Debug, Deserialize, Clone)]
pub struct SummarizationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "default_trigger_tokens")]
    pub trigger_tokens: u32,
    #[serde(default = "default_keep_tokens")]
    pub keep_tokens: u32,
    #[serde(default = "default_trim_tokens")]
    pub trim_tokens: u32,
}

impl Default for SummarizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model: None,
            trigger_tokens: 80000,
            keep_tokens: 20000,
            trim_tokens: 4000,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_trigger_tokens() -> u32 {
    80000
}
fn default_keep_tokens() -> u32 {
    20000
}
fn default_trim_tokens() -> u32 {
    4000
}

#[derive(Debug, Deserialize, Clone)]
pub struct FilesystemPermission {
    pub operations: Vec<String>,
    pub paths: Vec<String>,
    pub mode: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubAgentConfig {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    #[serde(default)]
    pub skills: Option<Vec<String>>,
    #[serde(default)]
    pub permissions: Option<Vec<FilesystemPermission>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    #[serde(default = "default_true")]
    pub plan_required: bool,
    #[serde(default = "default_tool_output_limit")]
    pub tool_output_token_limit: u32,
    #[serde(default = "default_user_message_limit")]
    pub user_message_token_limit: u32,
    #[serde(default = "default_execute_timeout")]
    pub execute_timeout_secs: u64,
    #[serde(default)]
    pub execute_enabled: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            plan_required: true,
            tool_output_token_limit: 20000,
            user_message_token_limit: 50000,
            execute_timeout_secs: 3600,
            execute_enabled: false,
        }
    }
}

fn default_max_iterations() -> u32 {
    50
}
fn default_tool_output_limit() -> u32 {
    20000
}
fn default_user_message_limit() -> u32 {
    50000
}
fn default_execute_timeout() -> u64 {
    3600
}

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
        let config: Config =
            serde_json::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?;
        Ok(config)
    }

    pub fn get_api_key(&self) -> Result<String, ConfigError> {
        std::env::var(&self.llm.api_key_env)
            .map_err(|_| ConfigError::MissingApiKey(self.llm.api_key_env.clone()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Missing API key env var: {0}")]
    MissingApiKey(String),
}
