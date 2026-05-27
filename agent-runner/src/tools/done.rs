use async_trait::async_trait;
use crate::provider::ToolDefinition;
use crate::tools::{Tool, ToolOutput};

pub struct TaskDoneTool;

#[async_trait]
impl Tool for TaskDoneTool {
    fn name(&self) -> &str {
        "task_done"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_done".into(),
            description: "Signal that the current task is complete.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "Summary of what was accomplished" },
                    "success": { "type": "boolean", "description": "Whether the task succeeded" }
                },
                "required": ["summary", "success"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let summary = args["summary"].as_str().unwrap_or("");
        let success = args["success"].as_bool().unwrap_or(false);
        ToolOutput {
            content: format!(
                "Task completed. Success: {}. Summary: {}",
                success, summary
            ),
            is_error: false,
        }
    }
}
