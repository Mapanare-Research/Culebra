use colored::Colorize;
use serde_json::json;

use super::engine::Finding;
use super::schema::Severity;

// ---------------------------------------------------------------------------
// Output formats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
    Sarif,
    Markdown,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => OutputFormat::Json,
            "sarif" => OutputFormat::Sarif,
            "markdown" | "md" => OutputFormat::Markdown,
            _ => OutputFormat::Text,
        }
    }
}

// ---------------------------------------------------------------------------
// Text output
// ---------------------------------------------------------------------------

pub fn print_text(findings: &[Finding], file: &str) {
    if findings.is_empty() {
        println!("{}", "  No findings.".green());
        return;
    }

    for finding in findings {
        let sev_str = format_severity(&finding.severity);
        let location = if let Some(ref func) = finding.function {
            format!("{}:{} (in {})", file, finding.line, func)
        } else {
            format!("{}:{}", file, finding.line)
        };

        println!(
            "  {} [{}] {} — {}",
            sev_str,
            finding.template_id.bold(),
            finding.template_name,
            location.dimmed()
        );

        if !finding.report_format.is_empty() {
            for line in finding.report_format.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    println!("    {}", trimmed);
                }
            }
        } else if !finding.matched_text.is_empty() {
            println!("    {}", finding.matched_text.dimmed());
        }

        if !finding.suggestion.is_empty() {
            println!("    {} {}", "fix:".cyan(), finding.suggestion.lines().next().unwrap_or(""));
        }

        println!();
    }
}

fn format_severity(sev: &Severity) -> String {
    match sev {
        Severity::Critical => "CRITICAL".red().bold().to_string(),
        Severity::High => "HIGH".red().to_string(),
        Severity::Medium => "MEDIUM".yellow().to_string(),
        Severity::Low => "LOW".blue().to_string(),
        Severity::Info => "INFO".dimmed().to_string(),
    }
}

// ---------------------------------------------------------------------------
// JSON output
// ---------------------------------------------------------------------------

pub fn print_json(findings: &[Finding], file: &str) {
    let items: Vec<_> = findings
        .iter()
        .map(|f| {
            json!({
                "template_id": f.template_id,
                "template_name": f.template_name,
                "severity": f.severity.to_string(),
                "file": file,
                "line": f.line,
                "function": f.function,
                "matched_text": f.matched_text,
                "description": f.description,
                "impact": f.impact,
                "suggestion": f.suggestion,
                "cwe": f.cwe,
                "tags": f.tags,
                "extractions": f.extractions,
            })
        })
        .collect();

    let output = json!({
        "file": file,
        "findings_count": findings.len(),
        "findings": items,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

// ---------------------------------------------------------------------------
// SARIF output (GitHub Code Scanning compatible)
// ---------------------------------------------------------------------------

pub fn print_sarif(findings: &[Finding], file: &str) {
    let results: Vec<_> = findings
        .iter()
        .map(|f| {
            json!({
                "ruleId": f.template_id,
                "level": sarif_level(&f.severity),
                "message": {
                    "text": if !f.report_format.is_empty() {
                        &f.report_format
                    } else {
                        &f.template_name
                    }
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": file
                        },
                        "region": {
                            "startLine": f.line
                        }
                    }
                }]
            })
        })
        .collect();

    let rules: Vec<_> = findings
        .iter()
        .map(|f| {
            json!({
                "id": f.template_id,
                "name": f.template_name,
                "shortDescription": {
                    "text": f.template_name
                },
                "fullDescription": {
                    "text": f.description
                },
                "defaultConfiguration": {
                    "level": sarif_level(&f.severity)
                },
                "help": {
                    "text": f.suggestion
                }
            })
        })
        .collect();

    let sarif = json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "culebra",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/Mapanare-Research/Culebra",
                    "rules": rules
                }
            },
            "results": results
        }]
    });

    println!("{}", serde_json::to_string_pretty(&sarif).unwrap());
}

fn sarif_level(sev: &Severity) -> &'static str {
    match sev {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    }
}

// ---------------------------------------------------------------------------
// Markdown output
// ---------------------------------------------------------------------------

pub fn print_markdown(findings: &[Finding], file: &str) {
    println!("# Culebra Scan Report");
    println!();
    println!("**File:** `{}`", file);
    println!("**Findings:** {}", findings.len());
    println!();

    if findings.is_empty() {
        println!("No issues found.");
        return;
    }

    let critical = findings
        .iter()
        .filter(|f| f.severity == Severity::Critical)
        .count();
    let high = findings
        .iter()
        .filter(|f| f.severity == Severity::High)
        .count();
    let medium = findings
        .iter()
        .filter(|f| f.severity == Severity::Medium)
        .count();
    let low = findings
        .iter()
        .filter(|f| f.severity == Severity::Low)
        .count();

    println!("| Severity | Count |");
    println!("|----------|-------|");
    if critical > 0 {
        println!("| Critical | {} |", critical);
    }
    if high > 0 {
        println!("| High | {} |", high);
    }
    if medium > 0 {
        println!("| Medium | {} |", medium);
    }
    if low > 0 {
        println!("| Low | {} |", low);
    }
    println!();

    for finding in findings {
        println!(
            "### {} `{}` — {}",
            severity_emoji(&finding.severity),
            finding.template_id,
            finding.template_name
        );
        println!();

        let location = if let Some(ref func) = finding.function {
            format!("`{}:{}` (in `{}`)", file, finding.line, func)
        } else {
            format!("`{}:{}`", file, finding.line)
        };
        println!("**Location:** {}", location);
        println!();

        if !finding.report_format.is_empty() {
            println!("{}", finding.report_format.trim());
            println!();
        }

        if !finding.suggestion.is_empty() {
            println!("**Fix:** {}", finding.suggestion.lines().next().unwrap_or(""));
            println!();
        }

        println!("---");
        println!();
    }
}

fn severity_emoji(sev: &Severity) -> &'static str {
    match sev {
        Severity::Critical => "[CRITICAL]",
        Severity::High => "[HIGH]",
        Severity::Medium => "[MEDIUM]",
        Severity::Low => "[LOW]",
        Severity::Info => "[INFO]",
    }
}

// ---------------------------------------------------------------------------
// Summary line
// ---------------------------------------------------------------------------

pub fn print_summary(findings: &[Finding]) {
    let critical = findings.iter().filter(|f| f.severity == Severity::Critical).count();
    let high = findings.iter().filter(|f| f.severity == Severity::High).count();
    let medium = findings.iter().filter(|f| f.severity == Severity::Medium).count();
    let low = findings.iter().filter(|f| f.severity == Severity::Low).count();

    let parts: Vec<String> = [
        (critical, "critical"),
        (high, "high"),
        (medium, "medium"),
        (low, "low"),
    ]
    .iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, label)| format!("{n} {label}"))
    .collect();

    if parts.is_empty() {
        println!(
            "{}",
            "  0 findings — all clear.".green()
        );
    } else {
        println!(
            "  {} total findings: {}",
            findings.len().to_string().bold(),
            parts.join(", ")
        );
    }
}
