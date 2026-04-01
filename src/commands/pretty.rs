use colored::Colorize;
use regex::Regex;

use crate::ir;

pub fn run(file: &str, function: Option<&str>, no_color: bool) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    if let Some(func_name) = function {
        // Pretty-print one function
        let func = module.functions.values().find(|f| {
            f.name == func_name || f.name.contains(func_name)
        });
        match func {
            Some(f) => {
                print_function_header(f);
                print_function_body(&f.body, no_color);
            }
            None => {
                eprintln!("{}: Function '{}' not found", "error".red().bold(), func_name);
                let close: Vec<_> = module.functions.keys()
                    .filter(|k| k.contains(&func_name[..func_name.len().min(4).max(1)]))
                    .take(10).collect();
                if !close.is_empty() {
                    eprintln!("  Similar: {}", close.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                }
                return 1;
            }
        }
    } else {
        // Pretty-print module overview
        print_module_overview(&module, &content, no_color);
    }

    0
}

fn print_function_header(func: &ir::IRFunction) {
    let m = &func.metrics;
    println!(
        "{} {} (L{}-L{}, {}i, {}bb, {}a, {}c)",
        "fn".cyan().bold(),
        func.name.yellow().bold(),
        func.line_start,
        func.line_end,
        m.instructions,
        m.basic_blocks,
        m.allocas,
        m.calls
    );

    // Parse return type and params from signature
    println!("  {}", func.signature.dimmed());
    println!();
}

fn print_function_body(body: &str, no_color: bool) {
    let label_re = Regex::new(r"^([a-zA-Z_][\w.]*):").unwrap();
    let alloca_re = Regex::new(r"(%[\w.]+)\s*=\s*alloca\s+(.+?)(?:\s*,|$)").unwrap();
    let call_re = Regex::new(r"call\s+\S+\s+@([\w.]+)\(").unwrap();
    let phi_re = Regex::new(r"= phi\s+").unwrap();
    let br_re = Regex::new(r"^\s*br\s+").unwrap();
    let ret_re = Regex::new(r"^\s*ret\s+").unwrap();
    let switch_re = Regex::new(r"^\s*switch\s+").unwrap();
    let store_re = Regex::new(r"^\s*store\s+").unwrap();
    let load_re = Regex::new(r"=\s*load\s+").unwrap();
    let gep_re = Regex::new(r"=\s*getelementptr\s+").unwrap();

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            println!();
            continue;
        }

        if no_color {
            println!("  {}", trimmed);
            continue;
        }

        // Labels
        if let Some(caps) = label_re.captures(trimmed) {
            println!("{}:", caps[1].cyan().bold());
            continue;
        }

        // Classify and colorize
        if alloca_re.is_match(trimmed) {
            // Allocas in green
            println!("  {}", colorize_ir(trimmed, "green"));
        } else if phi_re.is_match(trimmed) {
            // PHI nodes in magenta
            println!("  {}", colorize_ir(trimmed, "magenta"));
        } else if ret_re.is_match(trimmed) || br_re.is_match(trimmed) || switch_re.is_match(trimmed) {
            // Terminators in red
            println!("  {}", colorize_ir(trimmed, "red"));
        } else if call_re.is_match(trimmed) {
            // Calls in yellow
            println!("  {}", colorize_ir(trimmed, "yellow"));
        } else if store_re.is_match(trimmed) {
            // Stores in blue
            println!("  {}", colorize_ir(trimmed, "blue"));
        } else if load_re.is_match(trimmed) || gep_re.is_match(trimmed) {
            // Loads/GEPs dim
            println!("  {}", trimmed.dimmed());
        } else {
            println!("  {}", trimmed);
        }
    }
}

fn colorize_ir(line: &str, base_color: &str) -> String {
    // Highlight %variables and @functions within the line
    let var_re = Regex::new(r"(%[\w.]+)").unwrap();
    let func_re = Regex::new(r"(@[\w.]+)").unwrap();
    let type_re = Regex::new(r"\b(i1|i8|i16|i32|i64|float|double|ptr|void)\b").unwrap();

    let mut result = line.to_string();

    // Apply base color to the whole line, then overlay
    match base_color {
        "green" => result.green().to_string(),
        "magenta" => result.purple().to_string(),
        "red" => result.red().to_string(),
        "yellow" => result.yellow().to_string(),
        "blue" => result.blue().to_string(),
        _ => result,
    }
}

fn print_module_overview(module: &ir::IRModule, content: &str, _no_color: bool) {
    let total_insns: usize = module.functions.values().map(|f| f.metrics.instructions).sum();
    let total_lines = content.lines().count();

    println!("{} Module overview", "culebra".green().bold());
    println!();

    // Stats
    println!("  {}", "Stats:".bold());
    println!("    Lines:        {}", total_lines);
    println!("    Functions:    {}", module.functions.len());
    println!("    Instructions: {}", total_insns);
    println!("    Declarations: {}", module.declares.len());
    println!("    Globals:      {}", module.globals.len());
    println!("    Struct types: {}", module.struct_types.len());
    println!("    Strings:      {}", module.string_constants.len());
    println!();

    // Type definitions
    if !module.struct_types.is_empty() {
        println!("  {}", "Types:".bold());
        for st in &module.struct_types {
            println!("    {} = {{ {} fields }}", st.name.cyan(), st.fields.len());
        }
        println!();
    }

    // Top functions by size
    let mut funcs: Vec<_> = module.functions.values().collect();
    funcs.sort_by(|a, b| b.metrics.instructions.cmp(&a.metrics.instructions));

    println!("  {} (top 15):", "Functions:".bold());
    for f in funcs.iter().take(15) {
        let bar_len = (f.metrics.instructions as f64 / funcs[0].metrics.instructions as f64 * 30.0) as usize;
        let bar = "█".repeat(bar_len.max(1));
        println!(
            "    {:>5}i  {}  {}",
            f.metrics.instructions,
            bar.green(),
            f.name
        );
    }
    if funcs.len() > 15 {
        println!("    ... and {} more", funcs.len() - 15);
    }
}
