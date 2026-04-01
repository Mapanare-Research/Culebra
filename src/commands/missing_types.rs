use colored::Colorize;
use regex::Regex;
use std::collections::{HashMap, HashSet};

use crate::ir;

pub fn run(file: &str, verbose: bool) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    // Collect defined types
    let defined: HashSet<String> = module
        .struct_types
        .iter()
        .map(|s| s.name.clone())
        .collect();

    // Collect all used named types from the entire IR
    let type_use_re = Regex::new(r"(%(?:struct|enum)\.\w+)").unwrap();
    let type_def_re = Regex::new(r"^(%(?:struct|enum)\.\w+)\s*=\s*type\s").unwrap();

    // Also collect type definitions from raw source (in case ir parser missed some)
    let mut defined_raw: HashSet<String> = defined.clone();
    for line in content.lines() {
        if let Some(caps) = type_def_re.captures(line) {
            defined_raw.insert(caps[1].to_string());
        }
    }

    // Find all used types and where they're used
    let mut used: HashMap<String, Vec<UseSite>> = HashMap::new();

    // Check function signatures
    for func in module.functions.values() {
        for caps in type_use_re.captures_iter(&func.signature) {
            let ty = caps[1].to_string();
            if !defined_raw.contains(&ty) {
                used.entry(ty).or_default().push(UseSite {
                    function: func.name.clone(),
                    line: func.line_start,
                    context: "signature".to_string(),
                });
            }
        }

        // Check function body
        for (offset, line) in func.body.lines().enumerate() {
            for caps in type_use_re.captures_iter(line) {
                let ty = caps[1].to_string();
                if !defined_raw.contains(&ty) {
                    let ctx = if line.contains("insertvalue") {
                        "insertvalue"
                    } else if line.contains("= load") {
                        "load"
                    } else if line.contains("store ") {
                        "store"
                    } else if line.contains("alloca") {
                        "alloca"
                    } else if line.contains("getelementptr") {
                        "gep"
                    } else if line.contains("sret(") {
                        "sret"
                    } else if line.contains("byref") || line.contains("byval") {
                        "byref/byval"
                    } else {
                        "other"
                    };

                    used.entry(ty.clone()).or_default().push(UseSite {
                        function: func.name.clone(),
                        line: func.line_start + offset,
                        context: ctx.to_string(),
                    });
                }
            }
        }
    }

    // Also check declarations
    for decl in &module.declares {
        for caps in type_use_re.captures_iter(decl) {
            let ty = caps[1].to_string();
            if !defined_raw.contains(&ty) {
                used.entry(ty).or_default().push(UseSite {
                    function: "(declaration)".to_string(),
                    line: 0,
                    context: "declare".to_string(),
                });
            }
        }
    }

    if used.is_empty() {
        println!(
            "{} All named types are defined ({} types, {} functions)",
            "culebra".green().bold(),
            defined_raw.len(),
            module.functions.len()
        );
        return 0;
    }

    // Sort by number of uses (most used first)
    let mut sorted: Vec<_> = used.into_iter().collect();
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    println!(
        "{} {} undefined types found ({} defined, {} functions)",
        "culebra".green().bold(),
        sorted.len(),
        defined_raw.len(),
        module.functions.len()
    );
    println!();

    for (ty, sites) in &sorted {
        // Deduplicate functions
        let mut funcs: Vec<String> = sites.iter().map(|s| s.function.clone()).collect();
        funcs.sort();
        funcs.dedup();

        // Collect contexts
        let mut contexts: Vec<String> = sites.iter().map(|s| s.context.clone()).collect();
        contexts.sort();
        contexts.dedup();

        println!(
            "  {} {} — {} uses in {} function{} ({})",
            "MISSING".red().bold(),
            ty.yellow().bold(),
            sites.len(),
            funcs.len(),
            if funcs.len() == 1 { "" } else { "s" },
            contexts.join(", ")
        );

        if verbose {
            for func in &funcs {
                let func_sites: Vec<_> = sites.iter().filter(|s| &s.function == func).collect();
                println!(
                    "    {} ({} uses)",
                    func.dimmed(),
                    func_sites.len()
                );
            }
        }
    }

    println!();
    println!(
        "  {} Add type definitions to the IR, e.g.:",
        "fix:".green().bold()
    );
    for (ty, _) in sorted.iter().take(3) {
        println!("    {} = type {{ ... }}", ty);
    }
    if sorted.len() > 3 {
        println!("    ... and {} more", sorted.len() - 3);
    }

    1
}

struct UseSite {
    function: String,
    #[allow(dead_code)]
    line: usize,
    context: String,
}
