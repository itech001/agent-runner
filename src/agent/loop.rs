use crate::permissions::PermissionEvaluator;
use crate::provider::{Message, Provider, ToolCall, ToolDefinition};
use crate::summarization::Summarizer;
use crate::tools::compact::CompactTool;
use crate::tools::todos::TodosTool;
use crate::tools::Tool;
use crate::trace::TraceLogger;
use std::sync::Arc;
use std::time::Instant;

pub struct LoopResult {
    pub final_text: String,
    pub status: String,
    pub exit_code: u32,
    pub total_iterations: u32,
    pub total_tool_calls: u32,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub permission_denials: u32,
    pub duration_secs: f64,
}

pub async fn run_loop(
    provider: Arc<dyn Provider>,
    tools: Vec<Box<dyn Tool>>,
    messages: &mut Vec<Message>,
    max_iterations: u32,
    summarizer: &mut Summarizer,
    evaluator: PermissionEvaluator,
    trace: Arc<TraceLogger>,
    compact_tool: Arc<CompactTool>,
    _todos_tool: Arc<TodosTool>,
    verbose: bool,
) -> LoopResult {
    let start = Instant::now();
    let mut total_tool_calls: u32 = 0;
    let mut total_input_tokens: u32 = 0;
    let mut total_output_tokens: u32 = 0;
    let mut permission_denials: u32 = 0;
    let mut iteration: u32 = 0;

    let tool_defs: Vec<ToolDefinition> = tools.iter().map(|t| t.definition()).collect();

    loop {
        iteration += 1;
        if iteration > max_iterations {
            return LoopResult {
                final_text: "Max iterations reached".into(),
                status: "max_iterations_reached".into(),
                exit_code: 2,
                total_iterations: max_iterations,
                total_tool_calls,
                total_input_tokens,
                total_output_tokens,
                permission_denials,
                duration_secs: start.elapsed().as_secs_f64(),
            };
        }

        if summarizer.should_summarize(messages) {
            let _ = summarizer.summarize(messages, iteration).await;
        }

        {
            let flag = compact_tool.triggered.lock().unwrap();
            if *flag {
                drop(flag);
                {
                    let mut flag = compact_tool.triggered.lock().unwrap();
                    *flag = false;
                }
                let _ = summarizer.summarize(messages, iteration).await;
            }
        }

        if verbose {
            eprintln!("--- Iteration {} ---", iteration);
        }

        trace.log_llm_request(iteration, 0);

        let response = match provider.complete(messages, &tool_defs).await {
            Ok(r) => r,
            Err(e) => {
                return LoopResult {
                    final_text: format!("LLM error: {}", e),
                    status: "failed".into(),
                    exit_code: 1,
                    total_iterations: iteration,
                    total_tool_calls,
                    total_input_tokens,
                    total_output_tokens,
                    permission_denials,
                    duration_secs: start.elapsed().as_secs_f64(),
                };
            }
        };

        total_input_tokens += response.usage.input_tokens;
        total_output_tokens += response.usage.output_tokens;

        let tc_names: Vec<String> = response
            .tool_calls
            .iter()
            .map(|tc| tc.name.clone())
            .collect();
        trace.log_llm_response(
            iteration,
            response.usage.output_tokens,
            response.reasoning.as_deref(),
            &tc_names,
        );

        messages.push(Message::assistant(response.content.clone(), response.tool_calls.clone()));

        if response.tool_calls.is_empty() {
            let final_text = response.content.unwrap_or_default();
            trace.log_task_done(&final_text, true);
            return LoopResult {
                final_text,
                status: "completed".into(),
                exit_code: 0,
                total_iterations: iteration,
                total_tool_calls,
                total_input_tokens,
                total_output_tokens,
                permission_denials,
                duration_secs: start.elapsed().as_secs_f64(),
            };
        }

        for tc in &response.tool_calls {
            total_tool_calls += 1;
            let input_str = tc.arguments.to_string();
            trace.log_tool_call(iteration, &tc.name, &input_str);

            if let Some(op) = tool_operation(&tc.name) {
                let path = tc.arguments["file_path"]
                    .as_str()
                    .or_else(|| tc.arguments["path"].as_str())
                    .unwrap_or("");
                if !evaluator.check(op, path) {
                    permission_denials += 1;
                    trace.log_permission_denied(
                        iteration,
                        &tc.name,
                        path,
                        &[op.to_string()],
                    );
                    messages.push(Message::tool_result(
                        tc.id.clone(),
                        format!("Permission denied: {} access to {}", op, path),
                    ));
                    continue;
                }
            }

            let tool_start = Instant::now();
            let output = execute_tool(&tools, tc).await;
            let duration_ms = tool_start.elapsed().as_millis() as u64;

            let truncated = false;
            trace.log_tool_result(iteration, &tc.name, &output.content, duration_ms, truncated);
            messages.push(Message::tool_result(tc.id.clone(), output.content));

            if verbose {
                eprintln!("  Tool {}: {}ms", tc.name, duration_ms);
            }
        }

        if response.done {
            let summary = response
                .tool_calls
                .iter()
                .find(|tc| tc.name == "task_done")
                .and_then(|tc| tc.arguments["summary"].as_str())
                .unwrap_or("Task completed");
            let success = response
                .tool_calls
                .iter()
                .find(|tc| tc.name == "task_done")
                .and_then(|tc| tc.arguments["success"].as_bool())
                .unwrap_or(true);
            trace.log_task_done(summary, success);

            return LoopResult {
                final_text: summary.into(),
                status: if success {
                    "completed".into()
                } else {
                    "failed".into()
                },
                exit_code: if success { 0 } else { 1 },
                total_iterations: iteration,
                total_tool_calls,
                total_input_tokens,
                total_output_tokens,
                permission_denials,
                duration_secs: start.elapsed().as_secs_f64(),
            };
        }
    }
}

fn tool_operation(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "ls" | "read_file" | "glob" | "grep" => Some("read"),
        "write_file" | "edit_file" | "execute" => Some("write"),
        _ => None,
    }
}

async fn execute_tool(tools: &[Box<dyn Tool>], tc: &ToolCall) -> crate::tools::ToolOutput {
    let tool = tools.iter().find(|t| t.name() == tc.name);
    match tool {
        Some(t) => t.execute(tc.arguments.clone()).await,
        None => crate::tools::ToolOutput {
            content: format!("Unknown tool: {}", tc.name),
            is_error: true,
        },
    }
}
