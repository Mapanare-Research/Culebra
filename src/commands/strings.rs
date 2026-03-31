use crate::ir;
use colored::Colorize;
use std::collections::HashMap;

pub fn run(file: &str, verbose: bool, json: bool) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    let module = ir::parse_ir(&text);
    let constants = &module.string_constants;

    let mismatches: Vec<_> = constants
        .iter()
        .filter(|c| c.declared_size != c.actual_size)
        .collect();

    // Duplicates
    let mut content_map: HashMap<&str, Vec<&str>> = HashMap::new();
    for c in constants {
        content_map.entry(&c.content).or_default().push(&c.name);
    }
    let n_duplicates: usize = content_map.values().map(|v| v.len().saturating_sub(1)).sum();

    // Large constants
    let large: Vec<_> = constants.iter().filter(|c| c.declared_size > 512).collect();

    if json {
        let result = serde_json::json!({
            "total": constants.len(),
            "mismatches": mismatches.len(),
            "duplicates": n_duplicates,
            "large": large.len(),
            "details": mismatches.iter().map(|c| serde_json::json!({
                "name": c.name,
                "declared": c.declared_size,
                "actual": c.actual_size,
                "line": c.line,
                "delta": c.declared_size as i64 - c.actual_size as i64,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
        return if mismatches.is_empty() { 0 } else { 1 };
    }

    if mismatches.is_empty() {
        println!(
            "{} All {} string constants have correct byte counts.",
            "OK".green().bold(),
            constants.len()
        );
    } else {
        println!(
            "{} ({}):",
            "BYTE-COUNT MISMATCHES".red().bold(),
            mismatches.len()
        );
        for c in &mismatches {
            let delta = c.declared_size as i64 - c.actual_size as i64;
            let preview: String = c.content.chars().take(60).collect();
            println!(
                "  L{} {}: [{} x i8] but content is {} bytes (off by {})",
                c.line, c.name, c.declared_size, c.actual_size, delta
            );
            println!("    c\"{}{}\"", preview, if c.content.len() > 60 { "..." } else { "" });
        }
    }

    if n_duplicates > 0 {
        let unique_dups = content_map.values().filter(|v| v.len() > 1).count();
        println!(
            "\n{} duplicate string constants ({} unique values repeated).",
            n_duplicates, unique_dups
        );
        if verbose {
            let mut dups: Vec<_> = content_map
                .iter()
                .filter(|(_, v)| v.len() > 1)
                .collect();
            dups.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
            for (content, names) in dups.iter().take(20) {
                let preview: String = content.chars().take(40).collect();
                let preview = preview.replace('\n', "\\n");
                let name_list: String = names.iter().take(3).copied().collect::<Vec<_>>().join(", ");
                println!("  {}x: c\"{}\" -> {}...", names.len(), preview, name_list);
            }
        }
    }

    if !large.is_empty() {
        println!("\n{} large constants (>512 bytes):", large.len());
        let mut sorted = large.clone();
        sorted.sort_by(|a, b| b.declared_size.cmp(&a.declared_size));
        for c in sorted.iter().take(10) {
            println!("  {}: [{} x i8]", c.name, c.declared_size);
        }
    }

    if mismatches.is_empty() { 0 } else { 1 }
}
