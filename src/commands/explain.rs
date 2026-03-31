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

    // Find matching template
    let template = match templates.iter().find(|t| t.id == finding_id) {
        Some(t) => t,
        None => {
            eprintln!(
                "{}: Template '{}' not found. Run 'culebra templates list' to see available IDs.",
                "error".red().bold(),
                finding_id
            );
            return 1;
        }
    };

    // Run just this template
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

    if filtered.is_empty() {
        println!(
            "{} No matches for '{}'{} in {}",
            "culebra".green().bold(),
            finding_id,
            function.map(|f| format!(" in function '{}'", f)).unwrap_or_default(),
            file
        );
        return 0;
    }

    // Print template info
    println!("{} [{}] {}", "culebra".green().bold(), finding_id.bold(), template.info.name);
    println!();
    println!("  {}", "Description:".bold());
    for line in template.info.description.lines() {
        println!("    {}", line);
    }

    if !template.info.impact.is_empty() {
        println!();
        println!("  {}", "Impact:".bold());
        for line in template.info.impact.lines() {
            println!("    {}", line);
        }
    }

    // Show each finding with context
    println!();
    println!("  {} ({} match{}):", "Findings".bold(), filtered.len(), if filtered.len() == 1 { "" } else { "es" });
    println!();

    let source_lines: Vec<&str> = content.lines().collect();

    for finding in &filtered {
        let func_label = finding.function.as_deref().unwrap_or("(global)");
        println!(
            "  {} {} line {} in {}",
            format!("[{}]", finding.severity).red().bold(),
            file,
            finding.line,
            func_label.cyan()
        );

        // Show context: 2 lines before, the match, 2 lines after
        let line_idx = finding.line.saturating_sub(1); // 0-indexed
        let start = line_idx.saturating_sub(2);
        let end = (line_idx + 3).min(source_lines.len());

        for i in start..end {
            let prefix = if i == line_idx { ">>" } else { "  " };
            let line_text = source_lines[i];
            if i == line_idx {
                println!("    {} {:>5} | {}", prefix.red().bold(), i + 1, line_text.yellow());
            } else {
                println!("    {} {:>5} | {}", prefix.dimmed(), i + 1, line_text.dimmed());
            }
        }
        println!();
    }

    // Remediation
    if let Some(ref rem) = template.remediation {
        if !rem.suggestion.is_empty() {
            println!("  {}", "Remediation:".bold());
            for line in rem.suggestion.lines() {
                println!("    {}", line.green());
            }
        }
        if rem.autofix.is_some() {
            println!();
            println!(
                "    {} Run with --autofix to apply automatically",
                "tip:".cyan().bold()
            );
        }
    }

    0
}
