use crate::ir;

pub fn run(file: &str, func_name: &str) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    let module = ir::parse_ir(&text);

    // Exact match first, then substring
    let func = module
        .functions
        .get(func_name)
        .or_else(|| {
            module
                .functions
                .values()
                .find(|f| f.name.contains(func_name))
        });

    match func {
        Some(f) => {
            println!("; Function: {} (L{}-L{})", f.name, f.line_start, f.line_end);
            println!("; Instructions: {}, Blocks: {}, Allocas: {} ({} bytes)",
                f.metrics.instructions, f.metrics.basic_blocks,
                f.metrics.allocas, f.metrics.alloca_bytes);
            println!("{} {{", f.signature);
            println!("{}", f.body);
            println!("}}");
            0
        }
        None => {
            eprintln!("Function '{func_name}' not found.");
            let matches: Vec<_> = module
                .functions
                .keys()
                .filter(|n| n.to_lowercase().contains(&func_name.to_lowercase()))
                .take(10)
                .collect();
            if !matches.is_empty() {
                eprintln!("Similar functions:");
                for name in matches {
                    eprintln!("  {name}");
                }
            }
            1
        }
    }
}
