use colored::Colorize;
use std::collections::HashMap;

use crate::ir;
use crate::template::engine;
use crate::template::loader;
use crate::template::schema::{FileType, Severity};

const BASELINE_FILE: &str = ".culebra-baseline.json";

pub fn run(file: &str, baseline_path: Option<&str>) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    // Current scan
    let templates_dir = match loader::find_templates_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("{}: No templates directory found", "error".red().bold());
            return 1;
        }
    };
    let templates = loader::load_templates(&templates_dir);

    let mut findings: Vec<engine::Finding> = Vec::new();
    for t in &templates {
        if t.scope.file_type == FileType::CrossReference {
            continue;
        }
        findings.extend(engine::run_template(t, &module));
    }
    findings.sort_by(|a, b| a.severity.cmp(&b.severity));

    // Count by severity
    let crit = findings.iter().filter(|f| f.severity == Severity::Critical).count();
    let high = findings.iter().filter(|f| f.severity == Severity::High).count();
    let med = findings.iter().filter(|f| f.severity == Severity::Medium).count();

    // Group by template
    let mut groups: HashMap<String, usize> = HashMap::new();
    for f in &findings {
        *groups.entry(f.template_id.clone()).or_default() += 1;
    }

    // IR stats
    let fn_count = module.functions.len();
    let total_insns: usize = module.functions.values().map(|f| f.metrics.instructions).sum();
    let total_allocas: usize = module.functions.values().map(|f| f.metrics.allocas).sum();
    let structs = module.struct_types.len();

    println!(
        "{} Progress report: {}",
        "culebra".green().bold(),
        file
    );
    println!();

    // IR summary
    println!("  {}", "IR Summary:".bold());
    println!("    Functions:    {}", fn_count);
    println!("    Instructions: {}", total_insns);
    println!("    Allocas:      {}", total_allocas);
    println!("    Struct types: {}", structs);
    println!();

    // Current findings
    println!("  {}", "Current Findings:".bold());
    println!(
        "    {} critical, {} high, {} medium ({} total across {} patterns)",
        crit.to_string().red().bold(),
        high.to_string().yellow().bold(),
        med,
        findings.len(),
        groups.len()
    );

    // Top issues
    let mut sorted_groups: Vec<_> = groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| b.1.cmp(&a.1));

    if !sorted_groups.is_empty() {
        println!();
        println!("  {}", "Top issues:".bold());
        for (id, count) in sorted_groups.iter().take(5) {
            let sev = findings.iter().find(|f| &f.template_id == id).map(|f| &f.severity);
            let sev_str = match sev {
                Some(Severity::Critical) => "CRIT".red().bold().to_string(),
                Some(Severity::High) => "HIGH".yellow().bold().to_string(),
                Some(Severity::Medium) => "MED ".cyan().to_string(),
                _ => "    ".to_string(),
            };
            println!("    [{}] {} ({})", sev_str, id, count);
        }
    }

    // Baseline comparison if available
    let bp = baseline_path.unwrap_or(BASELINE_FILE);
    if let Ok(baseline_json) = std::fs::read_to_string(bp) {
        if let Ok(baseline) = serde_json::from_str::<serde_json::Value>(&baseline_json) {
            let baseline_count = baseline["findings"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0);

            println!();
            println!("  {}", "vs Baseline:".bold());
            let delta = findings.len() as i64 - baseline_count as i64;
            let delta_str = if delta < 0 {
                format!("{} (improved)", delta).green().bold().to_string()
            } else if delta > 0 {
                format!("+{} (regressed)", delta).red().bold().to_string()
            } else {
                "0 (same)".to_string()
            };
            println!("    Was: {} findings → Now: {} findings ({})", baseline_count, findings.len(), delta_str);
        }
    }

    // Health score (simple heuristic)
    let score = if crit == 0 && high == 0 {
        100
    } else if crit == 0 {
        (80.0 - (high as f64 * 2.0).min(30.0)) as i32
    } else {
        (50.0 - (crit as f64 * 10.0).min(40.0) - (high as f64 * 2.0).min(10.0)) as i32
    };
    let score = score.max(0);

    println!();
    let score_colored = if score >= 80 {
        format!("{}%", score).green().bold().to_string()
    } else if score >= 50 {
        format!("{}%", score).yellow().bold().to_string()
    } else {
        format!("{}%", score).red().bold().to_string()
    };
    println!("  Health score: {}", score_colored);

    if crit > 0 { 1 } else { 0 }
}
