use colored::Colorize;
use regex::Regex;
use std::collections::HashMap;

use crate::ir;

pub fn run(file: &str, function: &str, block: Option<&str>) -> i32 {
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
        "{} Inspect: {} ({} blocks)",
        "culebra".green().bold(),
        func.name.cyan().bold(),
        func.metrics.basic_blocks
    );
    println!();

    // Parse blocks
    let blocks = parse_blocks(&func.body);

    if let Some(target_block) = block {
        // Show one block in detail
        if let Some(blk) = blocks.iter().find(|b| b.label == target_block || b.label.contains(target_block)) {
            print_block_detail(blk, &blocks);
        } else {
            eprintln!("{}: Block '{}' not found", "error".red().bold(), target_block);
            let labels: Vec<_> = blocks.iter().map(|b| b.label.as_str()).collect();
            eprintln!("  Available: {}", labels.join(", "));
            return 1;
        }
    } else {
        // Show all blocks with flow
        print_block_flow(&blocks);
    }

    0
}

#[derive(Debug)]
struct Block {
    label: String,
    instructions: Vec<String>,
    terminator: Option<String>,
    successors: Vec<String>,
    predecessors: Vec<String>,
    allocas: usize,
    stores: usize,
    loads: usize,
    calls: usize,
}

fn parse_blocks(body: &str) -> Vec<Block> {
    let label_re = Regex::new(r"^([a-zA-Z_][\w.]*):").unwrap();
    let br_label_re = Regex::new(r"br label %(\w+)").unwrap();
    let br_cond_re = Regex::new(r"br i1 .+, label %(\w+), label %(\w+)").unwrap();
    let switch_re = Regex::new(r"label %(\w+)").unwrap();

    let mut blocks: Vec<Block> = Vec::new();
    let mut current_label = "entry".to_string();
    let mut current_instrs: Vec<String> = Vec::new();
    let mut current_term: Option<String> = None;

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        if let Some(caps) = label_re.captures(trimmed) {
            // Save previous block
            if !current_instrs.is_empty() || current_term.is_some() {
                let successors = extract_successors(current_term.as_deref().unwrap_or(""));
                blocks.push(make_block(current_label.clone(), current_instrs.clone(), current_term.clone(), successors));
            }
            current_label = caps[1].to_string();
            current_instrs.clear();
            current_term = None;
        } else if trimmed.starts_with("br ") || trimmed.starts_with("ret ") ||
                  trimmed.starts_with("switch ") || trimmed.starts_with("unreachable") {
            current_term = Some(trimmed.to_string());
        } else {
            current_instrs.push(trimmed.to_string());
        }
    }

    // Last block
    if !current_instrs.is_empty() || current_term.is_some() {
        let successors = extract_successors(current_term.as_deref().unwrap_or(""));
        blocks.push(make_block(current_label, current_instrs, current_term, successors));
    }

    // Compute predecessors (two-pass to avoid borrow conflict)
    let mut pred_map: HashMap<String, Vec<String>> = HashMap::new();
    for blk in &blocks {
        for succ in &blk.successors {
            pred_map.entry(succ.clone()).or_default().push(blk.label.clone());
        }
    }
    for blk in &mut blocks {
        if let Some(preds) = pred_map.remove(&blk.label) {
            blk.predecessors = preds;
        }
    }

    blocks
}

fn make_block(label: String, instrs: Vec<String>, term: Option<String>, successors: Vec<String>) -> Block {
    let allocas = instrs.iter().filter(|i| i.contains("= alloca ")).count();
    let stores = instrs.iter().filter(|i| i.starts_with("store ")).count();
    let loads = instrs.iter().filter(|i| i.contains("= load ")).count();
    let calls = instrs.iter().filter(|i| i.contains("call ")).count();

    Block {
        label,
        instructions: instrs,
        terminator: term,
        successors,
        predecessors: Vec::new(),
        allocas, stores, loads, calls,
    }
}

fn extract_successors(term: &str) -> Vec<String> {
    let mut succs = Vec::new();
    let br_label_re = Regex::new(r"br label %(\w+)").unwrap();
    let br_cond_re = Regex::new(r"br i1 .+, label %(\w+), label %(\w+)").unwrap();
    let label_re = Regex::new(r"label %(\w+)").unwrap();

    if let Some(caps) = br_cond_re.captures(term) {
        succs.push(caps[1].to_string());
        succs.push(caps[2].to_string());
    } else if let Some(caps) = br_label_re.captures(term) {
        succs.push(caps[1].to_string());
    } else if term.starts_with("switch") {
        for caps in label_re.captures_iter(term) {
            let label = caps[1].to_string();
            if !succs.contains(&label) {
                succs.push(label);
            }
        }
    }
    succs
}

fn print_block_flow(blocks: &[Block]) {
    for (i, blk) in blocks.iter().enumerate() {
        let term_kind = if let Some(ref t) = blk.terminator {
            if t.starts_with("ret ") { "ret".red().bold().to_string() }
            else if t.contains("br i1") { "cond".yellow().to_string() }
            else if t.starts_with("br label") { "jump".dimmed().to_string() }
            else if t.starts_with("switch") { "switch".cyan().to_string() }
            else { "?".to_string() }
        } else {
            "none".red().to_string()
        };

        let preds = if blk.predecessors.is_empty() {
            "(entry)".dimmed().to_string()
        } else {
            blk.predecessors.join(", ").dimmed().to_string()
        };

        let succs = if blk.successors.is_empty() {
            "(exit)".dimmed().to_string()
        } else {
            blk.successors.iter().map(|s| s.cyan().to_string()).collect::<Vec<_>>().join(", ")
        };

        println!(
            "  {:>3}. {} ({} insns, {}a {}s {}l {}c) [{}]",
            i + 1,
            blk.label.cyan().bold(),
            blk.instructions.len(),
            blk.allocas, blk.stores, blk.loads, blk.calls,
            term_kind
        );
        println!(
            "       {} ← {}  → {}",
            "from".dimmed(),
            preds,
            succs
        );
    }
}

fn print_block_detail(blk: &Block, all_blocks: &[Block]) {
    println!("  {} ({} instructions):", blk.label.cyan().bold(), blk.instructions.len());
    println!("    predecessors: {}",
        if blk.predecessors.is_empty() { "(entry)".to_string() }
        else { blk.predecessors.join(", ") }
    );
    println!("    successors:   {}",
        if blk.successors.is_empty() { "(exit)".to_string() }
        else { blk.successors.join(", ") }
    );
    println!();

    for instr in &blk.instructions {
        let colored = if instr.contains("= alloca ") {
            instr.green().to_string()
        } else if instr.starts_with("store ") {
            instr.blue().to_string()
        } else if instr.contains("call ") {
            instr.yellow().to_string()
        } else if instr.contains("= phi ") {
            instr.purple().to_string()
        } else {
            instr.dimmed().to_string()
        };
        println!("    {}", colored);
    }

    if let Some(ref term) = blk.terminator {
        println!("    {}", term.red().bold());
    }
}
