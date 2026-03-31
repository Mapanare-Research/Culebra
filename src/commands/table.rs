use crate::ir;
use colored::Colorize;

pub fn run(file: &str, top: Option<usize>, sort_by: &str) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    let module = ir::parse_ir(&text);
    let mut funcs: Vec<_> = module.functions.values().collect();

    funcs.sort_by(|a, b| {
        let key = |f: &ir::IRFunction| -> usize {
            match sort_by {
                "allocas" => f.metrics.allocas,
                "alloca_bytes" => f.metrics.alloca_bytes,
                "calls" => f.metrics.calls,
                "blocks" => f.metrics.basic_blocks,
                "stores" => f.metrics.stores,
                "loads" => f.metrics.loads,
                "geps" => f.metrics.geps,
                _ => f.metrics.instructions,
            }
        };
        key(b).cmp(&key(a))
    });

    if let Some(n) = top {
        funcs.truncate(n);
    }

    println!(
        "{:<40} {:>6} {:>6} {:>6} {:>8} {:>6} {:>6} {:>6}",
        "Function".bold(),
        "Insns".bold(),
        "Blocks".bold(),
        "Allocs".bold(),
        "AlBytes".bold(),
        "Calls".bold(),
        "GEPs".bold(),
        "Stores".bold(),
    );
    println!("{}", "-".repeat(86));

    for f in &funcs {
        let m = &f.metrics;
        println!(
            "{:<40} {:>6} {:>6} {:>6} {:>8} {:>6} {:>6} {:>6}",
            truncate_name(&f.name, 40),
            m.instructions,
            m.basic_blocks,
            m.allocas,
            m.alloca_bytes,
            m.calls,
            m.geps,
            m.stores,
        );
    }

    println!(
        "\n{} functions, {} total instructions",
        module.functions.len(),
        module.functions.values().map(|f| f.metrics.instructions).sum::<usize>()
    );
    0
}

fn truncate_name(name: &str, max: usize) -> String {
    if name.len() <= max {
        name.to_string()
    } else {
        format!("{}...", &name[..max - 3])
    }
}
