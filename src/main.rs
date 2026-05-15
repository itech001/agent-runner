use clap::Parser;
use std::path::PathBuf;

pub mod config;
pub mod provider;
pub mod tools;

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
}

fn main() {
    let cli = Cli::parse();
    let config_path = cli.agent_dir.join("config.json");
    match config::Config::load(&config_path) {
        Ok(config) => {
            if cli.verbose {
                eprintln!("Config loaded: model={}", config.llm.model);
            }
        }
        Err(e) => {
            eprintln!("Config error: {}", e);
            std::process::exit(3);
        }
    }
}
