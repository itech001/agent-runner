use crate::config::FilesystemPermission;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Component, PathBuf};

/// Normalize a path string by removing `.` components so that joined
/// relative paths (e.g. `/work/./src/**`) become canonical (`/work/src/**`).
/// This is necessary for globset matching to work consistently.
fn normalize(path: &str) -> String {
    let p = PathBuf::from(path);
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().into_owned()
}

pub struct PermissionEvaluator {
    rules: Vec<FilesystemPermission>,
    writable_globs: GlobSet,
    working_dir: PathBuf,
}

impl PermissionEvaluator {
    pub fn new(
        rules: Vec<FilesystemPermission>,
        writable_paths: Vec<String>,
        working_dir: PathBuf,
    ) -> Self {
        let normalized_wd = normalize(&working_dir.to_string_lossy());
        let mut builder = GlobSetBuilder::new();
        for pattern in &writable_paths {
            // Resolve relative patterns against the working directory so they
            // match absolute paths consistently. Strip a leading "./" so the
            // joined path doesn't contain a spurious "." component.
            let clean = pattern.strip_prefix("./").unwrap_or(pattern);
            let resolved_pattern = if PathBuf::from(pattern).is_absolute() {
                normalize(pattern)
            } else {
                format!("{}/{}", normalized_wd, clean)
            };
            if let Ok(glob) = Glob::new(&resolved_pattern) {
                builder.add(glob);
            }
        }
        let writable_globs = builder.build().unwrap_or_else(|_| GlobSet::empty());

        Self {
            rules,
            writable_globs,
            working_dir: PathBuf::from(normalized_wd),
        }
    }

    /// Resolve a (possibly relative) path against the working directory so
    /// that all permission checks operate on canonical absolute paths.
    fn resolve(&self, path: &str) -> String {
        let p = PathBuf::from(path);
        let resolved = if p.is_absolute() {
            normalize(path)
        } else {
            normalize(&format!("{}/{}", self.working_dir.to_string_lossy(), path))
        };
        resolved
    }

    pub fn check(&self, operation: &str, path: &str) -> bool {
        // Read operations are allowed everywhere by default.
        if operation == "read" {
            return true;
        }

        // Write operations: check writable_paths first, then fall back to the
        // explicit permissions array for backwards compatibility.
        let resolved = self.resolve(path);

        // 1. writable_paths whitelist (globset match).
        if self.writable_globs.is_match(&resolved) {
            return true;
        }
        // Also try matching the raw (unresolved) path, since a user may
        // configure writable_paths with relative patterns.
        if path != resolved && self.writable_globs.is_match(path) {
            return true;
        }

        // 2. Explicit permissions rules (legacy / advanced).
        for rule in &self.rules {
            if !rule.operations.iter().any(|op| op == operation) {
                continue;
            }

            let path_matches = rule.paths.iter().any(|pattern| {
                if pattern.ends_with("/*") {
                    let prefix = &pattern[..pattern.len() - 2];
                    resolved == prefix || resolved.starts_with(&format!("{}/", prefix))
                } else {
                    resolved == *pattern || path == *pattern
                }
            });

            if path_matches {
                return rule.mode == "allow";
            }
        }

        // 3. Default-deny for writes.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evaluator(writable: Vec<&str>, working_dir: &str) -> PermissionEvaluator {
        PermissionEvaluator::new(
            Vec::new(),
            writable.into_iter().map(String::from).collect(),
            PathBuf::from(working_dir),
        )
    }

    #[test]
    fn read_always_allowed() {
        let e = evaluator(vec![], "/work");
        assert!(e.check("read", "/etc/passwd"));
        assert!(e.check("read", "anything"));
    }

    #[test]
    fn write_denied_by_default() {
        let e = evaluator(vec![], "/work");
        assert!(!e.check("write", "/work/src/main.rs"));
    }

    #[test]
    fn write_allowed_in_writable_paths() {
        let e = evaluator(vec!["/work/src/**"], "/work");
        assert!(e.check("write", "/work/src/main.rs"));
        assert!(e.check("write", "/work/src/nested/deep.rs"));
        assert!(!e.check("write", "/work/other.rs"));
    }

    #[test]
    fn write_allowed_with_relative_pattern() {
        let e = evaluator(vec!["./src/**"], "/work");
        // Resolved absolute path should match.
        assert!(e.check("write", "src/main.rs"));
        assert!(e.check("write", "/work/src/main.rs"));
        assert!(!e.check("write", "/work/out/main.rs"));
    }

    #[test]
    fn legacy_permissions_still_work() {
        let rules = vec![FilesystemPermission {
            operations: vec!["write".into()],
            paths: vec!["/work/out/*".into()],
            mode: "allow".into(),
        }];
        let e = PermissionEvaluator::new(rules, vec![], PathBuf::from("/work"));
        assert!(e.check("write", "/work/out/file.txt"));
        assert!(!e.check("write", "/work/secret.txt"));
    }
}
