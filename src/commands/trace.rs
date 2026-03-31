use colored::Colorize;
use regex::Regex;

use crate::ir;

pub fn run(file: &str, function: &str, var: &str) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    // Find function (exact or substring match)
    let func = module.functions.values().find(|f| {
        f.name == function || f.name.contains(function)
    });

    let func = match func {
        Some(f) => f,
        None => {
            eprintln!(
                "{}: Function '{}' not found in {}",
                "error".red().bold(),
                function,
                file
            );
            // Show close matches
            let close: Vec<_> = module.functions.keys()
                .filter(|k| k.contains(&function[..function.len().min(5).max(1)]))
                .take(10)
                .collect();
            if !close.is_empty() {
                eprintln!("  Similar: {}", close.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
            return 1;
        }
    };

    // Normalize var name — ensure % prefix
    let var_name = if var.starts_with('%') {
        var.to_string()
    } else {
        format!("%{}", var)
    };

    println!(
        "{} Trace: {} in {}",
        "culebra".green().bold(),
        var_name.yellow().bold(),
        func.name.cyan()
    );
    println!();

    let body_lines: Vec<&str> = func.body.lines().collect();
    let mut hits = 0;
    let mut current_block = "entry";

    // Patterns to match variable usage
    let def_re = Regex::new(&format!(
        r"{}(?:\.\w+)?\s*=",
        regex::escape(&var_name)
    )).unwrap();

    let use_re = Regex::new(&format!(
        r"(?:store|load|call|phi|insertvalue|extractvalue|getelementptr|icmp|br|ret|add|sub|mul|and|or|xor)\s+.*{}",
        regex::escape(&var_name)
    )).unwrap();

    let block_re = Regex::new(r"^([a-zA-Z_][\w.]*):").unwrap();

    for (i, line) in body_lines.iter().enumerate() {
        let trimmed = line.trim();

        // Track current basic block
        if let Some(caps) = block_re.captures(trimmed) {
            current_block = caps.get(1).unwrap().as_str();
            // Only print block header if it has relevant lines
        }

        if trimmed.is_empty() {
            continue;
        }

        let is_def = def_re.is_match(trimmed);
        let is_use = !is_def && use_re.is_match(trimmed);

        if is_def || is_use {
            let kind = if is_def { "DEF" } else { "USE" };
            let kind_colored = if is_def {
                kind.green().bold().to_string()
            } else {
                kind.yellow().to_string()
            };

            // Classify the operation
            let op = classify_operation(trimmed);

            println!(
                "  {:>4} [{:<12}] [{}] {} {}",
                func.line_start + i,
                current_block.dimmed(),
                kind_colored,
                op.cyan(),
                trimmed.trim()
            );
            hits += 1;
        }
    }

    if hits == 0 {
        println!("  No references to {} found in {}", var_name, func.name);
        println!();
        // Show available variables
        let alloca_re = Regex::new(r"(%[\w.]+)\s*=\s*alloca").unwrap();
        let vars: Vec<_> = body_lines.iter()
            .filter_map(|l| alloca_re.captures(l).map(|c| c[1].to_string()))
            .take(20)
            .collect();
        if !vars.is_empty() {
            println!("  Available allocas: {}", vars.join(", "));
        }
    } else {
        println!();
        println!("  {} references across function body", hits);
    }

    0
}

fn classify_operation(line: &str) -> &'static str {
    let trimmed = line.trim();
    if trimmed.contains("= alloca ") { return "alloca"; }
    if trimmed.contains("= load ") { return "load"; }
    if trimmed.starts_with("store ") { return "store"; }
    if trimmed.contains("= phi ") { return "phi"; }
    if trimmed.contains("= call ") || trimmed.starts_with("call ") { return "call"; }
    if trimmed.contains("= getelementptr ") { return "gep"; }
    if trimmed.contains("= insertvalue ") { return "insert"; }
    if trimmed.contains("= extractvalue ") { return "extract"; }
    if trimmed.contains("= icmp ") { return "cmp"; }
    if trimmed.starts_with("br ") { return "branch"; }
    if trimmed.starts_with("ret ") { return "ret"; }
    if trimmed.contains("= add ") || trimmed.contains("= sub ") { return "arith"; }
    "other"
}
