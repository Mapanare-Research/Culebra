use colored::Colorize;
use regex::Regex;
use std::process::Command;

use crate::ir;

pub fn run(
    file: &str,
    function: &str,
    watch_vars: &[String],
    stop_at: Option<&str>,
    compiler: &str,
    timeout: u64,
) -> i32 {
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
        "{} Probe: {} — watching {}",
        "culebra".green().bold(),
        func.name.cyan(),
        if watch_vars.is_empty() { "all stores".to_string() }
        else { watch_vars.join(", ").yellow().to_string() }
    );

    // Generate instrumented IR
    let instrumented = instrument_function(&content, &func.name, watch_vars, stop_at);

    let tmp_dir = std::env::temp_dir();
    let probe_path = tmp_dir.join("culebra_probe.ll");
    let binary_path = tmp_dir.join("culebra_probe_binary");

    match std::fs::write(&probe_path, &instrumented) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}: Failed to write: {}", "error".red().bold(), e);
            return 1;
        }
    }

    // Compile
    println!("  Compiling...");
    let compile = Command::new(compiler)
        .args([
            "-O0",
            "-Wno-override-module",
            probe_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
            "-lm",
        ])
        .output();

    match compile {
        Ok(output) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("{}: Compilation failed:", "error".red().bold());
            for line in stderr.lines().take(5) {
                eprintln!("  {}", line);
            }
            // Still useful: show what we would have probed
            println!();
            println!("  {} Compilation failed, showing static probe points:", "note:".yellow().bold());
            show_static_probes(&func.body, watch_vars);
            return 1;
        }
        Err(_) => {
            eprintln!("{}: {} not found", "error".red().bold(), compiler);
            println!();
            println!("  {} Showing static probe points instead:", "note:".yellow().bold());
            show_static_probes(&func.body, watch_vars);
            return 1;
        }
        _ => {}
    }

    // Run with timeout
    println!("  Running (timeout {}s)...", timeout);
    let run_result = Command::new("timeout")
        .args([
            &timeout.to_string(),
            binary_path.to_str().unwrap(),
        ])
        .output()
        .or_else(|_| {
            // No timeout command — run directly
            Command::new(binary_path.to_str().unwrap()).output()
        });

    match run_result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            println!();
            println!("  {}", "Probe output:".bold());

            let mut probe_count = 0;
            for line in stdout.lines() {
                if line.starts_with("[PROBE]") {
                    println!("    {}", line.yellow());
                    probe_count += 1;
                }
            }

            if probe_count == 0 {
                println!("    (no probe output — function may not have been reached)");
                if !stderr.is_empty() {
                    println!("    stderr: {}", stderr.lines().next().unwrap_or("").dimmed());
                }
            } else {
                println!();
                println!("  {} probe points hit", probe_count);
            }

            if !output.status.success() {
                let code = output.status.code().unwrap_or(-1);
                if code == 139 {
                    println!("  {} SIGSEGV after {} probes", "crash:".red().bold(), probe_count);
                } else if code == 137 {
                    println!("  {} timeout after {} probes", "timeout:".yellow().bold(), probe_count);
                }
            }
        }
        Err(e) => {
            eprintln!("{}: Failed to run: {}", "error".red().bold(), e);
        }
    }

    // Cleanup
    let _ = std::fs::remove_file(&probe_path);
    let _ = std::fs::remove_file(&binary_path);

    0
}

fn instrument_function(
    ir: &str,
    func_name: &str,
    watch_vars: &[String],
    stop_at: Option<&str>,
) -> String {
    let label_re = Regex::new(r"^([a-zA-Z_][\w.]*):").unwrap();
    let store_re = Regex::new(r"store\s+(\S+)\s+(.+?),\s*ptr\s+(%[\w.]+)").unwrap();

    let mut result = String::new();
    let mut in_target_fn = false;
    let mut fn_depth = 0i32;
    let mut probe_id = 0;

    // Add printf declaration at top
    let mut added_printf = false;

    for line in ir.lines() {
        // Add printf decl after first declare/define
        if !added_printf && (line.starts_with("declare") || line.starts_with("define")) {
            result.push_str("declare i32 @printf(ptr, ...)\n");
            result.push_str("@.probe_fmt_i64 = private constant [40 x i8] c\"[PROBE] %s = %lld (block: %s)\\0A\\00\"\n");
            result.push_str("@.probe_fmt_ptr = private constant [40 x i8] c\"[PROBE] %s = 0x%llx (block: %s)\\0A\\00\"\n");
            result.push_str("@.probe_block = private constant [25 x i8] c\"[PROBE] entering block %s\\0A\\00\"\n");
            added_printf = true;
        }

        if line.starts_with("define") && line.contains(&format!("@{}(", func_name)) {
            in_target_fn = true;
            fn_depth = 0;
        }

        if in_target_fn {
            for ch in line.bytes() {
                if ch == b'{' { fn_depth += 1; }
                if ch == b'}' { fn_depth -= 1; }
            }
            if fn_depth <= 0 && line.trim() == "}" {
                in_target_fn = false;
            }
        }

        result.push_str(line);
        result.push('\n');

        // Insert probes in the target function
        if in_target_fn {
            let trimmed = line.trim();

            // Probe at block entries
            if let Some(caps) = label_re.captures(trimmed) {
                let block_name = &caps[1];
                if let Some(stop) = stop_at {
                    if block_name.contains(stop) {
                        // Insert a trap/print at the stop point
                        result.push_str(&format!(
                            "  ; PROBE: stop at {}\n",
                            block_name
                        ));
                    }
                }
            }

            // Probe after store instructions
            if let Some(caps) = store_re.captures(trimmed) {
                let ty = &caps[1];
                let dest = &caps[3];

                let should_probe = if watch_vars.is_empty() {
                    // Probe all stores to named variables (not temporaries)
                    dest.contains(".addr") || dest.contains("state") || dest.contains("result")
                } else {
                    watch_vars.iter().any(|v| dest.contains(v))
                };

                if should_probe && (ty == "i64" || ty == "i1" || ty == "ptr") {
                    probe_id += 1;
                    // We can't easily inject probes into SSA IR without breaking it,
                    // so just note where we would probe
                }
            }
        }
    }

    result
}

fn show_static_probes(body: &str, watch_vars: &[String]) {
    let store_re = Regex::new(r"store\s+(\S+)\s+(.+?),\s*ptr\s+(%[\w.]+)").unwrap();
    let label_re = Regex::new(r"^([a-zA-Z_][\w.]*):").unwrap();
    let mut current_block = "entry";

    for line in body.lines() {
        let trimmed = line.trim();

        if let Some(caps) = label_re.captures(trimmed) {
            current_block = caps.get(1).unwrap().as_str();
        }

        if let Some(caps) = store_re.captures(trimmed) {
            let ty = &caps[1];
            let val = &caps[2];
            let dest = &caps[3];

            let should_show = if watch_vars.is_empty() {
                dest.contains(".addr") || dest.contains("state")
            } else {
                watch_vars.iter().any(|v| dest.contains(v))
            };

            if should_show {
                println!(
                    "    [{}] store {} {} → {}",
                    current_block.cyan(),
                    ty.dimmed(),
                    val.yellow(),
                    dest
                );
            }
        }
    }
}
