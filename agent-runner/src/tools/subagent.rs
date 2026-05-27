use async_trait::async_trait;
use serde_json::Value;
use crate::config::SubAgentConfig;
use crate::provider::ToolDefinition;
use crate::tools::{Tool, ToolOutput};

pub struct SubAgentTool {
    configs: Vec<SubAgentConfig>,
}

#[async_trait]
impl Tool for SubAgentTool {
    fn name(&self) -> &str {
        "task"
    }

    fn definition(&self) -> ToolDefinition {
        let subagent_types: Vec<Value> = self
            .configs
            .iter()
            .map(|c| Value::String(c.name.clone()))
            .collect();

        ToolDefinition {
            name: "task".into(),
            description: "Delegate a sub-task to a specialized sub-agent.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "Description of the task to delegate"
                    },
                    "subagent_type": {
                        "type": "string",
                        "description": "The type of sub-agent to use",
                        "enum": subagent_types
                    }
                },
                "required": ["description", "subagent_type"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let description = args["description"].as_str().unwrap_or("");
        let subagent_type = args["subagent_type"].as_str().unwrap_or("");

        let config_exists = self.configs.iter().any(|c| c.name == subagent_type);
        if !config_exists {
            return ToolOutput {
                content: format!("Unknown sub-agent type: {}", subagent_type),
                is_error: true,
            };
        }

        ToolOutput {
            content: format!(
                "Sub-agent task queued: type={}, description={}",
                subagent_type, description
            ),
            is_error: false,
        }
    }
}

pub fn from_configs(configs: &[SubAgentConfig]) -> Vec<Box<dyn Tool>> {
    if configs.is_empty() {
        return vec![];
    }
    vec![Box::new(SubAgentTool {
        configs: configs.to_vec(),
    })]
}
