use colored::Colorize;

use crate::ir;
use crate::template::engine;
use crate::template::loader;
use crate::template::report;
use crate::template::schema::Severity;

pub fn run(
    workflow_id: &str,
    inputs: &std::collections::HashMap<String, String>,
    format: &str,
) -> i32 {
    let templates_dir = match loader::find_templates_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("{}: No templates directory found.", "error".red().bold());
            return 1;
        }
    };

    let workflows = loader::load_workflows(&templates_dir);
    let Some(wf) = workflows.iter().find(|w| w.id == workflow_id) else {
        eprintln!("  Workflow '{}' not found.", workflow_id);
        let available: Vec<_> = workflows.iter().map(|w| w.id.as_str()).collect();
        if !available.is_empty() {
            eprintln!("  Available: {}", available.join(", "));
        }
        return 1;
    };

    let all_templates = loader::load_templates(&templates_dir);

    println!(
        "{} Running workflow: {}",
        "culebra".green().bold(),
        wf.info.name.bold()
    );
    println!();

    let mut all_findings = Vec::new();
    let mut step_num = 0;

    for step in &wf.workflow {
        step_num += 1;

        // Select templates for this step by tags or ids
        let step_templates: Vec<_> = all_templates
            .iter()
            .filter(|t| {
                if !step.templates.tags.is_empty() {
                    step.templates
                        .tags
                        .iter()
                        .any(|tag| t.info.tags.contains(tag))
                } else if !step.templates.ids.is_empty() {
                    step.templates.ids.contains(&t.id)
                } else {
                    false
                }
            })
            .collect();

        if step_templates.is_empty() {
            println!(
                "  Step {}: {} templates (skipped — no matching templates)",
                step_num, 0
            );
            continue;
        }

        // Resolve input file from step.input or step.inputs
        let input_file = step
            .input
            .as_ref()
            .map(|s| resolve_input(s, inputs))
            .or_else(|| {
                step.inputs
                    .as_ref()
                    .and_then(|m| m.get("ir_file").or(m.values().next()))
                    .map(|s| resolve_input(s, inputs))
            });

        let Some(ref file_path) = input_file else {
            println!("  Step {}: no input file resolved (skipped)", step_num);
            continue;
        };

        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  Step {}: failed to read {}: {}", step_num, file_path, e);
                continue;
            }
        };

        let module = ir::parse_ir(&content);

        println!(
            "  Step {}: running {} templates against {}",
            step_num,
            step_templates.len(),
            file_path
        );

        let mut step_findings = Vec::new();
        for tmpl in &step_templates {
            let findings = engine::run_template(tmpl, &module);
            step_findings.extend(findings);
        }

        let critical_count = step_findings
            .iter()
            .filter(|f| f.severity == Severity::Critical)
            .count();
        let high_count = step_findings
            .iter()
            .filter(|f| f.severity == Severity::High)
            .count();

        println!(
            "    {} findings ({} critical, {} high)",
            step_findings.len(),
            critical_count,
            high_count
        );

        all_findings.extend(step_findings);

        // Check stop condition
        if let Some(ref stop_sev) = step.stop_on {
            let should_stop = all_findings.iter().any(|f| f.severity <= *stop_sev);
            if should_stop {
                println!(
                    "    {} Stopping workflow — {} or worse severity found",
                    "!!".red().bold(),
                    stop_sev
                );
                break;
            }
        }
    }

    println!();
    all_findings.sort_by(|a, b| a.severity.cmp(&b.severity));

    let output_format = report::OutputFormat::from_str(format);
    match output_format {
        report::OutputFormat::Text => {
            report::print_text(&all_findings, "(workflow)");
            report::print_summary(&all_findings);
        }
        report::OutputFormat::Json => report::print_json(&all_findings, "(workflow)"),
        report::OutputFormat::Sarif => report::print_sarif(&all_findings, "(workflow)"),
        report::OutputFormat::Markdown => report::print_markdown(&all_findings, "(workflow)"),
    }

    let has_critical = all_findings
        .iter()
        .any(|f| matches!(f.severity, Severity::Critical | Severity::High));
    if has_critical { 1 } else { 0 }
}

fn resolve_input(template: &str, inputs: &std::collections::HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in inputs {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}
