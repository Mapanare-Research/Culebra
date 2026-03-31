use colored::Colorize;

use crate::ir;
use crate::template::engine::{self, Finding};
use crate::template::loader;
use crate::template::schema::{FileType, Severity};

pub fn run(file: &str, function: &str) -> i32 {
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

    // Run all templates
    let mut all_findings: Vec<Finding> = Vec::new();
    for t in &templates {
        if t.scope.file_type == FileType::CrossReference {
            continue;
        }
        all_findings.extend(engine::run_template(t, &module));
    }

    // Filter to target function
    let filtered: Vec<&Finding> = all_findings.iter().filter(|f| {
        f.function.as_deref().map(|n| n.contains(function)).unwrap_or(false)
    }).collect();

    if filtered.is_empty() {
        println!(
            "{} No findings in function '{}' — nothing to suggest.",
            "culebra".green().bold(),
            function
        );
        return 0;
    }

    // Group by severity
    let critical: Vec<_> = filtered.iter().filter(|f| f.severity == Severity::Critical).collect();
    let high: Vec<_> = filtered.iter().filter(|f| f.severity == Severity::High).collect();
    let medium: Vec<_> = filtered.iter().filter(|f| f.severity == Severity::Medium).collect();

    println!(
        "{} Suggestions for {} ({} findings: {} critical, {} high, {} medium):",
        "culebra".green().bold(),
        function.cyan().bold(),
        filtered.len(),
        critical.len(), high.len(), medium.len()
    );
    println!();

    // Deduplicate by template_id
    let mut seen = std::collections::HashSet::new();
    let mut priority = 1;

    // Critical first
    for f in critical.iter().chain(high.iter()).chain(medium.iter()) {
        if !seen.insert(&f.template_id) {
            continue;
        }

        let sev_str = match f.severity {
            Severity::Critical => "critical".red().bold().to_string(),
            Severity::High => "high".yellow().bold().to_string(),
            Severity::Medium => "medium".cyan().to_string(),
            _ => format!("{}", f.severity),
        };

        println!(
            "  {}. [{}] {}",
            priority,
            sev_str,
            f.template_id.bold()
        );

        // Show concise description
        if !f.description.is_empty() {
            let first_line = f.description.lines().next().unwrap_or("");
            println!("     {}", first_line.dimmed());
        }

        // Show actionable fix
        if !f.suggestion.is_empty() {
            println!("     {}", "Fix:".green().bold());
            for line in f.suggestion.lines().take(4) {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    println!("       {}", trimmed.green());
                }
            }
        }

        // Context-specific hints based on the template
        print_contextual_hint(&f.template_id, function);

        println!();
        priority += 1;
    }

    // Overall recommendation
    if !critical.is_empty() {
        println!(
            "  {} Fix critical issues first. They cause crashes and silent data corruption.",
            "→".green().bold()
        );
    }
    if seen.contains(&"match-phi-zeroinit-corruption".to_string()) ||
       seen.contains(&"dropped-else-branch".to_string()) ||
       seen.contains(&"break-inside-nested-control".to_string()) {
        println!(
            "  {} These are Python lowerer bugs. Restructure .mn source to avoid the patterns.",
            "→".green().bold()
        );
        println!(
            "    Common fix: extract match arms into helper functions with explicit return.",
        );
    }

    if filtered.len() > 5 {
        println!();
        println!(
            "  {} Run 'culebra baseline save {}' before fixing, then 'culebra baseline diff' after.",
            "tip:".cyan().bold(),
            file
        );
    }

    if critical.is_empty() { 0 } else { 1 }
}

fn print_contextual_hint(template_id: &str, function: &str) {
    match template_id {
        "option-type-pun-zeroinit" => {
            println!("     {} Check if {} passes an enum variant where Option<T> is expected.", "hint:".cyan(), function);
            println!("       Wrap with Some() explicitly in the .mn source.");
        }
        "break-inside-nested-control" => {
            println!("     {} Replace break with flag: let mut done = false; if !done {{ if cond {{ done = true }} }}", "hint:".cyan());
        }
        "return-inside-nested-block" => {
            println!("     {} Extract the match/if body into a helper function with early return.", "hint:".cyan());
        }
        "dynamic-alloca-non-entry" => {
            println!("     {} Move allocas to function entry block or use a pre-entry pattern.", "hint:".cyan());
        }
        "phi-operand-type-mismatch" | "match-phi-zeroinit-corruption" => {
            println!("     {} The match statement generates a dead PHI. Restructure to use helper functions.", "hint:".cyan());
        }
        _ => {}
    }
}
