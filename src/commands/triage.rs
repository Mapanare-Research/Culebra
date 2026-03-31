use colored::Colorize;
use std::collections::HashMap;

use crate::ir;
use crate::template::engine::{self, Finding};
use crate::template::loader;
use crate::template::schema::{FileType, Severity};

pub fn run(file: &str, format: &str) -> i32 {
    // Load and run all templates (same as scan but with triage output)
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

    let mut all_findings: Vec<Finding> = Vec::new();
    for template in &templates {
        if template.scope.file_type == FileType::CrossReference {
            continue;
        }
        all_findings.extend(engine::run_template(template, &module));
    }

    if all_findings.is_empty() {
        println!("{} No findings — IR looks clean.", "culebra".green().bold());
        return 0;
    }

    // Group by template_id (root cause)
    let mut groups: HashMap<String, Vec<&Finding>> = HashMap::new();
    for f in &all_findings {
        groups.entry(f.template_id.clone()).or_default().push(f);
    }

    // Sort groups by worst severity, then by count
    let mut sorted_groups: Vec<_> = groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| {
        let sev_a = a.1.iter().map(|f| &f.severity).min().unwrap();
        let sev_b = b.1.iter().map(|f| &f.severity).min().unwrap();
        sev_a.cmp(sev_b).then_with(|| b.1.len().cmp(&a.1.len()))
    });

    if format == "json" {
        print_json(&sorted_groups, file, &all_findings);
    } else {
        print_text(&sorted_groups, file, &all_findings);
    }

    let has_critical = all_findings
        .iter()
        .any(|f| matches!(f.severity, Severity::Critical | Severity::High));
    if has_critical { 1 } else { 0 }
}

fn severity_label(s: &Severity) -> &'static str {
    match s {
        Severity::Critical => "critical",
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
        Severity::Info => "info",
    }
}

fn print_text(groups: &[(String, Vec<&Finding>)], file: &str, all: &[Finding]) {
    let crit = all.iter().filter(|f| f.severity == Severity::Critical).count();
    let high = all.iter().filter(|f| f.severity == Severity::High).count();
    let med = all.iter().filter(|f| f.severity == Severity::Medium).count();

    println!(
        "{} Triage: {} — {} root causes, {} total findings ({} critical, {} high, {} medium)",
        "culebra".green().bold(),
        file,
        groups.len(),
        all.len(),
        crit, high, med
    );
    println!();

    for (i, (template_id, findings)) in groups.iter().enumerate() {
        let sev = severity_label(&findings[0].severity);
        let sev_colored = match findings[0].severity {
            Severity::Critical => sev.red().bold().to_string(),
            Severity::High => sev.yellow().bold().to_string(),
            Severity::Medium => sev.cyan().to_string(),
            _ => sev.to_string(),
        };

        // Collect affected functions
        let mut functions: Vec<String> = findings
            .iter()
            .filter_map(|f| f.function.clone())
            .collect();
        functions.sort();
        functions.dedup();

        println!(
            "  {}. [{}] {} ({} hit{})",
            i + 1,
            sev_colored,
            template_id.bold(),
            findings.len(),
            if findings.len() == 1 { "" } else { "s" }
        );

        if !functions.is_empty() {
            println!("     functions: {}", functions.join(", "));
        }

        // Show suggestion (first finding's)
        let suggestion = &findings[0].suggestion;
        if !suggestion.is_empty() {
            let first_line = suggestion.lines().next().unwrap_or("");
            println!("     fix: {}", first_line.dimmed());
        }
        println!();
    }
}

fn print_json(groups: &[(String, Vec<&Finding>)], file: &str, all: &[Finding]) {
    let entries: Vec<serde_json::Value> = groups
        .iter()
        .map(|(id, findings)| {
            let mut functions: Vec<String> = findings
                .iter()
                .filter_map(|f| f.function.clone())
                .collect();
            functions.sort();
            functions.dedup();

            serde_json::json!({
                "template_id": id,
                "severity": severity_label(&findings[0].severity),
                "count": findings.len(),
                "functions": functions,
                "suggestion": findings[0].suggestion.lines().next().unwrap_or(""),
                "lines": findings.iter().map(|f| f.line).collect::<Vec<_>>(),
            })
        })
        .collect();

    let output = serde_json::json!({
        "file": file,
        "root_causes": groups.len(),
        "total_findings": all.len(),
        "triage": entries,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
