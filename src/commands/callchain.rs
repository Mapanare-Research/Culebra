use colored::Colorize;
use regex::Regex;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::ir;

pub fn run(file: &str, from: &str, to: &str, max_depth: usize) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    // Build call graph: caller -> list of callees
    let call_re = Regex::new(r"call\s+.+@([\w.]+)\(").unwrap();
    let mut call_graph: HashMap<String, Vec<String>> = HashMap::new();

    for func in module.functions.values() {
        let callees: Vec<String> = call_re
            .captures_iter(&func.body)
            .map(|c| c[1].to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        call_graph.insert(func.name.clone(), callees);
    }

    // Resolve from/to — allow substring matching
    let from_fn = resolve_fn(&module, from);
    let to_fn = resolve_fn(&module, to);

    let from_name = match &from_fn {
        Some(n) => n.clone(),
        None => {
            eprintln!("{}: Function '{}' not found", "error".red().bold(), from);
            return 1;
        }
    };
    let to_name = match &to_fn {
        Some(n) => n.clone(),
        None => {
            eprintln!("{}: Function '{}' not found", "error".red().bold(), to);
            return 1;
        }
    };

    println!(
        "{} Call chain: {} → {}",
        "culebra".green().bold(),
        from_name.cyan(),
        to_name.cyan()
    );
    println!();

    // BFS to find all paths from -> to (up to max_depth)
    let paths = find_paths(&call_graph, &from_name, &to_name, max_depth);

    if paths.is_empty() {
        println!("  No call path found within {} hops.", max_depth);

        // Show what from_fn calls
        if let Some(callees) = call_graph.get(&from_name) {
            let mut sorted = callees.clone();
            sorted.sort();
            println!();
            println!("  {} calls: {}", from_name.cyan(), sorted.join(", "));
        }

        // Show what calls to_fn
        let callers: Vec<_> = call_graph.iter()
            .filter(|(_, callees)| callees.contains(&to_name))
            .map(|(caller, _)| caller.as_str())
            .collect();
        if !callers.is_empty() {
            println!("  {} called by: {}", to_name.cyan(), callers.join(", "));
        }

        return 1;
    }

    // Print paths with struct info
    for (i, path) in paths.iter().enumerate() {
        println!("  Path {} ({} hops):", i + 1, path.len() - 1);
        for (j, fn_name) in path.iter().enumerate() {
            let indent = "  ".repeat(j + 1);
            let metrics = module.functions.get(fn_name);
            let info = if let Some(f) = metrics {
                format!(
                    "{}i, {}c, {}a",
                    f.metrics.instructions,
                    f.metrics.calls,
                    f.metrics.allocas
                )
            } else {
                "(external)".to_string()
            };

            let arrow = if j < path.len() - 1 { "→" } else { "●" };
            let colored_name = if fn_name == &from_name || fn_name == &to_name {
                fn_name.cyan().bold().to_string()
            } else {
                fn_name.to_string()
            };

            println!(
                "  {} {} {} ({})",
                indent,
                arrow,
                colored_name,
                info.dimmed()
            );
        }
        println!();
    }

    // Show struct types passed along the chain (for the shortest path)
    if let Some(shortest) = paths.first() {
        let struct_re = Regex::new(r"%struct\.(\w+)").unwrap();
        let mut chain_structs: Vec<(String, HashSet<String>)> = Vec::new();

        for fn_name in shortest {
            if let Some(func) = module.functions.get(fn_name) {
                let structs: HashSet<String> = struct_re
                    .captures_iter(&func.signature)
                    .map(|c| format!("%struct.{}", &c[1]))
                    .collect();
                if !structs.is_empty() {
                    chain_structs.push((fn_name.clone(), structs));
                }
            }
        }

        if !chain_structs.is_empty() {
            println!("  {} Struct types along the chain:", "Structs:".bold());
            for (fn_name, structs) in &chain_structs {
                let structs_str: Vec<_> = structs.iter().map(|s| s.as_str()).collect();
                println!("    {} → {}", fn_name.dimmed(), structs_str.join(", ").yellow());
            }
        }
    }

    0
}

fn resolve_fn(module: &ir::IRModule, name: &str) -> Option<String> {
    // Exact match first
    if module.functions.contains_key(name) {
        return Some(name.to_string());
    }
    // Substring match
    let matches: Vec<_> = module.functions.keys()
        .filter(|k| k.contains(name))
        .collect();
    if matches.len() == 1 {
        return Some(matches[0].clone());
    }
    if matches.len() > 1 {
        // Prefer shortest match (most specific)
        return matches.into_iter().min_by_key(|k| k.len()).cloned();
    }
    None
}

fn find_paths(
    graph: &HashMap<String, Vec<String>>,
    from: &str,
    to: &str,
    max_depth: usize,
) -> Vec<Vec<String>> {
    let mut results = Vec::new();
    let mut queue: VecDeque<Vec<String>> = VecDeque::new();
    queue.push_back(vec![from.to_string()]);

    while let Some(path) = queue.pop_front() {
        if path.len() > max_depth + 1 {
            continue;
        }

        let current = path.last().unwrap();

        if current == to && path.len() > 1 {
            results.push(path);
            if results.len() >= 5 {
                break; // Limit to 5 paths
            }
            continue;
        }

        if let Some(callees) = graph.get(current) {
            for callee in callees {
                if !path.contains(callee) {
                    let mut new_path = path.clone();
                    new_path.push(callee.clone());
                    queue.push_back(new_path);
                }
            }
        }
    }

    results.sort_by_key(|p| p.len());
    results
}
