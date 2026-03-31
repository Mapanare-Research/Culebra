use crate::ir;
use colored::Colorize;

pub fn run(file_a: &str, file_b: &str, metric: &str, threshold: f64) -> i32 {
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

    println!(
        "{} Comparing {} vs {} (metric: {}, threshold: {:.0}%)",
        "culebra".green().bold(),
        file_a, file_b, metric, threshold * 100.0
    );
    println!();

    // Header
    println!(
        "  {:<40} {:>8} {:>8} {:>8} {:>8}",
        "Function", "A", "B", "Delta", "Drop%"
    );
    println!("  {}", "-".repeat(76));

    let mut alerts = 0;
    let mut rows: Vec<(String, i64, i64, f64)> = Vec::new();

    // Match functions by name — check both directions
    for (name, fa) in &mod_a.functions {
        if let Some(fb) = mod_b.functions.get(name) {
            let (va, vb) = get_metric(fa, fb, metric);
            if va == 0 {
                continue;
            }
            let drop_pct = if va > 0 {
                (va as f64 - vb as f64) / va as f64
            } else {
                0.0
            };
            rows.push((name.clone(), va, vb, drop_pct));
        } else {
            // Function only in A — 100% drop
            let va = get_single_metric(fa, metric);
            if va > 0 {
                rows.push((name.clone(), va, 0, 1.0));
            }
        }
    }

    // Sort by drop percentage (worst first)
    rows.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));

    for (name, va, vb, drop_pct) in &rows {
        if *drop_pct >= threshold {
            alerts += 1;
            let delta = vb - va;
            let pct_str = format!("{:.0}%", drop_pct * 100.0);
            let flag = if *drop_pct >= 0.5 {
                "ALERT".red().bold().to_string()
            } else {
                "warn".yellow().to_string()
            };
            println!(
                "  {:<40} {:>8} {:>8} {:>8} {:>8} {}",
                name, va, vb, delta, pct_str, flag
            );
        }
    }

    if alerts == 0 {
        println!("  No significant drops found.");
    }

    // Summary
    let total_a: i64 = rows.iter().map(|r| r.1).sum();
    let total_b: i64 = rows.iter().map(|r| r.2).sum();
    let fn_a = mod_a.functions.len();
    let fn_b = mod_b.functions.len();

    println!();
    println!("  --- Summary ---");
    println!("  Functions:   {} -> {} ({})", fn_a, fn_b,
        if fn_b < fn_a { format!("-{} MISSING", fn_a - fn_b).red().to_string() }
        else if fn_b > fn_a { format!("+{} new", fn_b - fn_a) }
        else { "same".to_string() }
    );
    println!("  Total {}:  {} -> {}", metric, total_a, total_b);
    println!("  Alerts:      {} functions with >{:.0}% drop", alerts, threshold * 100.0);

    if alerts > 0 { 1 } else { 0 }
}

fn get_metric(fa: &ir::IRFunction, fb: &ir::IRFunction, metric: &str) -> (i64, i64) {
    match metric {
        "instructions" | "insns" => (fa.metrics.instructions as i64, fb.metrics.instructions as i64),
        "blocks" => (fa.metrics.basic_blocks as i64, fb.metrics.basic_blocks as i64),
        "calls" => (fa.metrics.calls as i64, fb.metrics.calls as i64),
        "allocas" => (fa.metrics.allocas as i64, fb.metrics.allocas as i64),
        "stores" => (fa.metrics.stores as i64, fb.metrics.stores as i64),
        "loads" => (fa.metrics.loads as i64, fb.metrics.loads as i64),
        "pushes" | "list_pushes" => (fa.metrics.list_pushes as i64, fb.metrics.list_pushes as i64),
        "rets" => (fa.metrics.rets as i64, fb.metrics.rets as i64),
        "branches" => (fa.metrics.branches as i64, fb.metrics.branches as i64),
        _ => (fa.metrics.instructions as i64, fb.metrics.instructions as i64),
    }
}

fn get_single_metric(f: &ir::IRFunction, metric: &str) -> i64 {
    match metric {
        "instructions" | "insns" => f.metrics.instructions as i64,
        "blocks" => f.metrics.basic_blocks as i64,
        "calls" => f.metrics.calls as i64,
        "allocas" => f.metrics.allocas as i64,
        "stores" => f.metrics.stores as i64,
        "loads" => f.metrics.loads as i64,
        "pushes" | "list_pushes" => f.metrics.list_pushes as i64,
        "rets" => f.metrics.rets as i64,
        "branches" => f.metrics.branches as i64,
        _ => f.metrics.instructions as i64,
    }
}
