use crate::config::SubAgentConfig;
use crate::provider::{Message, Provider, ToolDefinition};
use crate::tools::Tool;
use crate::trace::TraceLogger;
use std::sync::Arc;

pub struct SubAgentRunner {
    config: SubAgentConfig,
    provider: Arc<dyn Provider>,
    tools: Vec<Box<dyn Tool>>,
    trace: Arc<TraceLogger>,
}

impl SubAgentRunner {
    pub fn new(
        config: SubAgentConfig,
        provider: Arc<dyn Provider>,
        tools: Vec<Box<dyn Tool>>,
        trace: Arc<TraceLogger>,
    ) -> Self {
        Self {
            config,
            provider,
            tools,
            trace,
        }
    }

    pub async fn run(&self, description: &str, parent_iteration: u32) -> Result<String, String> {
        let max_iterations = 50u32;
        let mut messages = vec![
            Message::system(self.config.system_prompt.clone()),
            Message::user(description.to_string()),
        ];

        let tool_definitions: Vec<ToolDefinition> = self.tools.iter().map(|t| t.definition()).collect();

        for iteration in 0..max_iterations {
            self.trace.log(
                "subagent_iteration",
                serde_json::json!({
                    "parent_iteration": parent_iteration,
                    "subagent_iteration": iteration,
                    "subagent_type": self.config.name,
                }),
            );

            let response = self
                .provider
                .complete(&messages, &tool_definitions)
                .await
                .map_err(|e| format!("Sub-agent LLM error: {}", e))?;

            self.trace.log(
                "subagent_llm_response",
                serde_json::json!({
                    "parent_iteration": parent_iteration,
                    "subagent_iteration": iteration,
                    "output_tokens": response.usage.output_tokens,
                    "tool_calls": response.tool_calls.iter().map(|tc| tc.name.clone()).collect::<Vec<_>>(),
                }),
            );

            messages.push(Message::assistant(response.content.clone(), response.tool_calls.clone()));

            if response.done || response.tool_calls.is_empty() {
                let final_content = response.content.unwrap_or_default();
                return Ok(final_content);
            }

            for tool_call in &response.tool_calls {
                self.trace.log(
                    "subagent_tool_call",
                    serde_json::json!({
                        "parent_iteration": parent_iteration,
                        "subagent_iteration": iteration,
                        "tool": tool_call.name,
                        "input": tool_call.arguments.to_string(),
                    }),
                );

                let tool_result = if let Some(tool) = self.tools.iter().find(|t| t.name() == tool_call.name) {
                    tool.execute(tool_call.arguments.clone()).await
                } else {
                    crate::tools::ToolOutput {
                        content: format!("Unknown tool: {}", tool_call.name),
                        is_error: true,
                    }
                };

                self.trace.log(
                    "subagent_tool_result",
                    serde_json::json!({
                        "parent_iteration": parent_iteration,
                        "subagent_iteration": iteration,
                        "tool": tool_call.name,
                        "output": tool_result.content,
                        "is_error": tool_result.is_error,
                    }),
                );

                messages.push(Message::tool_result(tool_call.id.clone(), tool_result.content));
            }
        }

        Err(format!(
            "Sub-agent '{}' exceeded max iterations ({})",
            self.config.name, max_iterations
        ))
    }
}
