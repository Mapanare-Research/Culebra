use colored::Colorize;

use crate::ir;
use crate::template::engine::{self, Finding};
use crate::template::loader;
use crate::template::report::{self, OutputFormat};
use crate::template::schema::{DrainQueue, FileType, Severity};

pub fn run(
    queue_file: &str,
    format: &str,
    autofix: bool,
    dry_run: bool,
    clear: bool,
) -> i32 {
    // Read and parse the queue file
    let content = match std::fs::read_to_string(queue_file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "{}: Failed to read queue file {}: {}",
                "error".red().bold(),
                queue_file,
                e
            );
            return 1;
        }
    };

    let queue: DrainQueue = match serde_yaml::from_str(&content) {
        Ok(q) => q,
        Err(e) => {
            eprintln!(
                "{}: Failed to parse queue file {}: {}",
                "error".red().bold(),
                queue_file,
                e
            );
            return 1;
        }
    };

    if queue.queued.is_empty() {
        println!(
            "{} Queue is empty — nothing to drain.",
            "culebra".green().bold()
        );
        return 0;
    }

    // Load all available templates
    let templates_dir = match loader::find_templates_dir() {
        Some(dir) => dir,
        None => {
            eprintln!(
                "{}: No templates directory found. Run 'culebra init' or create culebra-templates/",
                "error".red().bold()
            );
            return 1;
        }
    };

    let all_templates = loader::load_templates(&templates_dir);
    if all_templates.is_empty() {
        eprintln!("  No templates found in {}", templates_dir.display());
        return 1;
    }

    println!(
        "{} Draining {} queued entries from {}",
        "culebra".green().bold(),
        queue.queued.len(),
        queue_file
    );
    println!();

    let mut all_findings: Vec<Finding> = Vec::new();
    let mut stopped = false;

    for (i, entry) in queue.queued.iter().enumerate() {
        // Resolve which templates to run for this entry
        let selected = if let Some(ref id) = entry.template {
            // Match by ID — support both exact ("empty-switch-body") and
            // path-style ("ir/empty-switch-body") lookups
            let needle = id
                .rsplit('/')
                .next()
                .unwrap_or(id);
            all_templates
                .iter()
                .filter(|t| t.id == *id || t.id == needle)
                .cloned()
                .collect::<Vec<_>>()
        } else if !entry.tags.is_empty() {
            loader::filter_templates(&all_templates, &entry.tags, &[], &[])
        } else {
            eprintln!(
                "  {} Entry #{} has no template or tags — skipping",
                "warning".yellow().bold(),
                i + 1
            );
            continue;
        };

        if selected.is_empty() {
            let selector = entry
                .template
                .clone()
                .unwrap_or_else(|| entry.tags.join(","));
            eprintln!(
                "  {} No templates matched '{}' — skipping",
                "warning".yellow().bold(),
                selector
            );
            continue;
        }

        // Print entry header
        let reason_str = entry
            .reason
            .as_deref()
            .map(|r| format!(" — {}", r))
            .unwrap_or_default();
        println!(
            "  {} [{}/{}] {} template(s) → {}{}",
            "drain".cyan().bold(),
            i + 1,
            queue.queued.len(),
            selected.len(),
            entry.target,
            reason_str.dimmed()
        );

        // Read the target file
        let target_content = match std::fs::read_to_string(&entry.target) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "    {} Failed to read {}: {}",
                    "error".red().bold(),
                    entry.target,
                    e
                );
                continue;
            }
        };

        let module = ir::parse_ir(&target_content);
        let header_content = entry
            .header
            .as_ref()
            .and_then(|h| std::fs::read_to_string(h).ok());

        let mut entry_findings: Vec<Finding> = Vec::new();

        for template in &selected {
            if template.scope.file_type == FileType::CrossReference {
                if let Some(ref hdr) = header_content {
                    let findings =
                        engine::run_cross_reference_with_files(template, &target_content, hdr);
                    entry_findings.extend(findings);
                }
                continue;
            }

            let findings = engine::run_template(template, &module);
            entry_findings.extend(findings);
        }

        // Check stop_on condition
        if let Some(ref stop_sev) = entry.stop_on {
            let should_stop = entry_findings.iter().any(|f| f.severity <= *stop_sev);
            if should_stop {
                println!(
                    "    {} Stopping drain — findings at {} or above",
                    "stop".red().bold(),
                    stop_sev
                );
                all_findings.extend(entry_findings);
                stopped = true;
                break;
            }
        }

        all_findings.extend(entry_findings);
    }

    // Sort by severity
    all_findings.sort_by(|a, b| a.severity.cmp(&b.severity));

    // Output
    println!();
    let output_format = OutputFormat::from_str(format);
    match output_format {
        OutputFormat::Text => {
            report::print_text(&all_findings, queue_file);
            report::print_summary(&all_findings);
        }
        OutputFormat::Json => report::print_json(&all_findings, queue_file),
        OutputFormat::Sarif => report::print_sarif(&all_findings, queue_file),
        OutputFormat::Markdown => report::print_markdown(&all_findings, queue_file),
    }

    // Autofix — collect fixes grouped by target file
    if autofix && !all_findings.is_empty() {
        let fixable: Vec<_> = all_findings
            .iter()
            .filter(|f| f.autofix.is_some())
            .collect();

        if fixable.is_empty() {
            println!("\n  No autofixable findings.");
        } else if dry_run {
            println!(
                "\n  {} Dry run — {} fixes would be applied",
                "autofix".cyan().bold(),
                fixable.len()
            );
        } else {
            println!(
                "\n  {} {} fixable findings (apply per-file with culebra scan --autofix)",
                "autofix".cyan().bold(),
                fixable.len()
            );
        }
    }

    // Clear the queue file after successful drain
    if clear && !stopped {
        let empty = "queued: []\n";
        match std::fs::write(queue_file, empty) {
            Ok(_) => println!(
                "\n  {} Cleared {}",
                "drain".cyan().bold(),
                queue_file
            ),
            Err(e) => eprintln!(
                "\n  {} Failed to clear queue: {}",
                "warning".yellow().bold(),
                e
            ),
        }
    }

    // Exit code
    let has_critical = all_findings.iter().any(|f| {
        matches!(
            f.severity,
            Severity::Critical | Severity::High
        )
    });
    if has_critical {
        1
    } else {
        0
    }
}
