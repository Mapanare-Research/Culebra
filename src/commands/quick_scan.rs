use colored::Colorize;

use crate::ir;
use crate::template::engine;
use crate::template::loader;
use crate::template::schema::{FileType, Severity};

pub fn run(file: &str, template_path: Option<&str>) -> i32 {
    let templates_dir = if let Some(path) = template_path {
        std::path::PathBuf::from(path)
    } else {
        match loader::find_templates_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("{}: No templates directory found.", "error".red().bold());
                return 1;
            }
        }
    };

    let mut templates = loader::load_templates(&templates_dir);
    if templates.is_empty() {
        eprintln!("No templates found.");
        return 1;
    }

    // Filter to critical + high only
    let sev_filters = vec![Severity::Critical, Severity::High];
    templates = loader::filter_templates(&templates, &[], &sev_filters, &[]);

    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let is_c = file.ends_with(".c") || file.ends_with(".h");
    let module = if is_c {
        ir::parse_ir_from_raw(&content)
    } else {
        ir::parse_ir(&content)
    };

    let mut all_findings = Vec::new();

    for template in &templates {
        if is_c && template.scope.file_type == FileType::LlvmIr {
            continue;
        }
        if !is_c && template.scope.file_type == FileType::CSource {
            continue;
        }
        if template.scope.file_type == FileType::CrossReference {
            continue; // Skip cross-ref in quick mode
        }

        let findings = engine::run_template(template, &module);
        all_findings.extend(findings);
    }

    all_findings.sort_by(|a, b| a.severity.cmp(&b.severity));

    if all_findings.is_empty() {
        println!("{} {} — 0 findings", "PASS".green().bold(), file);
        return 0;
    }

    // One-line per finding, no remediation
    for f in &all_findings {
        let sev = match f.severity {
            Severity::Critical => "CRIT".red().bold().to_string(),
            Severity::High => "HIGH".red().to_string(),
            _ => unreachable!(),
        };
        let loc = if let Some(ref func) = f.function {
            format!("{}:{} ({})", file, f.line, func)
        } else {
            format!("{}:{}", file, f.line)
        };
        println!("  {} {} {}", sev, f.template_id, loc.dimmed());
    }

    // Summary line
    let critical = all_findings.iter().filter(|f| f.severity == Severity::Critical).count();
    let high = all_findings.iter().filter(|f| f.severity == Severity::High).count();
    let mut parts = Vec::new();
    if critical > 0 { parts.push(format!("{critical} critical")); }
    if high > 0 { parts.push(format!("{high} high")); }
    println!(
        "{} {} — {}",
        "FAIL".red().bold(),
        file,
        parts.join(", ")
    );

    1
}
