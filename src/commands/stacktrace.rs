use colored::Colorize;
use regex::Regex;

use crate::ir;

pub fn run(crash_input: &str, ir_file: Option<&str>) -> i32 {
    // crash_input can be a file path or "-" for stdin
    let crash_text = if crash_input == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).unwrap_or_default();
        buf
    } else {
        match std::fs::read_to_string(crash_input) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}: Failed to read {}: {}", "error".red().bold(), crash_input, e);
                return 1;
            }
        }
    };

    // Parse crash frames
    let frames = parse_crash_output(&crash_text);

    if frames.is_empty() {
        eprintln!("{}: No crash frames found in input", "error".red().bold());
        eprintln!("  Supported formats: valgrind, ASAN, gdb backtrace, SIGSEGV");
        return 1;
    }

    println!(
        "{} Parsed {} crash frames",
        "culebra".green().bold(),
        frames.len()
    );

    // Extract key info
    let crash_addr = extract_crash_address(&crash_text);
    let signal = extract_signal(&crash_text);

    if let Some(sig) = &signal {
        println!("  Signal: {}", sig.red().bold());
    }
    if let Some(addr) = &crash_addr {
        println!("  Address: {}", addr.yellow());
    }
    println!();

    // Print stack trace with coloring
    println!("  {}", "Stack trace:".bold());
    for (i, frame) in frames.iter().enumerate() {
        let func_colored = if frame.function.starts_with("__mn_") || frame.function.starts_with("mn_") {
            frame.function.cyan().to_string()
        } else if frame.function.contains("lower") || frame.function.contains("emit") ||
                  frame.function.contains("parse") || frame.function.contains("tokenize") ||
                  frame.function.contains("check") || frame.function.contains("scan") {
            frame.function.yellow().bold().to_string()
        } else {
            frame.function.dimmed().to_string()
        };

        let addr_str = frame.address.as_deref().unwrap_or("???");
        println!(
            "    #{:<3} {} {}",
            i,
            format!("0x{}", addr_str).dimmed(),
            func_colored
        );
    }

    // If IR file provided, cross-reference
    if let Some(ir_path) = ir_file {
        let ir_content = match std::fs::read_to_string(ir_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("\n{}: Failed to read IR {}: {}", "error".red().bold(), ir_path, e);
                return 1;
            }
        };

        let module = ir::parse_ir(&ir_content);

        println!();
        println!("  {} (from {}):", "IR cross-reference".bold(), ir_path);

        for frame in &frames {
            if let Some(func) = module.functions.get(&frame.function) {
                let m = &func.metrics;
                println!(
                    "    {} — {}i, {}bb, {}a, {}c (L{}-L{})",
                    frame.function.cyan(),
                    m.instructions,
                    m.basic_blocks,
                    m.allocas,
                    m.calls,
                    func.line_start,
                    func.line_end
                );
            }
        }

        // Map crash address to struct if possible
        if let Some(ref addr) = crash_addr {
            if let Ok(offset) = usize::from_str_radix(addr.trim_start_matches("0x"), 16) {
                if offset < 256 {
                    println!();
                    println!(
                        "  {} Small offset (0x{:x}) — likely null struct dereference",
                        "hint:".cyan().bold(),
                        offset
                    );
                    println!(
                        "    Run: culebra crashmap {} --offset 0x{:x} --struct <StructName>",
                        ir_path, offset
                    );
                }
            }
        }
    }

    0
}

struct CrashFrame {
    function: String,
    address: Option<String>,
}

fn parse_crash_output(text: &str) -> Vec<CrashFrame> {
    let mut frames = Vec::new();

    // Valgrind format: ==PID==    at 0xADDR: function_name (in binary)
    let valgrind_re = Regex::new(r"==\d+==\s+(?:at|by)\s+0x([0-9A-Fa-f]+):\s+(\w+)").unwrap();
    // GDB format: #N  0xADDR in function_name
    let gdb_re = Regex::new(r"#\d+\s+0x([0-9A-Fa-f]+)\s+in\s+(\w+)").unwrap();
    // ASAN format: #N 0xADDR in function_name
    let asan_re = Regex::new(r"#\d+\s+0x([0-9A-Fa-f]+)\s+in\s+(\w+)").unwrap();
    // Simple: function_name (in binary) or function_name+0xOFFSET
    let simple_re = Regex::new(r"(\w+)\s*\+\s*0x([0-9A-Fa-f]+)").unwrap();

    for line in text.lines() {
        if let Some(caps) = valgrind_re.captures(line) {
            frames.push(CrashFrame {
                address: Some(caps[1].to_string()),
                function: caps[2].to_string(),
            });
        } else if let Some(caps) = gdb_re.captures(line) {
            frames.push(CrashFrame {
                address: Some(caps[1].to_string()),
                function: caps[2].to_string(),
            });
        } else if let Some(caps) = asan_re.captures(line) {
            frames.push(CrashFrame {
                address: Some(caps[1].to_string()),
                function: caps[2].to_string(),
            });
        } else if let Some(caps) = simple_re.captures(line) {
            frames.push(CrashFrame {
                address: Some(caps[2].to_string()),
                function: caps[1].to_string(),
            });
        }
    }

    // Deduplicate consecutive frames
    frames.dedup_by(|a, b| a.function == b.function);
    frames
}

fn extract_crash_address(text: &str) -> Option<String> {
    // "Address 0xNNN is not stack'd" or "Invalid read at address 0xNNN"
    let re = Regex::new(r"[Aa]ddress\s+(0x[0-9A-Fa-f]+)").unwrap();
    re.captures(text).map(|c| c[1].to_string())
}

fn extract_signal(text: &str) -> Option<String> {
    if text.contains("SIGSEGV") { return Some("SIGSEGV (segmentation fault)".to_string()); }
    if text.contains("SIGABRT") { return Some("SIGABRT (abort)".to_string()); }
    if text.contains("SIGBUS") { return Some("SIGBUS (bus error)".to_string()); }
    if text.contains("SIGFPE") { return Some("SIGFPE (floating point exception)".to_string()); }
    if text.contains("stack overflow") { return Some("Stack overflow".to_string()); }
    if text.contains("heap-buffer-overflow") { return Some("Heap buffer overflow (ASAN)".to_string()); }
    if text.contains("use-after-free") { return Some("Use after free (ASAN)".to_string()); }
    None
}
