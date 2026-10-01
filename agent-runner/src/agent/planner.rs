use crate::provider::{Message, Provider};
use crate::trace::TraceLogger;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A single step in an execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: u32,
    pub description: String,
    pub status: String,
}

/// A structured execution plan, persisted as `plan.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub task: String,
    pub created_at: String,
    pub steps: Vec<PlanStep>,
}

pub struct Planner {
    provider: Arc<dyn Provider>,
    trace: Arc<TraceLogger>,
}

impl Planner {
    pub fn new(provider: Arc<dyn Provider>, trace: Arc<TraceLogger>) -> Self {
        Self { provider, trace }
    }

    pub async fn generate_plan(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tool_names: &[String],
    ) -> Result<String, String> {
        let plan_prompt = format!(
            "Given the task below, generate a step-by-step execution plan. \
             Each step should be a concrete action. Available tools: {}\n\n\
             Task: {}",
            tool_names.join(", "),
            user_prompt,
        );

        let messages = vec![
            Message::system(system_prompt.into()),
            Message::user(plan_prompt),
        ];

        let response = self
            .provider
            .complete(&messages, &[])
            .await
            .map_err(|e| format!("Plan generation failed: {}", e))?;

        let plan = response.content.unwrap_or_default();
        let step_count = plan
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();

        self.trace.log_plan(&plan, step_count);
        Ok(plan)
    }

    /// Parse the free-form plan text returned by the LLM into a structured `Plan`.
    /// Each non-empty line becomes a step with an incrementing id (starting at 1)
    /// and a default status of "pending".
    pub fn parse_plan(plan_text: &str, task: &str) -> Plan {
        let steps: Vec<PlanStep> = plan_text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .enumerate()
            .map(|(i, line)| PlanStep {
                id: (i + 1) as u32,
                description: line.trim_start_matches(|c| c == '-' || c == '*' || c == ' ' || c == '\t')
                    .trim()
                    .to_string(),
                status: "pending".to_string(),
            })
            .collect();

        Plan {
            task: task.to_string(),
            created_at: Utc::now().to_rfc3339(),
            steps,
        }
    }

    /// Persist the structured plan as `plan.json` in the output directory.
    pub fn save_plan_json(plan: &Plan, output_dir: &std::path::Path) -> Result<(), String> {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output dir: {}", e))?;
        let json = serde_json::to_string_pretty(plan)
            .map_err(|e| format!("Failed to serialize plan: {}", e))?;
        std::fs::write(output_dir.join("plan.json"), json)
            .map_err(|e| format!("Failed to write plan.json: {}", e))
    }
}
