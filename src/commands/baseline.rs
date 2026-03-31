use colored::Colorize;
use std::collections::HashMap;

use crate::ir;
use crate::template::engine::{self, Finding};
use crate::template::loader;
use crate::template::schema::{FileType, Severity};

const BASELINE_FILE: &str = ".culebra-baseline.json";

#[derive(serde::Serialize, serde::Deserialize)]
struct BaselineEntry {
    template_id: String,
    severity: String,
    line: usize,
    function: Option<String>,
    matched_text: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Baseline {
    file: String,
    timestamp: String,
    findings: Vec<BaselineEntry>,
}

fn severity_str(s: &Severity) -> &'static str {
    match s {
        Severity::Critical => "critical",
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
        Severity::Info => "info",
    }
}

fn scan_file(file: &str) -> Vec<Finding> {
    let templates_dir = match loader::find_templates_dir() {
        Some(dir) => dir,
        None => return Vec::new(),
    };
    let templates = loader::load_templates(&templates_dir);
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let module = ir::parse_ir(&content);

    let mut all: Vec<Finding> = Vec::new();
    for t in &templates {
        if t.scope.file_type == FileType::CrossReference {
            continue;
        }
        all.extend(engine::run_template(t, &module));
    }
    all.sort_by(|a, b| a.severity.cmp(&b.severity));
    all
}

fn findings_to_entries(findings: &[Finding]) -> Vec<BaselineEntry> {
    findings
        .iter()
        .map(|f| BaselineEntry {
            template_id: f.template_id.clone(),
            severity: severity_str(&f.severity).to_string(),
            line: f.line,
            function: f.function.clone(),
            matched_text: f.matched_text.chars().take(120).collect(),
        })
        .collect()
}

pub fn run_save(file: &str, output: Option<&str>) -> i32 {
    let findings = scan_file(file);
    let baseline_path = output.unwrap_or(BASELINE_FILE);

    let baseline = Baseline {
        file: file.to_string(),
        timestamp: chrono_now(),
        findings: findings_to_entries(&findings),
    };

    let json = match serde_json::to_string_pretty(&baseline) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("{}: Failed to serialize: {}", "error".red().bold(), e);
            return 1;
        }
    };

    match std::fs::write(baseline_path, &json) {
        Ok(_) => {
            println!(
                "{} Baseline saved: {} findings from {} → {}",
                "culebra".green().bold(),
                findings.len(),
                file,
                baseline_path
            );
            // Quick summary
            let crit = findings.iter().filter(|f| f.severity == Severity::Critical).count();
            let high = findings.iter().filter(|f| f.severity == Severity::High).count();
            let med = findings.iter().filter(|f| f.severity == Severity::Medium).count();
            println!("  {} critical, {} high, {} medium", crit, high, med);
            0
        }
        Err(e) => {
            eprintln!("{}: Failed to write {}: {}", "error".red().bold(), baseline_path, e);
            1
        }
    }
}

pub fn run_diff(file: &str, baseline_path: Option<&str>) -> i32 {
    let bp = baseline_path.unwrap_or(BASELINE_FILE);

    // Load baseline
    let baseline_json = match std::fs::read_to_string(bp) {
        Ok(j) => j,
        Err(e) => {
            eprintln!(
                "{}: No baseline found at '{}': {}. Run 'culebra baseline save <file>' first.",
                "error".red().bold(), bp, e
            );
            return 1;
        }
    };
    let baseline: Baseline = match serde_json::from_str(&baseline_json) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}: Failed to parse baseline: {}", "error".red().bold(), e);
            return 1;
        }
    };

    // Current scan
    let current_findings = scan_file(file);
    let current_entries = findings_to_entries(&current_findings);

    // Build fingerprints for comparison: (template_id, function, line)
    let baseline_set: HashMap<String, &BaselineEntry> = baseline.findings.iter()
        .map(|e| (format!("{}:{}:{}", e.template_id, e.function.as_deref().unwrap_or(""), e.line), e))
        .collect();
    let current_set: HashMap<String, &BaselineEntry> = current_entries.iter()
        .map(|e| (format!("{}:{}:{}", e.template_id, e.function.as_deref().unwrap_or(""), e.line), e))
        .collect();

    let mut fixed = Vec::new();
    let mut remaining = Vec::new();
    let mut new_findings = Vec::new();

    // Fixed: in baseline but not in current
    for (key, entry) in &baseline_set {
        if !current_set.contains_key(key) {
            fixed.push(*entry);
        } else {
            remaining.push(*entry);
        }
    }

    // New: in current but not in baseline
    for (key, entry) in &current_set {
        if !baseline_set.contains_key(key) {
            new_findings.push(*entry);
        }
    }

    // Group fixed/new by template_id for compact output
    let fixed_groups = group_by_template(&fixed);
    let new_groups = group_by_template(&new_findings);

    println!(
        "{} Baseline diff: {} → {}",
        "culebra".green().bold(),
        baseline.file,
        file
    );
    println!("  Baseline from: {}", baseline.timestamp.dimmed());
    println!();

    // Summary line
    println!(
        "  {} Fixed, {} New, {} Remaining (was {}, now {})",
        fixed.len().to_string().green().bold(),
        new_findings.len().to_string().red().bold(),
        remaining.len(),
        baseline.findings.len(),
        current_entries.len()
    );
    println!();

    if !fixed.is_empty() {
        println!("  {} ({}):", "Fixed".green().bold(), fixed.len());
        for (tid, entries) in &fixed_groups {
            let fns: Vec<_> = entries.iter()
                .filter_map(|e| e.function.as_deref())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            let fn_str = if fns.is_empty() { String::new() } else { format!(" in {}", fns.join(", ")) };
            println!("    {} {} ({}{})", "✓".green(), tid, entries.len(), fn_str);
        }
        println!();
    }

    if !new_findings.is_empty() {
        println!("  {} ({}):", "New".red().bold(), new_findings.len());
        for (tid, entries) in &new_groups {
            let fns: Vec<_> = entries.iter()
                .filter_map(|e| e.function.as_deref())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            let fn_str = if fns.is_empty() { String::new() } else { format!(" in {}", fns.join(", ")) };
            println!("    {} {} ({}{})", "✗".red(), tid, entries.len(), fn_str);
        }
        println!();
    }

    if new_findings.is_empty() && !fixed.is_empty() {
        println!("  {} No regressions. {} findings fixed since baseline.", "✓".green().bold(), fixed.len());
    }

    if new_findings.is_empty() { 0 } else { 1 }
}

fn group_by_template<'a>(entries: &[&'a BaselineEntry]) -> Vec<(String, Vec<&'a BaselineEntry>)> {
    let mut map: HashMap<String, Vec<&'a BaselineEntry>> = HashMap::new();
    for e in entries {
        map.entry(e.template_id.clone()).or_default().push(e);
    }
    let mut sorted: Vec<_> = map.into_iter().collect();
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    sorted
}

fn chrono_now() -> String {
    // Simple timestamp without chrono dependency
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}
