use colored::Colorize;
use regex::Regex;

use crate::ir;

pub fn run(file: &str, struct_name: Option<&str>) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    // Build search pattern for the target struct(s)
    let struct_patterns: Vec<String> = if let Some(name) = struct_name {
        vec![
            format!("%struct.{}", name),
            format!("%enum.{}", name),
            format!("%{}", name),
        ]
    } else {
        // All struct types
        module.struct_types.iter().map(|s| s.name.clone()).collect()
    };

    let label = struct_name.unwrap_or("all structs");

    println!(
        "{} Struct health check: {} in {}",
        "culebra".green().bold(),
        label.bold(),
        file
    );
    println!();

    let mut issues = 0;

    // Check 1: PHI with zeroinitializer on struct types
    let phi_zeroinit_re = Regex::new(r"= phi (%[\w.]+) .+zeroinitializer").unwrap();
    for func in module.functions.values() {
        for (offset, line) in func.body.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(caps) = phi_zeroinit_re.captures(trimmed) {
                let ty = &caps[1];
                if matches_any(ty, &struct_patterns) {
                    issues += 1;
                    println!(
                        "  {} [phi-zeroinit] line {} in {} — {} PHI has zeroinitializer arm",
                        "WARN".yellow().bold(),
                        func.line_start + offset,
                        func.name.cyan(),
                        ty.yellow()
                    );
                    println!("    {}", trimmed.dimmed());
                }
            }
        }
    }

    // Check 2: Store of zeroinitializer to struct alloca
    let store_zeroinit_re = Regex::new(r"store (%[\w.]+) zeroinitializer, ptr (%[\w.]+)").unwrap();
    for func in module.functions.values() {
        for (offset, line) in func.body.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(caps) = store_zeroinit_re.captures(trimmed) {
                let ty = &caps[1];
                if matches_any(ty, &struct_patterns) {
                    issues += 1;
                    println!(
                        "  {} [store-zeroinit] line {} in {} — {} stored as zeroinitializer",
                        "WARN".yellow().bold(),
                        func.line_start + offset,
                        func.name.cyan(),
                        ty.yellow()
                    );
                    println!("    {}", trimmed.dimmed());
                }
            }
        }
    }

    // Check 3: Type-pun store (smaller type stored to struct alloca)
    // Look for: store {small_type} %val, ptr %alloca where alloca is a larger struct
    let alloca_re = Regex::new(r"(%[\w.]+)\s*=\s*alloca\s+(%[\w.]+(?:\s*\{[^}]*\})?)").unwrap();
    let store_re = Regex::new(r"store\s+(\S+)\s+.+,\s*ptr\s+(%[\w.]+)").unwrap();

    for func in module.functions.values() {
        let mut allocas: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        for line in func.body.lines() {
            let trimmed = line.trim();
            if let Some(caps) = alloca_re.captures(trimmed) {
                allocas.insert(caps[1].to_string(), caps[2].to_string());
            }
        }

        for (offset, line) in func.body.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(caps) = store_re.captures(trimmed) {
                let store_ty = &caps[1];
                let dest = &caps[2];
                if let Some(alloca_ty) = allocas.get(dest) {
                    if matches_any(alloca_ty, &struct_patterns) && store_ty != alloca_ty && store_ty != "zeroinitializer" {
                        // Type mismatch — possible type pun
                        if !trimmed.contains("zeroinitializer") {
                            issues += 1;
                            println!(
                                "  {} [type-pun] line {} in {} — store {} to {} alloca ({})",
                                "WARN".yellow().bold(),
                                func.line_start + offset,
                                func.name.cyan(),
                                store_ty.red(),
                                dest,
                                alloca_ty.yellow()
                            );
                            println!("    {}", trimmed.dimmed());
                        }
                    }
                }
            }
        }
    }

    // Check 4: Load of struct at suspicious named types
    let load_re = Regex::new(r"= load (%[\w.]+), ptr null").unwrap();
    for func in module.functions.values() {
        for (offset, line) in func.body.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(caps) = load_re.captures(trimmed) {
                let ty = &caps[1];
                if matches_any(ty, &struct_patterns) {
                    issues += 1;
                    println!(
                        "  {} [null-load] line {} in {} — load {} from null pointer",
                        "CRIT".red().bold(),
                        func.line_start + offset,
                        func.name.cyan(),
                        ty.yellow()
                    );
                    println!("    {}", trimmed.dimmed());
                }
            }
        }
    }

    // Summary
    println!();
    if issues == 0 {
        println!(
            "  {} No struct health issues found for {}",
            "✓".green().bold(),
            label
        );
        0
    } else {
        println!(
            "  {} {} struct health issues found for {}",
            "⚠".yellow().bold(),
            issues,
            label
        );
        1
    }
}

fn matches_any(ty: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| ty == p || ty.starts_with(&format!("{}.", p)))
}
