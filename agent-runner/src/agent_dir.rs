use crate::config::Config;
use crate::skills::Skill;
use std::fs;
use std::path::{Path, PathBuf};

pub struct AgentDir {
    pub dir: PathBuf,
    pub system_prompt: String,
    pub skills: Vec<Skill>,
    pub config: Config,
}

impl AgentDir {
    pub fn load(dir: &Path) -> Result<Self, String> {
        if !dir.exists() {
            return Err(format!("Agent directory does not exist: {}", dir.display()));
        }

        let agents_md = dir.join("AGENTS.md");
        let system_prompt = if agents_md.exists() {
            fs::read_to_string(&agents_md)
                .map_err(|e| format!("Failed to read AGENTS.md: {}", e))?
        } else {
            "You are a helpful assistant.".to_string()
        };

        let config_path = dir.join("agent-runner.json");
        let config =
            Config::load(&config_path).map_err(|e| format!("Failed to load config: {}", e))?;

        let skills_dir = dir.join("skills");
        let skills = if skills_dir.is_dir() {
            fs::read_dir(&skills_dir)
                .map_err(|e| format!("Failed to read skills dir: {}", e))?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| Skill::load(&entry.path()).ok())
                .collect()
        } else {
            Vec::new()
        };

        Ok(Self {
            dir: dir.to_path_buf(),
            system_prompt,
            skills,
            config,
        })
    }
}
