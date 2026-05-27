use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub summarization: SummarizationConfig,
    #[serde(default)]
    pub permissions: Vec<FilesystemPermission>,
    #[serde(default)]
    pub subagents: Vec<SubAgentConfig>,
    #[serde(default)]
    pub agent: AgentConfig,
}

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
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
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
        let config: Config =
            serde_json::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?;
        Ok(config)
    }

    pub fn load_llm() -> Result<LlmConfig, ConfigError> {
        let provider = std::env::var("LLM_PROVIDER")
            .map_err(|_| ConfigError::MissingEnv("LLM_PROVIDER".into()))?;
        let model =
            std::env::var("LLM_MODEL").map_err(|_| ConfigError::MissingEnv("LLM_MODEL".into()))?;

        let api_key = std::env::var("LLM_API_KEY")
            .or_else(|_| match provider.as_str() {
                "anthropic" => std::env::var("ANTHROPIC_API_KEY"),
                "openai" => std::env::var("OPENAI_API_KEY"),
                _ => Err(std::env::VarError::NotPresent),
            })
            .map_err(|_| {
                ConfigError::MissingApiKey(format!(
                    "LLM_API_KEY or {}_API_KEY",
                    provider.to_uppercase()
                ))
            })?;

        let base_url = std::env::var("LLM_BASE_URL").ok();
        let max_tokens = std::env::var("LLM_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4096);
        let temperature = std::env::var("LLM_TEMPERATURE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.7);

        Ok(LlmConfig {
            provider,
            model,
            api_key,
            base_url,
            max_tokens,
            temperature,
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mcp_servers: HashMap::new(),
            summarization: SummarizationConfig::default(),
            permissions: Vec::new(),
            subagents: Vec::new(),
            agent: AgentConfig::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Missing environment variable: {0}")]
    MissingEnv(String),
    #[error("Missing API key: {0}")]
    MissingApiKey(String),
}
