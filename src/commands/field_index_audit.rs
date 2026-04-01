use colored::Colorize;
use regex::Regex;
use std::collections::HashMap;

use crate::ir;

pub fn run(file: &str, struct_filter: Option<&str>) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    // Collect all extractvalue/insertvalue with named struct types
    // Pattern: extractvalue %struct.X %val, N
    let ev_re = Regex::new(
        r"(?:extractvalue|insertvalue) (%(?:struct|enum)\.\w+) .+, (\d+)"
    ).unwrap();

    // Per struct: collect all indices used
    let mut struct_indices: HashMap<String, IndexStats> = HashMap::new();

    for func in module.functions.values() {
        for line in func.body.lines() {
            let trimmed = line.trim();
            for caps in ev_re.captures_iter(trimmed) {
                let struct_name = caps[1].to_string();
                let index: usize = caps[2].parse().unwrap_or(0);

                if let Some(filter) = struct_filter {
                    if !struct_name.contains(filter) {
                        continue;
                    }
                }

                let stats = struct_indices.entry(struct_name).or_insert_with(IndexStats::new);
                *stats.index_counts.entry(index).or_default() += 1;
                stats.total += 1;

                // Track per-function usage
                let fn_stats = stats.per_function.entry(func.name.clone()).or_insert_with(HashMap::new);
                *fn_stats.entry(index).or_default() += 1;
            }
        }
    }

    if struct_indices.is_empty() {
        println!("{} No extractvalue/insertvalue on named structs found.", "culebra".green().bold());
        return 0;
    }

    println!(
        "{} Field index audit: {} struct types",
        "culebra".green().bold(),
        struct_indices.len()
    );
    println!();

    let mut alerts = 0;
    let mut sorted: Vec<_> = struct_indices.into_iter().collect();
    sorted.sort_by(|a, b| b.1.total.cmp(&a.1.total));

    for (struct_name, stats) in &sorted {
        let idx_count = stats.index_counts.len();
        let total = stats.total;
        let fn_count = stats.per_function.len();

        // Check for suspicious patterns
        let only_zero = idx_count == 1 && stats.index_counts.contains_key(&0) && total > 3;
        let mostly_zero = if let Some(&zero_count) = stats.index_counts.get(&0) {
            zero_count as f64 / total as f64 > 0.9 && total > 5 && idx_count <= 2
        } else {
            false
        };

        if only_zero {
            alerts += 1;
            println!(
                "  {} {} — ALL {} accesses use index 0 ({} functions)",
                "ALERT".red().bold(),
                struct_name.yellow().bold(),
                total,
                fn_count
            );
            println!(
                "    {} find_field_index likely returns 0 — struct may not be registered",
                "cause:".cyan().bold()
            );

            // Show top functions
            let mut fn_list: Vec<_> = stats.per_function.iter().collect();
            fn_list.sort_by(|a, b| {
                let a_total: usize = a.1.values().sum();
                let b_total: usize = b.1.values().sum();
                b_total.cmp(&a_total)
            });
            for (fname, indices) in fn_list.iter().take(5) {
                let total: usize = indices.values().sum();
                println!("      {} ({} accesses)", fname.dimmed(), total);
            }
            if fn_list.len() > 5 {
                println!("      ... and {} more", fn_list.len() - 5);
            }
            println!();
        } else if mostly_zero {
            alerts += 1;
            let zero_count = stats.index_counts.get(&0).unwrap_or(&0);
            println!(
                "  {} {} — {} of {} accesses use index 0 ({} functions)",
                "WARN".yellow().bold(),
                struct_name.yellow(),
                zero_count,
                total,
                fn_count
            );
            println!(
                "    indices used: {}",
                stats.index_counts.keys().map(|k| k.to_string()).collect::<Vec<_>>().join(", ")
            );
            println!();
        } else {
            // Normal — show summary if verbose or filtered
            if struct_filter.is_some() {
                println!(
                    "  {} {} — {} accesses across {} indices ({} functions)",
                    "OK  ".green().bold(),
                    struct_name.cyan(),
                    total,
                    idx_count,
                    fn_count
                );
                let mut idx_list: Vec<_> = stats.index_counts.iter().collect();
                idx_list.sort_by_key(|&(k, _)| k);
                println!(
                    "    indices: {}",
                    idx_list.iter().map(|(k, v)| format!("{}({}x)", k, v)).collect::<Vec<_>>().join(", ")
                );
                println!();
            }
        }
    }

    // Summary
    if alerts == 0 {
        println!("  {} No suspicious index patterns found.", "✓".green().bold());
        0
    } else {
        println!(
            "  {} {} structs with suspicious index patterns (likely unregistered)",
            "⚠".yellow().bold(),
            alerts
        );
        println!(
            "    Run 'culebra missing-types {}' to check if these structs are undefined.",
            file
        );
        1
    }
}

struct IndexStats {
    index_counts: HashMap<usize, usize>,
    per_function: HashMap<String, HashMap<usize, usize>>,
    total: usize,
}

impl IndexStats {
    fn new() -> Self {
        IndexStats {
            index_counts: HashMap::new(),
            per_function: HashMap::new(),
            total: 0,
        }
    }
}
