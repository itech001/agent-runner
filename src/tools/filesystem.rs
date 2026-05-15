use async_trait::async_trait;
use crate::provider::ToolDefinition;
use crate::tools::{Tool, ToolOutput};
use std::path::{Path, PathBuf};

fn resolve_path(working_dir: &Path, input: &str) -> PathBuf {
    let path = PathBuf::from(input);
    if path.is_absolute() {
        path
    } else {
        working_dir.join(path)
    }
}

pub struct LsTool {
    working_dir: PathBuf,
}

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "ls"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ls".into(),
            description: "List directory entries, one per line.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path to list" }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let path_str = args["path"].as_str().unwrap_or(".");
        let path = resolve_path(&self.working_dir, path_str);

        match std::fs::read_dir(&path) {
            Ok(entries) => {
                let mut names: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                ToolOutput {
                    content: names.join("\n"),
                    is_error: false,
                }
            }
            Err(e) => ToolOutput {
                content: format!("Error listing directory: {}", e),
                is_error: true,
            },
        }
    }
}

pub struct ReadFileTool {
    working_dir: PathBuf,
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".into(),
            description: "Read file contents with line-based pagination.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Path to the file" },
                    "offset": { "type": "integer", "description": "Starting line number (0-based)", "default": 0 },
                    "limit": { "type": "integer", "description": "Maximum number of lines to read", "default": 100 }
                },
                "required": ["file_path"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let file_path = args["file_path"].as_str().unwrap_or("");
        let offset = args["offset"].as_u64().unwrap_or(0) as usize;
        let limit = args["limit"].as_u64().unwrap_or(100) as usize;
        let path = resolve_path(&self.working_dir, file_path);

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let total = lines.len();
                let selected: Vec<&str> = lines.iter().skip(offset).take(limit).copied().collect();
                let end = offset + selected.len();
                let range_info = format!("Lines {}-{} of {}", offset, end.saturating_sub(1), total);
                ToolOutput {
                    content: format!("{}\n{}", range_info, selected.join("\n")),
                    is_error: false,
                }
            }
            Err(e) => ToolOutput {
                content: format!("Error reading file: {}", e),
                is_error: true,
            },
        }
    }
}

pub struct WriteFileTool {
    working_dir: PathBuf,
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".into(),
            description: "Write content to a file, creating parent directories if needed.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Path to the file" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["file_path", "content"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let file_path = args["file_path"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");
        let path = resolve_path(&self.working_dir, file_path);

        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return ToolOutput {
                    content: format!("Error creating directories: {}", e),
                    is_error: true,
                };
            }
        }

        match std::fs::write(&path, content) {
            Ok(()) => ToolOutput {
                content: format!("Successfully wrote to {}", file_path),
                is_error: false,
            },
            Err(e) => ToolOutput {
                content: format!("Error writing file: {}", e),
                is_error: true,
            },
        }
    }
}

pub struct EditFileTool {
    working_dir: PathBuf,
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit_file".into(),
            description: "Replace strings in a file. Reports occurrence count.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Path to the file" },
                    "old_string": { "type": "string", "description": "String to find" },
                    "new_string": { "type": "string", "description": "Replacement string" },
                    "replace_all": { "type": "boolean", "description": "Replace all occurrences", "default": false }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let file_path = args["file_path"].as_str().unwrap_or("");
        let old_string = args["old_string"].as_str().unwrap_or("");
        let new_string = args["new_string"].as_str().unwrap_or("");
        let replace_all = args["replace_all"].as_bool().unwrap_or(false);
        let path = resolve_path(&self.working_dir, file_path);

        if old_string.is_empty() {
            return ToolOutput {
                content: "old_string must not be empty".into(),
                is_error: true,
            };
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                return ToolOutput {
                    content: format!("Error reading file: {}", e),
                    is_error: true,
                }
            }
        };

        let count = content.matches(old_string).count();
        if count == 0 {
            return ToolOutput {
                content: "No occurrences found".into(),
                is_error: true,
            };
        }

        if !replace_all && count > 1 {
            return ToolOutput {
                content: format!(
                    "Found {} occurrences. Use replace_all=true to replace all.",
                    count
                ),
                is_error: true,
            };
        }

        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        match std::fs::write(&path, new_content) {
            Ok(()) => ToolOutput {
                content: format!("Replaced {} occurrence(s) in {}", count, file_path),
                is_error: false,
            },
            Err(e) => ToolOutput {
                content: format!("Error writing file: {}", e),
                is_error: true,
            },
        }
    }
}

pub struct GlobTool {
    working_dir: PathBuf,
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "glob".into(),
            description: "Find files matching a glob pattern.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern to match" },
                    "path": { "type": "string", "description": "Base directory to search in", "default": "/" }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let pattern = args["pattern"].as_str().unwrap_or("");
        let base_path = args["path"].as_str().unwrap_or("/");
        let base = resolve_path(&self.working_dir, base_path);

        let full_pattern = if pattern.starts_with('/') {
            pattern.to_string()
        } else {
            base.join(pattern).to_string_lossy().into_owned()
        };

        match glob::glob(&full_pattern) {
            Ok(paths) => {
                let results: Vec<String> = paths
                    .filter_map(|p| p.ok())
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                ToolOutput {
                    content: if results.is_empty() {
                        "No matches found".into()
                    } else {
                        results.join("\n")
                    },
                    is_error: false,
                }
            }
            Err(e) => ToolOutput {
                content: format!("Invalid glob pattern: {}", e),
                is_error: true,
            },
        }
    }
}

pub struct GrepTool {
    working_dir: PathBuf,
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "grep".into(),
            description: "Search file contents using a regex pattern.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern to search for" },
                    "path": { "type": "string", "description": "Directory or file to search in" },
                    "glob": { "type": "string", "description": "File glob pattern to filter files" }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let pattern_str = args["pattern"].as_str().unwrap_or("");
        let path_str = args["path"].as_str().unwrap_or(".");
        let glob_pattern = args["glob"].as_str();

        let re = match regex::Regex::new(pattern_str) {
            Ok(r) => r,
            Err(e) => {
                return ToolOutput {
                    content: format!("Invalid regex: {}", e),
                    is_error: true,
                }
            }
        };

        let search_path = resolve_path(&self.working_dir, path_str);
        let mut results: Vec<String> = Vec::new();

        if search_path.is_file() {
            search_file(&search_path, &re, &mut results);
        } else if search_path.is_dir() {
            search_dir(&search_path, &re, glob_pattern, &mut results);
        } else {
            return ToolOutput {
                content: format!("Path not found: {}", search_path.display()),
                is_error: true,
            };
        }

        ToolOutput {
            content: if results.is_empty() {
                "No matches found".into()
            } else {
                results.join("\n")
            },
            is_error: false,
        }
    }
}

fn search_file(path: &std::path::Path, re: &regex::Regex, results: &mut Vec<String>) {
    if let Ok(content) = std::fs::read_to_string(path) {
        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                results.push(format!("{}:{}: {}", path.display(), i + 1, line));
            }
        }
    }
}

fn search_dir(
    dir: &std::path::Path,
    re: &regex::Regex,
    glob_pattern: Option<&str>,
    results: &mut Vec<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            search_dir(&path, re, glob_pattern, results);
        } else if let Some(gp) = glob_pattern {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(pat) = glob::Pattern::new(gp) {
                    if pat.matches(name) {
                        search_file(&path, re, results);
                    }
                }
            }
        } else {
            search_file(&path, re, results);
        }
    }
}

pub fn create_filesystem_tools(working_dir: &Path) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(LsTool { working_dir: working_dir.to_path_buf() }),
        Box::new(ReadFileTool { working_dir: working_dir.to_path_buf() }),
        Box::new(WriteFileTool { working_dir: working_dir.to_path_buf() }),
        Box::new(EditFileTool { working_dir: working_dir.to_path_buf() }),
        Box::new(GlobTool { working_dir: working_dir.to_path_buf() }),
        Box::new(GrepTool { working_dir: working_dir.to_path_buf() }),
    ]
}
