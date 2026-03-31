use colored::Colorize;

use crate::template::loader;
use crate::template::schema::Severity;

pub fn run_list(tags: &[String]) -> i32 {
    let templates_dir = match loader::find_templates_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("{}: No templates directory found.", "error".red().bold());
            return 1;
        }
    };

    let templates = loader::load_templates(&templates_dir);
    if templates.is_empty() {
        println!("  No templates found in {}", templates_dir.display());
        return 0;
    }

    let filtered = if tags.is_empty() {
        templates.clone()
    } else {
        loader::filter_templates(&templates, tags, &[], &[])
    };

    println!(
        "{} {} templates in {}",
        "culebra".green().bold(),
        filtered.len(),
        templates_dir.display()
    );
    println!();

    // Group by directory/category (first tag or "uncategorized")
    println!(
        "  {:<30} {:<10} {:<40} {}",
        "ID".bold(),
        "SEVERITY".bold(),
        "NAME".bold(),
        "TAGS".bold()
    );
    println!("  {}", "-".repeat(100));

    let mut sorted = filtered;
    sorted.sort_by(|a, b| a.info.severity.cmp(&b.info.severity).then(a.id.cmp(&b.id)));

    for t in &sorted {
        let sev = match t.info.severity {
            Severity::Critical => "critical".red().bold().to_string(),
            Severity::High => "high".red().to_string(),
            Severity::Medium => "medium".yellow().to_string(),
            Severity::Low => "low".blue().to_string(),
            Severity::Info => "info".dimmed().to_string(),
        };
        let tags_str = t.info.tags.join(", ");
        println!("  {:<30} {:<10} {:<40} {}", t.id, sev, t.info.name, tags_str.dimmed());
    }

    println!();
    0
}

pub fn run_show(id: &str) -> i32 {
    let templates_dir = match loader::find_templates_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("{}: No templates directory found.", "error".red().bold());
            return 1;
        }
    };

    let templates = loader::load_templates(&templates_dir);
    let Some(t) = templates.iter().find(|t| t.id == id) else {
        eprintln!("  Template '{}' not found.", id);
        return 1;
    };

    println!("{} {}", "Template:".bold(), t.id.green().bold());
    println!();
    println!("  {:<15} {}", "Name:".bold(), t.info.name);
    println!("  {:<15} {}", "Severity:".bold(), t.info.severity);
    println!("  {:<15} {}", "Author:".bold(), t.info.author);
    if !t.info.cwe.is_empty() {
        println!("  {:<15} {}", "CWE:".bold(), t.info.cwe);
    }
    println!("  {:<15} {}", "Tags:".bold(), t.info.tags.join(", "));
    println!();

    if !t.info.description.is_empty() {
        println!("  {}", "Description:".bold());
        for line in t.info.description.lines() {
            println!("    {}", line);
        }
        println!();
    }

    if !t.info.impact.is_empty() {
        println!("  {}", "Impact:".bold());
        for line in t.info.impact.lines() {
            println!("    {}", line);
        }
        println!();
    }

    if let Some(ref rem) = t.remediation {
        println!("  {}", "Remediation:".bold());
        for line in rem.suggestion.lines() {
            println!("    {}", line);
        }
        if let Some(ref fix) = rem.autofix {
            println!("    {} auto-fixable ({})", ">>".cyan(), fix.fix_type);
        }
        if let Some(ref diff) = rem.difficulty {
            println!("    Difficulty: {}", diff);
        }
        println!();
    }

    if !t.info.references.is_empty() {
        println!("  {}", "References:".bold());
        for r in &t.info.references {
            println!("    - {}", r);
        }
        println!();
    }

    if !t.related.is_empty() {
        println!("  {}", "Related:".bold());
        for r in &t.related {
            println!("    - {}", r);
        }
    }

    0
}
