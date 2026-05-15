use std::fs;
use std::path::{Path, PathBuf};

pub struct Skill {
    pub name: String,
    pub dir_path: PathBuf,
    pub instructions: String,
    pub references: Vec<String>,
    pub scripts: Vec<PathBuf>,
}

impl Skill {
    pub fn load(skill_dir: &Path) -> Result<Self, String> {
        let name = skill_dir
            .file_name()
            .ok_or_else(|| "Invalid skill directory path".to_string())?
            .to_string_lossy()
            .to_string();

        let skill_md = skill_dir.join("SKILL.md");
        let instructions = if skill_md.exists() {
            fs::read_to_string(&skill_md).map_err(|e| format!("Failed to read SKILL.md: {}", e))?
        } else {
            String::new()
        };

        let refs_dir = skill_dir.join("references");
        let references = if refs_dir.is_dir() {
            fs::read_dir(&refs_dir)
                .map_err(|e| format!("Failed to read references dir: {}", e))?
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| {
                    let content = fs::read_to_string(entry.path()).ok()?;
                    Some(content)
                })
                .collect()
        } else {
            Vec::new()
        };

        let scripts_dir = skill_dir.join("scripts");
        let scripts = if scripts_dir.is_dir() {
            fs::read_dir(&scripts_dir)
                .map_err(|e| format!("Failed to read scripts dir: {}", e))?
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .collect()
        } else {
            Vec::new()
        };

        Ok(Self {
            name,
            dir_path: skill_dir.to_path_buf(),
            instructions,
            references,
            scripts,
        })
    }

    pub fn full_context(&self) -> String {
        let mut parts = Vec::new();
        if !self.instructions.is_empty() {
            parts.push(self.instructions.clone());
        }
        for (i, r) in self.references.iter().enumerate() {
            parts.push(format!("--- Reference {} ---\n{}", i + 1, r));
        }
        parts.join("\n\n")
    }
}
