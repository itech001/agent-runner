use crate::provider::{Message, Provider};
use crate::trace::TraceLogger;
use std::sync::Arc;

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

    pub fn save_plan(plan: &str, output_dir: &std::path::Path) -> Result<(), String> {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output dir: {}", e))?;
        std::fs::write(output_dir.join("plan.md"), plan)
            .map_err(|e| format!("Failed to write plan: {}", e))
    }
}
