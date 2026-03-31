use crate::ir;
use colored::Colorize;
use std::collections::HashSet;

pub fn run(file_a: &str, file_b: &str, top_n: usize) -> i32 {
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

    // Build call graph for impact ranking
    let callers_of = build_caller_map(&text_a);

    let names_a: HashSet<_> = mod_a.functions.keys().cloned().collect();
    let names_b: HashSet<_> = mod_b.functions.keys().cloned().collect();

    #[derive(Debug)]
    struct Divergence {
        name: String,
        kind: DivKind,
        size_a: usize,
        size_b: usize,
        impact: usize, // number of callers
        delta_pct: f64,
    }

    #[derive(Debug)]
    enum DivKind {
        Missing,    // in A not in B
        Added,      // in B not in A
        Changed,    // in both, different hash
    }

    let mut divergences: Vec<Divergence> = Vec::new();

    // Functions only in A (missing from B)
    for name in names_a.difference(&names_b) {
        let fa = &mod_a.functions[name];
        let callers = callers_of.get(name.as_str()).map(|c| c.len()).unwrap_or(0);
        divergences.push(Divergence {
            name: name.clone(),
            kind: DivKind::Missing,
            size_a: fa.metrics.instructions,
            size_b: 0,
            impact: callers,
            delta_pct: 1.0,
        });
    }

    // Functions only in B (added)
    for name in names_b.difference(&names_a) {
        let fb = &mod_b.functions[name];
        divergences.push(Divergence {
            name: name.clone(),
            kind: DivKind::Added,
            size_a: 0,
            size_b: fb.metrics.instructions,
            impact: 0,
            delta_pct: 1.0,
        });
    }

    // Functions in both but different
    for name in names_a.intersection(&names_b) {
        let fa = &mod_a.functions[name];
        let fb = &mod_b.functions[name];
        if fa.body_hash != fb.body_hash {
            let callers = callers_of.get(name.as_str()).map(|c| c.len()).unwrap_or(0);
            let delta = if fa.metrics.instructions > 0 {
                (fa.metrics.instructions as f64 - fb.metrics.instructions as f64).abs()
                    / fa.metrics.instructions as f64
            } else {
                1.0
            };
            divergences.push(Divergence {
                name: name.clone(),
                kind: DivKind::Changed,
                size_a: fa.metrics.instructions,
                size_b: fb.metrics.instructions,
                impact: callers,
                delta_pct: delta,
            });
        }
    }

    if divergences.is_empty() {
        println!(
            "{} Fixed point — no divergences between stages!",
            "culebra".green().bold()
        );
        return 0;
    }

    // Rank by: impact * delta_pct (callers * how much it changed)
    divergences.sort_by(|a, b| {
        let score_a = (a.impact + 1) as f64 * a.delta_pct * a.size_a.max(a.size_b) as f64;
        let score_b = (b.impact + 1) as f64 * b.delta_pct * b.size_a.max(b.size_b) as f64;
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    let total = divergences.len();
    let show = divergences.iter().take(top_n);

    println!(
        "{} Bisect: {} divergent functions (showing top {})",
        "culebra".green().bold(),
        total,
        top_n.min(total)
    );
    println!();
    println!(
        "  {:<4} {:<40} {:>8} {:>8} {:>8} {:>8}",
        "#", "Function", "A insns", "B insns", "Callers", "Kind"
    );
    println!("  {}", "-".repeat(80));

    for (i, d) in show.enumerate() {
        let kind_str = match d.kind {
            DivKind::Missing => "MISSING".red().bold().to_string(),
            DivKind::Added => "added".cyan().to_string(),
            DivKind::Changed => format!("{:+.0}%",
                if d.size_a > 0 { ((d.size_b as f64 / d.size_a as f64) - 1.0) * 100.0 } else { 100.0 }
            ),
        };

        println!(
            "  {:<4} {:<40} {:>8} {:>8} {:>8} {:>8}",
            i + 1,
            d.name,
            d.size_a,
            d.size_b,
            d.impact,
            kind_str
        );
    }

    // Summary
    let missing = divergences.iter().filter(|d| matches!(d.kind, DivKind::Missing)).count();
    let added = divergences.iter().filter(|d| matches!(d.kind, DivKind::Added)).count();
    let changed = divergences.iter().filter(|d| matches!(d.kind, DivKind::Changed)).count();

    println!();
    println!("  {} total: {} missing, {} changed, {} added",
        total, missing, changed, added);

    if total > top_n {
        println!("  (run with --top {} to see all)", total);
    }

    1
}

/// Build a map: function_name -> set of callers
fn build_caller_map(ir_text: &str) -> std::collections::HashMap<&str, HashSet<&str>> {
    let mut map: std::collections::HashMap<&str, HashSet<&str>> = std::collections::HashMap::new();
    let call_re = regex::Regex::new(r"call\s+.+@([\w.]+)\(").unwrap();
    let fn_re = regex::Regex::new(r"^define\s+.+@([\w.]+)\s*\(").unwrap();

    let mut current_fn = "";
    for line in ir_text.lines() {
        if let Some(caps) = fn_re.captures(line) {
            current_fn = caps.get(1).unwrap().as_str();
        }
        if !current_fn.is_empty() {
            for caps in call_re.captures_iter(line) {
                let callee = caps.get(1).unwrap().as_str();
                map.entry(callee).or_default().insert(current_fn);
            }
        }
    }
    map
}
