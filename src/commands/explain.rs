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
        // Diagnostic: explain WHY there were 0 matches
        println!(
            "{} No matches for '{}'{} in {}",
            "culebra".green().bold(),
            finding_id,
            function.map(|f| format!(" in function '{}'", f)).unwrap_or_default(),
            file
        );
        println!();
        println!("  {}", "Diagnosis:".bold());
        println!("    Template loaded: {}", "yes".green());
        println!("    Scope:           {} / {:?}",
            format!("{:?}", template.scope.file_type),
            template.scope.section
        );
        println!("    Functions in IR: {}", module.functions.len());
        println!("    Declarations:    {}", module.declares.len());

        if template.scope.file_type == FileType::CrossReference {
            println!("    {} This template requires --header for cross-reference matching.", "note:".yellow().bold());
            println!("    Run: culebra scan {} --id {} --header <runtime.h>", file, finding_id);
        } else if findings.is_empty() {
            // Template ran but produced 0 findings
            println!("    Matches (all):   0 — template regex did not match any IR lines");
            // Show what the template is looking for
            println!();
            println!("    {} The template pattern did not match anything in this file.", "reason:".yellow().bold());
            println!("    This means the bug pattern is absent — either it's already fixed,");
            println!("    or the IR uses a different code shape than the template expects.");
            println!("    Run 'culebra templates show {}' to see the exact patterns.", finding_id);
        } else if function.is_some() {
            // Template matched but not in the specified function
            let matched_fns: Vec<_> = findings.iter()
                .filter_map(|f| f.function.as_deref())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            println!("    Matches (all):   {} — but none in the specified function", findings.len());
            println!("    Matched in:      {}",
                if matched_fns.is_empty() { "(global scope)".to_string() }
                else { matched_fns.into_iter().collect::<Vec<_>>().join(", ") }
            );
        }
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
