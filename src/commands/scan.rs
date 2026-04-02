use colored::Colorize;

use crate::ir;
use crate::c_parser;
use crate::template::engine::{self, Finding};
use crate::template::loader;
use crate::template::report::{self, OutputFormat};
use crate::template::schema::FileType;

pub fn run(
    file: &str,
    tags: &[String],
    severity: &[String],
    ids: &[String],
    template_path: Option<&str>,
    header: Option<&str>,
    format: &str,
    autofix: bool,
    dry_run: bool,
) -> i32 {
    // Load templates
    let templates_dir = if let Some(path) = template_path {
        std::path::PathBuf::from(path)
    } else {
        match loader::find_templates_dir() {
            Some(dir) => dir,
            None => {
                eprintln!(
                    "{}: No templates directory found. Run 'culebra init' or create culebra-templates/",
                    "error".red().bold()
                );
                return 1;
            }
        }
    };

    println!(
        "{} Loading templates from {}",
        "culebra".green().bold(),
        templates_dir.display()
    );

    let mut templates = loader::load_templates(&templates_dir);
    if templates.is_empty() {
        eprintln!("  No templates found in {}", templates_dir.display());
        return 1;
    }

    // Parse severity filters
    let sev_filters: Vec<_> = severity
        .iter()
        .flat_map(|s| loader::parse_severities(s))
        .collect();

    // Apply filters
    templates = loader::filter_templates(&templates, tags, &sev_filters, ids);
    println!("  {} templates selected", templates.len());

    // Read the input file
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    // Parse IR or C based on file extension
    let is_c = file.ends_with(".c") || file.ends_with(".h");
    let module = if is_c {
        // For C files, create an IRModule from the C source so regex templates work
        // The C parser extracts functions/structs, but templates match on raw text
        ir::parse_ir_from_raw(&content)
    } else {
        ir::parse_ir(&content)
    };

    // Read header file if provided (for cross-reference templates)
    let header_content = header.and_then(|h| std::fs::read_to_string(h).ok());

    // Run templates
    println!(
        "{} Scanning {}...",
        "culebra".green().bold(),
        file
    );
    println!();

    let mut all_findings: Vec<Finding> = Vec::new();

    for template in &templates {
        // Skip templates that don't match the file type
        if is_c && template.scope.file_type == FileType::LlvmIr {
            continue;
        }
        if !is_c && template.scope.file_type == FileType::CSource {
            continue;
        }
        if template.scope.file_type == FileType::CrossReference {
            if let Some(ref hdr) = header_content {
                let findings =
                    engine::run_cross_reference_with_files(template, &content, hdr);
                all_findings.extend(findings);
            }
            // Skip cross-reference templates when no header is provided
            continue;
        }

        let findings = engine::run_template(template, &module);
        all_findings.extend(findings);
    }

    // Sort by severity
    all_findings.sort_by(|a, b| a.severity.cmp(&b.severity));

    // Output
    let output_format = OutputFormat::from_str(format);
    match output_format {
        OutputFormat::Text => {
            report::print_text(&all_findings, file);
            report::print_summary(&all_findings);
        }
        OutputFormat::Json => report::print_json(&all_findings, file),
        OutputFormat::Sarif => report::print_sarif(&all_findings, file),
        OutputFormat::Markdown => report::print_markdown(&all_findings, file),
    }

    // Autofix
    if autofix && !all_findings.is_empty() {
        let fixable: Vec<_> = all_findings
            .iter()
            .filter(|f| f.autofix.is_some())
            .collect();

        if fixable.is_empty() {
            println!("\n  No autofixable findings.");
        } else {
            let fixed = engine::apply_autofixes(&content, &fixable.iter().cloned().cloned().collect::<Vec<_>>());
            if dry_run {
                println!("\n  {} Dry run — {} fixes would be applied:", "autofix".cyan().bold(), fixable.len());
                // Show diff-like output
                for (i, (orig, new)) in content.lines().zip(fixed.lines()).enumerate() {
                    if orig != new {
                        println!("    {}:{}", file, i + 1);
                        println!("    {} {}", "-".red(), orig);
                        println!("    {} {}", "+".green(), new);
                    }
                }
            } else {
                match std::fs::write(file, &fixed) {
                    Ok(_) => println!(
                        "\n  {} Applied {} fixes to {}",
                        "autofix".cyan().bold(),
                        fixable.len(),
                        file
                    ),
                    Err(e) => {
                        eprintln!("  Failed to write fixes: {}", e);
                        return 1;
                    }
                }
            }
        }
    }

    // Exit code: 1 if critical/high findings, 0 otherwise
    let has_critical = all_findings
        .iter()
        .any(|f| matches!(f.severity, crate::template::schema::Severity::Critical | crate::template::schema::Severity::High));
    if has_critical { 1 } else { 0 }
}
