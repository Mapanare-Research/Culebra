use colored::Colorize;

use crate::ir;
use crate::template::engine;
use crate::template::loader;
use crate::template::schema::FileType;

pub fn run(file: &str, finding_id: &str, function: Option<&str>) -> i32 {
    let templates_dir = match loader::find_templates_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("{}: No templates directory found", "error".red().bold());
            return 1;
        }
    };

    let templates = loader::load_templates(&templates_dir);
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    let template = match templates.iter().find(|t| t.id == finding_id) {
        Some(t) => t,
        None => {
            eprintln!(
                "{}: Template '{}' not found",
                "error".red().bold(),
                finding_id
            );
            return 1;
        }
    };

    let findings = if template.scope.file_type == FileType::CrossReference {
        Vec::new()
    } else {
        engine::run_template(template, &module)
    };

    // Filter by function if specified
    let filtered: Vec<_> = if let Some(func) = function {
        findings.iter().filter(|f| {
            f.function.as_deref().map(|n| n.contains(func)).unwrap_or(false)
        }).collect()
    } else {
        findings.iter().collect()
    };

    let func_label = function
        .map(|f| format!(" in '{}'", f))
        .unwrap_or_default();

    if filtered.is_empty() {
        println!(
            "  {} [{}]{} — {} {}",
            "PASS".green().bold(),
            finding_id,
            func_label,
            "finding is gone!",
            "Fix verified.".green()
        );
        0
    } else {
        println!(
            "  {} [{}]{} — {} occurrence{} still present",
            "FAIL".red().bold(),
            finding_id,
            func_label,
            filtered.len(),
            if filtered.len() == 1 { "" } else { "s" }
        );
        for f in &filtered {
            println!(
                "    line {} {}",
                f.line,
                f.function.as_deref().unwrap_or("").dimmed()
            );
        }
        1
    }
}
