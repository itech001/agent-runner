use async_trait::async_trait;
use crate::provider::ToolDefinition;
use crate::skills::Skill;
use crate::tools::{Tool, ToolOutput};
use std::path::PathBuf;
use std::time::Duration;

pub struct SkillScriptTool {
    tool_name: String,
    script_path: PathBuf,
    description: String,
}

impl SkillScriptTool {
    pub fn new(tool_name: String, script_path: PathBuf, description: String) -> Self {
        Self {
            tool_name,
            script_path,
            description,
        }
    }
}

#[async_trait]
impl Tool for SkillScriptTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.tool_name.clone(),
            description: self.description.clone(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "args": { "type": "string", "description": "Arguments to pass to the script" }
                }
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let script_str = self.script_path.to_string_lossy().to_string();
        let extra_args = args["args"].as_str().unwrap_or("");

        let command = if extra_args.is_empty() {
            script_str
        } else {
            format!("{} {}", script_str, extra_args)
        };

        let result = tokio::time::timeout(
            Duration::from_secs(60),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
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
                content: format!("Failed to execute script: {}", e),
                is_error: true,
            },
            Err(_) => ToolOutput {
                content: "Script timed out after 60 seconds".to_string(),
                is_error: true,
            },
        }
    }
}

pub fn from_skills(skills: &[Skill]) -> Vec<Box<dyn Tool>> {
    let mut tools: Vec<Box<dyn Tool>> = Vec::new();
    for skill in skills {
        for script_path in &skill.scripts {
            let filename = script_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let tool_name = format!("skill_{}_{}", skill.name, filename);
            let description = format!("Execute skill script: {} ({})", skill.name, filename);
            tools.push(Box::new(SkillScriptTool::new(
                tool_name,
                script_path.clone(),
                description,
            )));
        }
    }
    tools
}
