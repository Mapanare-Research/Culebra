use colored::Colorize;
use std::collections::{HashMap, HashSet};

use crate::template::loader;
use crate::template::schema::{Severity, Template};

pub fn run(query: &str, show_all: bool) -> i32 {
    let templates_dir = match loader::find_templates_dir() {
        Some(dir) => dir,
        None => {
            eprintln!(
                "{}: No templates directory found.",
                "error".red().bold()
            );
            return 1;
        }
    };

    let templates = loader::load_templates(&templates_dir);
    if templates.is_empty() {
        eprintln!("  No templates found.");
        return 1;
    }

    let query_lower = query.to_lowercase();
    let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

    // Score each template against the query
    let mut scored: Vec<(usize, &Template)> = templates
        .iter()
        .map(|t| (score_template(t, &query_terms), t))
        .filter(|(s, _)| *s > 0)
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));

    if scored.is_empty() {
        println!(
            "{} No templates match '{}'. Try broader terms like: segfault, type, abi, phi, list, break, stack, enum",
            "culebra".green().bold(),
            query
        );
        return 0;
    }

    // Collect all related IDs to build the graph
    let all_ids: HashSet<String> = templates.iter().map(|t| t.id.clone()).collect();
    let id_to_template: HashMap<&str, &Template> =
        templates.iter().map(|t| (t.id.as_str(), t)).collect();

    // Expand via `related` links — one hop from direct matches
    let direct_ids: HashSet<String> = scored.iter().map(|(_, t)| t.id.clone()).collect();
    let mut expanded_ids: HashSet<String> = direct_ids.clone();
    for (_, t) in &scored {
        for rel in &t.related {
            if all_ids.contains(rel) {
                expanded_ids.insert(rel.clone());
            }
        }
    }

    // Add related templates that weren't direct matches
    let mut related_only: Vec<&Template> = Vec::new();
    for id in &expanded_ids {
        if !direct_ids.contains(id) {
            if let Some(t) = id_to_template.get(id.as_str()) {
                related_only.push(t);
            }
        }
    }

    // Print the map
    println!();
    println!(
        "  {} Diagnostic map for '{}'",
        "culebra".green().bold(),
        query.bold()
    );
    println!();

    // Group direct matches by severity
    let limit = if show_all { scored.len() } else { scored.len().min(12) };
    let display_set: Vec<&(usize, &Template)> = scored.iter().take(limit).collect();

    // Severity grouping
    let mut by_severity: Vec<(Severity, Vec<&Template>)> = vec![
        (Severity::Critical, Vec::new()),
        (Severity::High, Vec::new()),
        (Severity::Medium, Vec::new()),
        (Severity::Low, Vec::new()),
        (Severity::Info, Vec::new()),
    ];

    for (_, t) in &display_set {
        for (sev, group) in &mut by_severity {
            if t.info.severity == *sev {
                group.push(t);
                break;
            }
        }
    }

    for (sev, group) in &by_severity {
        if group.is_empty() {
            continue;
        }
        let sev_str = format!("{}", sev);
        let colored_sev = match sev {
            Severity::Critical => sev_str.red().bold().to_string(),
            Severity::High => sev_str.yellow().bold().to_string(),
            Severity::Medium => sev_str.cyan().to_string(),
            Severity::Low => sev_str.dimmed().to_string(),
            Severity::Info => sev_str.dimmed().to_string(),
        };
        println!("  {} {}", "---".dimmed(), colored_sev);

        for t in group {
            let tag_str = t.info.tags.join(", ");
            println!(
                "    {} {}",
                bullet_for_severity(&t.info.severity),
                t.id.bold()
            );
            println!("      {}", t.info.name);
            // One-line impact summary
            let impact_line = first_sentence(&t.info.impact);
            if !impact_line.is_empty() {
                println!("      {}", impact_line.dimmed());
            }
            println!("      tags: {}", tag_str.dimmed());

            // Show related links
            if !t.related.is_empty() {
                let rels: Vec<String> = t
                    .related
                    .iter()
                    .map(|r| {
                        if direct_ids.contains(r) {
                            format!("{}", r.bold())
                        } else if expanded_ids.contains(r) {
                            format!("{}", r)
                        } else {
                            format!("{}", r.dimmed())
                        }
                    })
                    .collect();
                println!("      {} {}", "see also:".dimmed(), rels.join(", "));
            }
            println!();
        }
    }

    // Show related-only templates (one hop away)
    if !related_only.is_empty() {
        println!(
            "  {} {}",
            "---".dimmed(),
            "related (one hop)".dimmed()
        );
        for t in &related_only {
            println!(
                "    {} {} — {}",
                "~".dimmed(),
                t.id,
                first_sentence(&t.info.description).dimmed()
            );
        }
        println!();
    }

    // Summary line
    let direct_count = display_set.len();
    let related_count = related_only.len();
    let total = direct_count + related_count;
    let crit_count = by_severity[0].1.len();
    let high_count = by_severity[1].1.len();

    print!("  {} templates mapped", total);
    if crit_count > 0 || high_count > 0 {
        print!(" (");
        let mut parts = Vec::new();
        if crit_count > 0 {
            parts.push(format!("{} critical", crit_count));
        }
        if high_count > 0 {
            parts.push(format!("{} high", high_count));
        }
        print!("{}", parts.join(", "));
        print!(")");
    }
    println!();

    if !show_all && scored.len() > limit {
        println!(
            "  {} more — use --all to show everything",
            scored.len() - limit
        );
    }

    // Suggest scan command
    println!();
    let top_tags: Vec<String> = collect_top_tags(&display_set);
    if !top_tags.is_empty() {
        println!(
            "  {} culebra scan <file.ll> --tags {}",
            "try:".green().bold(),
            top_tags.join(",")
        );
    }

    println!();
    0
}

/// Score a template against query terms. Higher = better match.
fn score_template(t: &Template, terms: &[&str]) -> usize {
    let mut score = 0;

    for term in terms {
        // ID match (strongest)
        if t.id.to_lowercase().contains(term) {
            score += 10;
        }

        // Tag match (strong)
        for tag in &t.info.tags {
            if tag.to_lowercase().contains(term) {
                score += 8;
            }
        }

        // Name match
        if t.info.name.to_lowercase().contains(term) {
            score += 5;
        }

        // Description match
        if t.info.description.to_lowercase().contains(term) {
            score += 3;
        }

        // Impact match
        if t.info.impact.to_lowercase().contains(term) {
            score += 3;
        }

        // CWE match
        if !t.info.cwe.is_empty() && t.info.cwe.to_lowercase().contains(term) {
            score += 4;
        }

        // Related match (weak — it's a pointer not a description)
        for rel in &t.related {
            if rel.to_lowercase().contains(term) {
                score += 1;
            }
        }
    }

    // Severity boost
    match t.info.severity {
        Severity::Critical => score += score / 3,
        Severity::High => score += score / 5,
        _ => {}
    }

    score
}

fn bullet_for_severity(sev: &Severity) -> String {
    match sev {
        Severity::Critical => "!!!".red().bold().to_string(),
        Severity::High => " !!".yellow().bold().to_string(),
        Severity::Medium => "  !".cyan().to_string(),
        Severity::Low => "  -".dimmed().to_string(),
        Severity::Info => "  .".dimmed().to_string(),
    }
}

fn first_sentence(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Take first line or first sentence
    let first_line = trimmed.lines().next().unwrap_or("");
    if let Some(dot) = first_line.find(". ") {
        first_line[..=dot].to_string()
    } else {
        first_line.to_string()
    }
}

fn collect_top_tags(templates: &[&(usize, &Template)]) -> Vec<String> {
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    for (_, t) in templates.iter() {
        for tag in &t.info.tags {
            *tag_counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }
    let broad = ["ir", "abi", "bootstrap", "binary"];
    let mut tags: Vec<(String, usize)> = tag_counts
        .into_iter()
        .filter(|(tag, _)| !broad.iter().any(|b| b == tag))
        .collect();
    tags.sort_by(|a, b| b.1.cmp(&a.1));
    tags.into_iter().take(3).map(|(t, _)| t).collect()
}
