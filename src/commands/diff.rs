use crate::ir;
use colored::Colorize;
use std::collections::HashSet;

pub fn run(file_a: &str, file_b: &str, verbose: bool) -> i32 {
    let text_a = match std::fs::read_to_string(file_a) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file_a}: {e}");
            return 1;
        }
    };
    let text_b = match std::fs::read_to_string(file_b) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file_b}: {e}");
            return 1;
        }
    };

    let mod_a = ir::parse_ir(&text_a);
    let mod_b = ir::parse_ir(&text_b);

    let names_a: HashSet<_> = mod_a.functions.keys().cloned().collect();
    let names_b: HashSet<_> = mod_b.functions.keys().cloned().collect();
    let all_names: Vec<_> = {
        let mut all: Vec<_> = names_a.union(&names_b).cloned().collect();
        all.sort();
        all
    };

    let mut matched = 0;
    let mut diverged = 0;
    let mut only_a = 0;
    let mut only_b = 0;

    println!(
        "{:<40} {:>12} {:>12}  Status",
        "Function", file_a, file_b
    );
    println!("{}", "-".repeat(80));

    for name in &all_names {
        let fa = mod_a.functions.get(name);
        let fb = mod_b.functions.get(name);

        match (fa, fb) {
            (Some(_), None) => {
                only_a += 1;
                println!("{:<40} {:>12} {:>12}  {}", name, "present", "-", "only in A".yellow());
            }
            (None, Some(_)) => {
                only_b += 1;
                println!("{:<40} {:>12} {:>12}  {}", name, "-", "present", "only in B".yellow());
            }
            (Some(a), Some(b)) => {
                if a.body_hash == b.body_hash {
                    matched += 1;
                } else {
                    diverged += 1;
                    println!(
                        "{:<40} {:>12} {:>12}  {}",
                        name,
                        format!("{}i", a.metrics.instructions),
                        format!("{}i", b.metrics.instructions),
                        "DIVERGED".red().bold()
                    );
                    if verbose {
                        let diffs = [
                            ("instructions", a.metrics.instructions, b.metrics.instructions),
                            ("basic_blocks", a.metrics.basic_blocks, b.metrics.basic_blocks),
                            ("allocas", a.metrics.allocas, b.metrics.allocas),
                            ("calls", a.metrics.calls, b.metrics.calls),
                            ("stores", a.metrics.stores, b.metrics.stores),
                            ("loads", a.metrics.loads, b.metrics.loads),
                        ];
                        for (metric, va, vb) in diffs {
                            if va != vb {
                                println!("    {metric}: {va} -> {vb}");
                            }
                        }
                    }
                }
            }
            (None, None) => unreachable!(),
        }
    }

    println!("\n--- Summary ---");
    println!("  Matched:  {matched}");
    println!("  Diverged: {diverged}");
    println!("  Only A:   {only_a}");
    println!("  Only B:   {only_b}");
    println!("  Total:    {}", all_names.len());

    if diverged > 0 || only_a > 0 || only_b > 0 { 1 } else { 0 }
}
