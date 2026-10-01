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
            description: "Find files matching a glob pattern. Respects .gitignore and skips hidden files by default.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern to match (e.g. **/*.rs, src/*.ts)" },
                    "path": { "type": "string", "description": "Base directory to search in", "default": "." }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let pattern = args["pattern"].as_str().unwrap_or("");
        let base_path = args["path"].as_str().unwrap_or(".");
        let base = resolve_path(&self.working_dir, base_path);

        if pattern.is_empty() {
            return ToolOutput {
                content: "Pattern must not be empty".into(),
                is_error: true,
            };
        }

        // Compile the user pattern into a GlobSet matcher.
        let glob = match globset::Glob::new(pattern) {
            Ok(g) => g,
            Err(e) => {
                return ToolOutput {
                    content: format!("Invalid glob pattern: {}", e),
                    is_error: true,
                }
            }
        };
        let matcher = match globset::GlobSetBuilder::new().add(glob).build() {
            Ok(gs) => gs,
            Err(e) => {
                return ToolOutput {
                    content: format!("Failed to compile glob set: {}", e),
                    is_error: true,
                }
            }
        };

        // If the base path is a single file, just test it directly.
        if base.is_file() {
            let rel = base.strip_prefix(&self.working_dir).unwrap_or(&base);
            let matched = matcher.is_match(base.as_path())
                || matcher.is_match(rel);
            let content = if matched {
                base.to_string_lossy().into_owned()
            } else {
                "No matches found".into()
            };
            return ToolOutput { content, is_error: false };
        }

        if !base.is_dir() {
            return ToolOutput {
                content: format!("Path not found: {}", base.display()),
                is_error: true,
            };
        }

        // Walk the directory tree with .gitignore / hidden-file awareness.
        let walker = ignore::WalkBuilder::new(&base)
            .hidden(true)
            .git_ignore(true)
            .git_exclude(true)
            .git_global(true)
            .build();

        let mut results: Vec<String> = Vec::new();
        for entry in walker.flatten() {
            if results.len() >= 1000 {
                results.push("... (truncated at 1000 results)".into());
                break;
            }
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            // Match against both the absolute path and the path relative to base.
            let rel = path.strip_prefix(&base).unwrap_or(path);
            if matcher.is_match(path) || matcher.is_match(rel) {
                results.push(path.to_string_lossy().into_owned());
            }
        }

        ToolOutput {
            content: if results.is_empty() {
                "No matches found".into()
            } else {
                results.sort();
                results.join("\n")
            },
            is_error: false,
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
            description: "Search file contents using a regex pattern. Respects .gitignore and skips hidden files by default.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern to search for" },
                    "path": { "type": "string", "description": "Directory or file to search in", "default": "." },
                    "glob": { "type": "string", "description": "File glob pattern to filter files (e.g. *.rs)" }
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

        // Optional file-name glob filter.
        let file_filter = if let Some(gp) = glob_pattern {
            match globset::Glob::new(gp) {
                Ok(g) => Some(g.compile_matcher()),
                Err(_) => None,
            }
        } else {
            None
        };

        let mut results: Vec<String> = Vec::new();
        let mut truncated = false;

        if search_path.is_file() {
            search_file(&search_path, &re, &mut results, 1000);
        } else if search_path.is_dir() {
            // Walk with .gitignore / hidden-file awareness.
            let walker = ignore::WalkBuilder::new(&search_path)
                .hidden(true)
                .git_ignore(true)
                .git_exclude(true)
                .git_global(true)
                .build();

            for entry in walker.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                // Apply optional glob filter on the file name.
                if let Some(ref filter) = file_filter {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if !filter.is_match(name) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                search_file(path, &re, &mut results, 1000);
                if results.len() >= 1000 {
                    truncated = true;
                    break;
                }
            }
        } else {
            return ToolOutput {
                content: format!("Path not found: {}", search_path.display()),
                is_error: true,
            };
        }

        if truncated {
            results.push("... (truncated at 1000 matches)".into());
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

fn search_file(
    path: &std::path::Path,
    re: &regex::Regex,
    results: &mut Vec<String>,
    limit: usize,
) {
    if let Ok(content) = std::fs::read_to_string(path) {
        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                results.push(format!("{}:{}: {}", path.display(), i + 1, line));
                if results.len() >= limit {
                    return;
                }
            }
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
