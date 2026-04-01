use colored::Colorize;
use regex::Regex;

use crate::ir;

pub fn run(file: &str, function: &str, verbose: bool) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    let func = match module.functions.values().find(|f| f.name == function || f.name.contains(function)) {
        Some(f) => f,
        None => {
            eprintln!("{}: Function '{}' not found", "error".red().bold(), function);
            return 1;
        }
    };

    println!(
        "{} Variable dump: {} ({} instructions)",
        "culebra".green().bold(),
        func.name.cyan().bold(),
        func.metrics.instructions
    );
    println!();

    let alloca_re = Regex::new(r"(%[\w.]+)\s*=\s*alloca\s+(.+?)(?:\s*,\s*align\s+\d+)?$").unwrap();
    let store_re = Regex::new(r"store\s+(\S+)\s+(.+?),\s*ptr\s+(%[\w.]+)").unwrap();
    let load_re = Regex::new(r"(%[\w.]+)\s*=\s*load\s+(\S+),\s*ptr\s+(%[\w.]+)").unwrap();
    let call_re = Regex::new(r"(%[\w.]+)\s*=\s*call\s+(\S+)\s+@([\w.]+)\(").unwrap();
    let call_void_re = Regex::new(r"call\s+void\s+@([\w.]+)\(.*ptr\s+(%[\w.]+)").unwrap();
    let phi_re = Regex::new(r"(%[\w.]+)\s*=\s*phi\s+(\S+)\s+(.+)").unwrap();
    let gep_re = Regex::new(r"(%[\w.]+)\s*=\s*getelementptr\s+\S+\s+(\S+),\s*ptr\s+(%[\w.]+)(.*)").unwrap();

    // Pass 1: collect allocas
    let mut allocas: Vec<(String, String, usize)> = Vec::new(); // name, type, line
    let mut stores: Vec<(String, String, String)> = Vec::new(); // dest, type, value
    let mut loads: Vec<(String, String, String)> = Vec::new(); // result, type, source
    let mut calls: Vec<(String, String, String)> = Vec::new(); // result, type, callee
    let mut phis: Vec<(String, String, String)> = Vec::new(); // result, type, entries
    let mut geps: Vec<(String, String, String)> = Vec::new(); // result, base, indices

    for (i, line) in func.body.lines().enumerate() {
        let trimmed = line.trim();

        if let Some(caps) = alloca_re.captures(trimmed) {
            allocas.push((caps[1].to_string(), caps[2].to_string(), func.line_start + i));
        }
        if let Some(caps) = store_re.captures(trimmed) {
            stores.push((caps[3].to_string(), caps[1].to_string(), caps[2].to_string()));
        }
        if let Some(caps) = load_re.captures(trimmed) {
            loads.push((caps[1].to_string(), caps[2].to_string(), caps[3].to_string()));
        }
        if let Some(caps) = call_re.captures(trimmed) {
            calls.push((caps[1].to_string(), caps[2].to_string(), caps[3].to_string()));
        }
        if let Some(caps) = phi_re.captures(trimmed) {
            phis.push((caps[1].to_string(), caps[2].to_string(), caps[3].to_string()));
        }
        if let Some(caps) = gep_re.captures(trimmed) {
            geps.push((caps[1].to_string(), caps[3].to_string(), caps[4].to_string()));
        }
    }

    // Allocas
    if !allocas.is_empty() {
        println!("  {} ({}):", "Allocas".bold(), allocas.len());
        for (name, ty, line) in &allocas {
            let store_count = stores.iter().filter(|(d, _, _)| d == name).count();
            let load_count = loads.iter().filter(|(_, _, s)| s == name).count();
            let size = estimate_size(ty);
            println!(
                "    {} {} ({} bytes) — {}w {}r  L{}",
                name.yellow(),
                ty.dimmed(),
                size,
                store_count,
                load_count,
                line
            );
        }
        println!();
    }

    // Call results
    if !calls.is_empty() {
        println!("  {} ({}):", "Calls".bold(), calls.len());
        for (result, ret_ty, callee) in &calls {
            println!(
                "    {} = @{} → {}",
                result.yellow(),
                callee.cyan(),
                ret_ty.dimmed()
            );
        }
        println!();
    }

    // PHI nodes
    if !phis.is_empty() {
        println!("  {} ({}):", "PHIs".bold(), phis.len());
        for (result, ty, entries) in &phis {
            let entry_count = entries.matches('[').count();
            let has_zeroinit = entries.contains("zeroinitializer");
            let warn = if has_zeroinit { " ⚠ zeroinit".red().to_string() } else { String::new() };
            println!(
                "    {} {} ({} entries){}",
                result.yellow(),
                ty.dimmed(),
                entry_count,
                warn
            );
        }
        println!();
    }

    // GEPs (verbose only)
    if verbose && !geps.is_empty() {
        println!("  {} ({}):", "GEPs".bold(), geps.len());
        for (result, base, indices) in &geps {
            println!(
                "    {} ← {}.{}",
                result.yellow(),
                base,
                indices.trim().dimmed()
            );
        }
        println!();
    }

    // Summary
    let total_bytes: usize = allocas.iter().map(|(_, ty, _)| estimate_size(ty)).sum();
    println!(
        "  {} {} allocas ({} bytes), {} calls, {} PHIs, {} stores, {} loads",
        "Total:".bold(),
        allocas.len(),
        total_bytes,
        calls.len(),
        phis.len(),
        stores.len(),
        loads.len()
    );

    // Warnings
    let zeroinit_phis: Vec<_> = phis.iter().filter(|(_, _, e)| e.contains("zeroinitializer")).collect();
    if !zeroinit_phis.is_empty() {
        println!();
        println!(
            "  {} {} PHI nodes with zeroinitializer — potential state corruption",
            "⚠".yellow().bold(),
            zeroinit_phis.len()
        );
    }

    0
}

fn estimate_size(ty: &str) -> usize {
    let ty = ty.trim();
    if ty == "i1" { return 1; }
    if ty == "i8" { return 1; }
    if ty == "i16" { return 2; }
    if ty == "i32" || ty == "float" { return 4; }
    if ty == "i64" || ty == "double" || ty == "ptr" { return 8; }
    if ty.starts_with('{') { return (ty.matches(',').count() + 1) * 8; }
    if ty.starts_with('%') { return 8; }
    8
}
