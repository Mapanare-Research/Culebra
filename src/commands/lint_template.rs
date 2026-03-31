use colored::Colorize;

use crate::ir;
use crate::template::engine;
use crate::template::loader;
use crate::template::schema::FileType;

pub fn run(file: &str, template_id: &str, expect: bool, reject: bool) -> i32 {
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

    let template = match templates.iter().find(|t| t.id == template_id) {
        Some(t) => t,
        None => {
            eprintln!(
                "{}: Template '{}' not found",
                "error".red().bold(),
                template_id
            );
            return 1;
        }
    };

    let findings = if template.scope.file_type == FileType::CrossReference {
        Vec::new()
    } else {
        engine::run_template(template, &module)
    };

    let matched = !findings.is_empty();

    if expect {
        // Template MUST fire
        if matched {
            println!(
                "  {} [{}] fires as expected ({} match{})",
                "PASS".green().bold(),
                template_id,
                findings.len(),
                if findings.len() == 1 { "" } else { "es" }
            );
            0
        } else {
            println!(
                "  {} [{}] expected to fire but found 0 matches in {} ({} functions)",
                "FAIL".red().bold(),
                template_id,
                file,
                module.functions.len()
            );
            1
        }
    } else if reject {
        // Template must NOT fire
        if matched {
            println!(
                "  {} [{}] should not fire but found {} match{} in {}",
                "FAIL".red().bold(),
                template_id,
                findings.len(),
                if findings.len() == 1 { "" } else { "es" },
                file
            );
            for f in &findings {
                println!(
                    "    line {} {}",
                    f.line,
                    f.function.as_deref().unwrap_or("").dimmed()
                );
            }
            1
        } else {
            println!(
                "  {} [{}] correctly absent from {}",
                "PASS".green().bold(),
                template_id,
                file
            );
            0
        }
    } else {
        // No assertion — just report
        if matched {
            println!(
                "  {} [{}] fires: {} match{}",
                "INFO".cyan().bold(),
                template_id,
                findings.len(),
                if findings.len() == 1 { "" } else { "es" }
            );
        } else {
            println!(
                "  {} [{}] does not fire on {}",
                "INFO".cyan().bold(),
                template_id,
                file
            );
        }
        0
    }
}
