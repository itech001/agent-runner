use crate::permissions::PermissionEvaluator;
use crate::provider::{Message, Provider, ToolCall, ToolDefinition};
use crate::run_log::{IterationLog, RunLogger, ToolCallLog};
use crate::summarization::Summarizer;
use crate::tools::compact::CompactTool;
use crate::tools::todos::TodosTool;
use crate::tools::Tool;
use crate::trace::TraceLogger;
use std::sync::Arc;
use std::time::{Duration, Instant};

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

pub struct LoopConfig {
    pub max_iterations: u32,
    pub tool_timeout_secs: u64,
    pub run_limit_secs: u64,
    pub verbose: bool,
}

pub async fn run_loop(
    provider: Arc<dyn Provider>,
    tools: Vec<Box<dyn Tool>>,
    messages: &mut Vec<Message>,
    config: LoopConfig,
    summarizer: &mut Summarizer,
    evaluator: PermissionEvaluator,
    trace: Arc<TraceLogger>,
    compact_tool: Arc<CompactTool>,
    _todos_tool: Arc<TodosTool>,
    run_logger: Arc<RunLogger>,
) -> LoopResult {
    let start = Instant::now();
    let run_deadline = start + Duration::from_secs(config.run_limit_secs);
    let tool_timeout = Duration::from_secs(config.tool_timeout_secs);
    let mut total_tool_calls: u32 = 0;
    let mut total_input_tokens: u32 = 0;
    let mut total_output_tokens: u32 = 0;
    let mut permission_denials: u32 = 0;
    let mut iteration: u32 = 0;

    let tool_defs: Vec<ToolDefinition> = tools.iter().map(|t| t.definition()).collect();

    loop {
        iteration += 1;

        if let Some(remaining) = run_deadline.checked_duration_since(Instant::now()) {
            if remaining.is_zero() {
                let msg = format!("Run limit of {}s exceeded", config.run_limit_secs);
                run_logger.add_error(iteration, "run_limit", &msg);
                let result = LoopResult {
                    final_text: msg.clone(),
                    status: "run_limit_exceeded".into(),
                    exit_code: 2,
                    total_iterations: iteration,
                    total_tool_calls,
                    total_input_tokens,
                    total_output_tokens,
                    permission_denials,
                    duration_secs: start.elapsed().as_secs_f64(),
                };
                run_logger.finish(&result.status, result.exit_code);
                run_logger.set_duration(start.elapsed().as_millis() as u64);
                return result;
            }
        } else {
            let msg = format!("Run limit of {}s exceeded", config.run_limit_secs);
            run_logger.add_error(iteration, "run_limit", &msg);
            let result = LoopResult {
                final_text: msg.clone(),
                status: "run_limit_exceeded".into(),
                exit_code: 2,
                total_iterations: iteration,
                total_tool_calls,
                total_input_tokens,
                total_output_tokens,
                permission_denials,
                duration_secs: start.elapsed().as_secs_f64(),
            };
            run_logger.finish(&result.status, result.exit_code);
            run_logger.set_duration(start.elapsed().as_millis() as u64);
            return result;
        }

        if iteration > config.max_iterations {
            let result = LoopResult {
                final_text: "Max iterations reached".into(),
                status: "max_iterations_reached".into(),
                exit_code: 2,
                total_iterations: config.max_iterations,
                total_tool_calls,
                total_input_tokens,
                total_output_tokens,
                permission_denials,
                duration_secs: start.elapsed().as_secs_f64(),
            };
            run_logger.finish(&result.status, result.exit_code);
            run_logger.set_duration(start.elapsed().as_millis() as u64);
            return result;
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

        if config.verbose {
            eprintln!("--- Iteration {} ---", iteration);
        }

        trace.log_llm_request(iteration, 0);

        let llm_start = Instant::now();
        let mut iter_log = IterationLog {
            iteration,
            started_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            llm_tat_ms: 0,
            llm_input_tokens: 0,
            llm_output_tokens: 0,
            llm_error: None,
            tool_calls: Vec::new(),
        };

        let response = match provider.complete(messages, &tool_defs).await {
            Ok(r) => r,
            Err(e) => {
                let err_msg = format!("LLM error: {}", e);
                iter_log.llm_error = Some(err_msg.clone());
                run_logger.add_error(iteration, "llm_call", &err_msg);
                run_logger.add_iteration(iter_log);
                let result = LoopResult {
                    final_text: err_msg,
                    status: "failed".into(),
                    exit_code: 1,
                    total_iterations: iteration,
                    total_tool_calls,
                    total_input_tokens,
                    total_output_tokens,
                    permission_denials,
                    duration_secs: start.elapsed().as_secs_f64(),
                };
                run_logger.finish(&result.status, result.exit_code);
                run_logger.set_duration(start.elapsed().as_millis() as u64);
                return result;
            }
        };

        iter_log.llm_tat_ms = llm_start.elapsed().as_millis() as u64;
        iter_log.llm_input_tokens = response.usage.input_tokens;
        iter_log.llm_output_tokens = response.usage.output_tokens;

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
            run_logger.add_iteration(iter_log);
            let result = LoopResult {
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
            run_logger.finish(&result.status, result.exit_code);
            run_logger.set_duration(start.elapsed().as_millis() as u64);
            return result;
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
                    let denied_msg = format!("Permission denied: {} access to {}", op, path);
                    trace.log_permission_denied(iteration, &tc.name, path, &[op.to_string()]);
                    run_logger.add_error(iteration, "permission", &denied_msg);
                    iter_log.tool_calls.push(ToolCallLog {
                        tool: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        result: denied_msg.clone(),
                        tat_ms: 0,
                        is_error: true,
                        error: None,
                        permission_denied: Some(denied_msg.clone()),
                        timed_out: false,
                    });
                    messages.push(Message::tool_result(tc.id.clone(), denied_msg));
                    continue;
                }
            }

            let tool_start = Instant::now();
            let output = match tokio::time::timeout(
                tool_timeout,
                execute_tool(&tools, tc),
            )
            .await
            {
                Ok(out) => out,
                Err(_) => {
                    let timeout_msg = format!(
                        "Tool '{}' timed out after {}s",
                        tc.name, config.tool_timeout_secs
                    );
                    run_logger.add_error(iteration, "tool_timeout", &timeout_msg);
                    iter_log.tool_calls.push(ToolCallLog {
                        tool: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        result: timeout_msg.clone(),
                        tat_ms: tool_start.elapsed().as_millis() as u64,
                        is_error: true,
                        error: Some(timeout_msg.clone()),
                        permission_denied: None,
                        timed_out: true,
                    });
                    messages.push(Message::tool_result(tc.id.clone(), timeout_msg));
                    continue;
                }
            };
            let duration_ms = tool_start.elapsed().as_millis() as u64;

            let truncated = false;
            trace.log_tool_result(iteration, &tc.name, &output.content, duration_ms, truncated);

            iter_log.tool_calls.push(ToolCallLog {
                tool: tc.name.clone(),
                arguments: tc.arguments.clone(),
                result: output.content.clone(),
                tat_ms: duration_ms,
                is_error: output.is_error,
                error: if output.is_error {
                    Some(output.content.clone())
                } else {
                    None
                },
                permission_denied: None,
                timed_out: false,
            });

            messages.push(Message::tool_result(tc.id.clone(), output.content));

            if config.verbose {
                eprintln!("  Tool {}: {}ms", tc.name, duration_ms);
            }
        }

        run_logger.add_iteration(iter_log);

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

            let result = LoopResult {
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
            run_logger.finish(&result.status, result.exit_code);
            run_logger.set_duration(start.elapsed().as_millis() as u64);
            return result;
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
