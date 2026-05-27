use async_trait::async_trait;
use crate::provider::ToolDefinition;
use crate::tools::{Tool, ToolOutput};
use std::path::PathBuf;
use std::time::Duration;

pub struct ExecuteTool {
    working_dir: PathBuf,
    default_timeout: u64,
}

impl ExecuteTool {
    pub fn new(working_dir: PathBuf, default_timeout: u64) -> Self {
        Self {
            working_dir,
            default_timeout,
        }
    }
}

#[async_trait]
impl Tool for ExecuteTool {
    fn name(&self) -> &str {
        "execute"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "execute".into(),
            description: "Execute a shell command with optional timeout.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute" },
                    "timeout": { "type": "integer", "description": "Timeout in seconds (optional)" }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let command = match args["command"].as_str() {
            Some(c) => c,
            None => {
                return ToolOutput {
                    content: "Missing required argument: command".into(),
                    is_error: true,
                }
            }
        };
        let timeout_secs = args["timeout"].as_u64().unwrap_or(self.default_timeout);

        let result = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&self.working_dir)
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);
                ToolOutput {
                    content: format!(
                        "Exit code: {}\nStdout:\n{}\nStderr:\n{}",
                        exit_code, stdout, stderr
                    ),
                    is_error: !output.status.success(),
                }
            }
            Ok(Err(e)) => ToolOutput {
                content: format!("Failed to execute command: {}", e),
                is_error: true,
            },
            Err(_) => ToolOutput {
                content: format!("Command timed out after {} seconds", timeout_secs),
                is_error: true,
            },
        }
    }
}
