use crate::config::FilesystemPermission;

pub struct PermissionEvaluator {
    rules: Vec<FilesystemPermission>,
}

impl PermissionEvaluator {
    pub fn new(rules: Vec<FilesystemPermission>) -> Self {
        Self { rules }
    }

    pub fn check(&self, operation: &str, path: &str) -> bool {
        for rule in &self.rules {
            if !rule.operations.iter().any(|op| op == operation) {
                continue;
            }

            let path_matches = rule.paths.iter().any(|pattern| {
                if pattern.ends_with("/*") {
                    let prefix = &pattern[..pattern.len() - 2];
                    path == prefix || path.starts_with(&format!("{}/", prefix))
                } else {
                    path == pattern
                }
            });

            if path_matches {
                return rule.mode == "allow";
            }
        }
        false
    }
}
