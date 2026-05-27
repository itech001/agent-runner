use dotenvy::dotenv;
use agent::planner::Planner;
use agent_dir::AgentDir;
use clap::Parser;
use permissions::PermissionEvaluator;
use provider::anthropic::AnthropicProvider;
use provider::openai::OpenAiProvider;
use provider::Provider;
use report::{Metrics, Report, TokenMetrics};
use run_log::RunLogger;
use std::path::PathBuf;
use std::sync::Arc;
use summarization::Summarizer;
use tools::compact::CompactTool;
use tools::done::TaskDoneTool;
use tools::filesystem::create_filesystem_tools;
use tools::todos::TodosTool;

pub mod agent;
pub mod agent_dir;
pub mod config;
pub mod mcp;
pub mod output;
pub mod permissions;
pub mod provider;
pub mod report;
pub mod run_log;
pub mod skills;
pub mod summarization;
pub mod tools;
pub mod trace;

#[derive(Parser, Debug)]
#[command(name = "agent-runner", version, about = "Non-interactive batch agent runner")]
pub struct Cli {
    #[arg(long)]
    pub agent_dir: PathBuf,

    #[arg(long)]
    pub prompt: String,

    #[arg(long, default_value_t = false)]
    pub plan_only: bool,

    #[arg(long, default_value_t = 50)]
    pub max_iterations: u32,

    #[arg(long, default_value = "./agent-output")]
    pub output_dir: PathBuf,

    #[arg(long, default_value = ".")]
    pub working_dir: PathBuf,

    #[arg(long)]
    pub mail_to: Option<String>,

    #[arg(long, default_value_t = false)]
    pub verbose: bool,

    #[arg(long, default_value_t = false)]
    pub sandbox: bool,

    #[arg(long, default_value_t = 120, value_name = "SECONDS")]
    pub tool_timeout: u64,

    #[arg(long, default_value_t = 3600, value_name = "SECONDS")]
    pub run_limit: u64,
}

#[tokio::main]
async fn main() {
    let _ = dotenv();

    let cli = Cli::parse();

    let agent_dir = match AgentDir::load(&cli.agent_dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Agent dir error: {}", e);
            std::process::exit(3);
        }
    };

    let llm = match config::Config::load_llm() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("LLM config error: {}", e);
            std::process::exit(3);
        }
    };

    let provider: Arc<dyn Provider> = match llm.provider.as_str() {
        "anthropic" => Arc::new(AnthropicProvider::new(
            llm.api_key,
            llm.model.clone(),
            llm.max_tokens,
            llm.temperature,
        )),
        _ => Arc::new(OpenAiProvider::new(
            llm.api_key,
            llm.model.clone(),
            llm.base_url.clone(),
            llm.max_tokens,
            llm.temperature,
        )),
    };

    let trace = Arc::new(match trace::TraceLogger::new(&cli.output_dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to create trace log: {}", e);
            std::process::exit(3);
        }
    });

    let evaluator = PermissionEvaluator::new(agent_dir.config.permissions.clone());
    let compact_tool = Arc::new(CompactTool::new());
    let todos_tool = Arc::new(TodosTool::new());

    let mut tools: Vec<Box<dyn tools::Tool>> = create_filesystem_tools(&cli.working_dir);

    if cli.sandbox || agent_dir.config.agent.execute_enabled {
        tools.push(Box::new(tools::execute::ExecuteTool::new(
            cli.working_dir.clone(),
            agent_dir.config.agent.execute_timeout_secs,
        )));
    }

    let tool_names: Vec<String> = tools.iter().map(|t| t.name().to_string()).collect();

    tools.push(Box::new(TaskDoneTool));
    tools.push(Box::new(tools::compact::CompactTool::new()));

    let skill_tools = tools::skill_tool::from_skills(&agent_dir.skills);
    tools.extend(skill_tools);

    let subagent_tools = tools::subagent::from_configs(&agent_dir.config.subagents);
    tools.extend(subagent_tools);

    let skill_names: Vec<String> = agent_dir.skills.iter().map(|s| s.name.clone()).collect();
    trace.log_init(
        &cli.agent_dir.to_string_lossy(),
        &llm.model,
        &tool_names,
        &skill_names,
    );

    let prompt_text = if std::path::Path::new(&cli.prompt).exists() {
        std::fs::read_to_string(&cli.prompt).unwrap_or(cli.prompt.clone())
    } else {
        cli.prompt.clone()
    };

    let run_logger = Arc::new(RunLogger::new(&prompt_text, &cli.output_dir));

    let mut system_prompt = agent_dir.system_prompt.clone();
    if !agent_dir.skills.is_empty() {
        for skill in &agent_dir.skills {
            system_prompt
                .push_str(&format!("\n\n## Skill: {}\n{}", skill.name, skill.instructions));
        }
    }

    let plan = if agent_dir.config.agent.plan_required {
        let planner = Planner::new(provider.clone(), trace.clone());
        let all_tool_names: Vec<String> = tools.iter().map(|t| t.name().to_string()).collect();
        let plan = planner
            .generate_plan(&system_prompt, &prompt_text, &all_tool_names)
            .await
            .unwrap_or_else(|e| {
                run_logger.add_error(0, "planning", &e);
                String::new()
            });
        Planner::save_plan(&plan, &cli.output_dir)
            .unwrap_or_else(|e| {
                run_logger.add_error(0, "planning", &format!("Failed to save plan: {}", e));
                eprintln!("Warning: {}", e)
            });
        plan
    } else {
        String::new()
    };

    if cli.plan_only {
        println!("{}", plan);
        std::process::exit(0);
    }

    let mut messages = vec![provider::Message::system(system_prompt)];
    if !plan.is_empty() {
        messages.push(provider::Message::system(format!(
            "[Execution Plan]\n{}",
            plan
        )));
    }
    messages.push(provider::Message::user(prompt_text.clone()));

    let mut summarizer = Summarizer::new(
        agent_dir.config.summarization.clone(),
        provider.clone(),
        trace.clone(),
    );

    let loop_config = agent::r#loop::LoopConfig {
        max_iterations: cli.max_iterations,
        tool_timeout_secs: cli.tool_timeout,
        run_limit_secs: cli.run_limit,
        verbose: cli.verbose,
    };

    let result = agent::r#loop::run_loop(
        provider,
        tools,
        &mut messages,
        loop_config,
        &mut summarizer,
        evaluator,
        trace.clone(),
        compact_tool.clone(),
        todos_tool.clone(),
        run_logger.clone(),
    )
    .await;

    let todos_final = todos_tool.state.lock().unwrap().clone();

    let report = Report::new(
        result.status.clone(),
        result.exit_code,
        prompt_text.clone(),
        cli.output_dir.join("plan.md").to_string_lossy().into_owned(),
        todos_final,
        0,
        Metrics {
            total_iterations: result.total_iterations,
            total_tool_calls: result.total_tool_calls,
            total_tokens: TokenMetrics {
                input: result.total_input_tokens,
                output: result.total_output_tokens,
            },
            summarization_runs: summarizer.runs(),
            tokens_saved_by_summarization: summarizer.tokens_saved(),
            permission_denials: result.permission_denials,
            duration_secs: result.duration_secs,
        },
    );

    if let Err(e) = output::write_output(
        &result.final_text,
        &report,
        &cli.output_dir,
        cli.mail_to.as_deref(),
    ) {
        eprintln!("Output error: {}", e);
        run_logger.add_error(result.total_iterations, "output", &format!("Output error: {}", e));
    }

    report::write_transcript(&messages, &cli.output_dir)
        .unwrap_or_else(|e| {
            eprintln!("Transcript error: {}", e);
            run_logger.add_error(result.total_iterations, "transcript", &format!("Transcript error: {}", e));
        });

    if let Err(e) = run_logger.write_to_file() {
        eprintln!("Failed to write run.json: {}", e);
    }

    std::process::exit(result.exit_code as i32);
}
