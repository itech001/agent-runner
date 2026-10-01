use async_trait::async_trait;
use crate::agent::planner::Plan;
use crate::provider::ToolDefinition;
use crate::tools::{Tool, ToolOutput};
use std::path::PathBuf;

/// Tool that reads the structured `plan.json` so the agent can review the
/// current plan and the status of each step during the execution loop.
pub struct ReadPlanTool {
    plan_path: PathBuf,
}

impl ReadPlanTool {
    pub fn new(plan_path: PathBuf) -> Self {
        Self { plan_path }
    }
}

#[async_trait]
impl Tool for ReadPlanTool {
    fn name(&self) -> &str {
        "read_plan"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_plan".into(),
            description: "Read the execution plan (plan.json) including all steps and their current status.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _args: serde_json::Value) -> ToolOutput {
        match std::fs::read_to_string(&self.plan_path) {
            Ok(content) => ToolOutput {
                content,
                is_error: false,
            },
            Err(_) => ToolOutput {
                content: "No plan found. A plan is generated at startup when plan_required is true.".into(),
                is_error: false,
            },
        }
    }
}

/// Tool that updates a single step's status (and optionally description) in
/// `plan.json`. Lets the agent track progress as it works through the plan.
pub struct UpdatePlanTool {
    plan_path: PathBuf,
}

impl UpdatePlanTool {
    pub fn new(plan_path: PathBuf) -> Self {
        Self { plan_path }
    }
}

#[async_trait]
impl Tool for UpdatePlanTool {
    fn name(&self) -> &str {
        "update_plan"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "update_plan".into(),
            description: "Update the status of a plan step in plan.json. Use this to mark steps as in_progress, done, or skipped as you work.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "step_id": { "type": "integer", "description": "The id of the step to update" },
                    "status": { "type": "string", "enum": ["pending", "in_progress", "done", "skipped"], "description": "New status for the step" },
                    "description": { "type": "string", "description": "Optional: update the step description" }
                },
                "required": ["step_id", "status"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let step_id = match args["step_id"].as_u64() {
            Some(id) => id as u32,
            None => {
                return ToolOutput {
                    content: "Missing or invalid 'step_id'".into(),
                    is_error: true,
                }
            }
        };
        let status = match args["status"].as_str() {
            Some(s) => s.to_string(),
            None => {
                return ToolOutput {
                    content: "Missing or invalid 'status'".into(),
                    is_error: true,
                }
            }
        };

        // Validate status value.
        let valid = ["pending", "in_progress", "done", "skipped"];
        if !valid.contains(&status.as_str()) {
            return ToolOutput {
                content: format!(
                    "Invalid status '{}'. Must be one of: {}",
                    status,
                    valid.join(", ")
                ),
                is_error: true,
            };
        }

        let new_description = args["description"].as_str().map(|s| s.to_string());

        // Read current plan.
        let content = match std::fs::read_to_string(&self.plan_path) {
            Ok(c) => c,
            Err(_) => {
                return ToolOutput {
                    content: "No plan found. Cannot update a non-existent plan.".into(),
                    is_error: true,
                }
            }
        };

        let mut plan: Plan = match serde_json::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                return ToolOutput {
                    content: format!("Failed to parse plan.json: {}", e),
                    is_error: true,
                }
            }
        };

        // Find and update the matching step.
        let mut found = None;
        for step in &mut plan.steps {
            if step.id == step_id {
                step.status = status.clone();
                if let Some(desc) = &new_description {
                    step.description = desc.clone();
                }
                found = Some(step.clone());
                break;
            }
        }

        let updated_step = match found {
            Some(s) => s,
            None => {
                return ToolOutput {
                    content: format!("Step with id {} not found in plan", step_id),
                    is_error: true,
                }
            }
        };

        // Write back.
        let json = match serde_json::to_string_pretty(&plan) {
            Ok(j) => j,
            Err(e) => {
                return ToolOutput {
                    content: format!("Failed to serialize updated plan: {}", e),
                    is_error: true,
                }
            }
        };
        if let Err(e) = std::fs::write(&self.plan_path, json) {
            return ToolOutput {
                content: format!("Failed to write plan.json: {}", e),
                is_error: true,
            };
        }

        ToolOutput {
            content: serde_json::to_string_pretty(&updated_step).unwrap_or_default(),
            is_error: false,
        }
    }
}
