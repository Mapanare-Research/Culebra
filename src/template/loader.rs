use std::path::{Path, PathBuf};
use super::schema::{Template, WorkflowTemplate, Severity};

/// Find the templates directory — checks in order:
/// 1. ./culebra-templates/
/// 2. Next to the binary: <exe_dir>/culebra-templates/
/// 3. ~/.culebra/templates/
pub fn find_templates_dir() -> Option<PathBuf> {
    // Local project directory
    let local = PathBuf::from("culebra-templates");
    if local.is_dir() {
        return Some(local);
    }

    // Next to binary
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside = dir.join("culebra-templates");
            if beside.is_dir() {
                return Some(beside);
            }
        }
    }

    // Home directory
    if let Some(home) = dirs_fallback() {
        let home_dir = PathBuf::from(home).join(".culebra").join("templates");
        if home_dir.is_dir() {
            return Some(home_dir);
        }
    }

    None
}

fn dirs_fallback() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

/// Load all templates from a directory (recursively)
pub fn load_templates(dir: &Path) -> Vec<Template> {
    let mut templates = Vec::new();
    walk_yaml_files(dir, &mut |path| {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                // Skip workflow files
                if content.contains("type: workflow") || content.contains("workflow:") {
                    if content.contains("workflow:") && !content.contains("match:") {
                        return; // This is a workflow template, skip
                    }
                }
                match serde_yaml::from_str::<Template>(&content) {
                    Ok(t) => templates.push(t),
                    Err(e) => {
                        eprintln!("  warning: failed to parse {}: {}", path.display(), e);
                    }
                }
            }
            Err(e) => {
                eprintln!("  warning: failed to read {}: {}", path.display(), e);
            }
        }
    });
    templates
}

/// Load workflow templates from a directory
pub fn load_workflows(dir: &Path) -> Vec<WorkflowTemplate> {
    let mut workflows = Vec::new();
    walk_yaml_files(dir, &mut |path| {
        if let Ok(content) = std::fs::read_to_string(path) {
            if content.contains("workflow:") && !content.contains("match:") {
                match serde_yaml::from_str::<WorkflowTemplate>(&content) {
                    Ok(w) => workflows.push(w),
                    Err(_) => {}
                }
            }
        }
    });
    workflows
}

fn walk_yaml_files(dir: &Path, cb: &mut dyn FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_yaml_files(&path, cb);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yaml" | "yml")
        ) {
            cb(&path);
        }
    }
}

/// Filter templates by tags, severity, and/or id
pub fn filter_templates(
    templates: &[Template],
    tags: &[String],
    severities: &[Severity],
    ids: &[String],
) -> Vec<Template> {
    templates
        .iter()
        .filter(|t| {
            // Filter by id
            if !ids.is_empty() && !ids.contains(&t.id) {
                return false;
            }
            // Filter by tag (any tag matches)
            if !tags.is_empty() && !tags.iter().any(|tag| t.info.tags.contains(tag)) {
                return false;
            }
            // Filter by severity
            if !severities.is_empty() && !severities.contains(&t.info.severity) {
                return false;
            }
            true
        })
        .cloned()
        .collect()
}

/// Parse a comma-separated severity string
pub fn parse_severities(s: &str) -> Vec<Severity> {
    s.split(',')
        .filter_map(|s| match s.trim().to_lowercase().as_str() {
            "critical" => Some(Severity::Critical),
            "high" => Some(Severity::High),
            "medium" => Some(Severity::Medium),
            "low" => Some(Severity::Low),
            "info" => Some(Severity::Info),
            _ => None,
        })
        .collect()
}
