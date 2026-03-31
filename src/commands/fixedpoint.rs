use colored::Colorize;
use sha2::{Digest, Sha256};
use std::process::Command;
use std::time::Instant;

pub fn run(
    compiler: &str,
    source: &str,
    max_iters: usize,
    timeout: u64,
    runtime: Option<&str>,
) -> i32 {
    println!("{}\n", "=== Fixed-Point Detection ===".bold());
    println!("Compiler: {compiler}");
    println!("Source:   {source}");
    println!("Max iterations: {max_iters}\n");

    let mut current_compiler = compiler.to_string();
    let mut prev_hash: Option<String> = None;
    let mut prev_ir: Option<String> = None;

    for iteration in 1..=max_iters {
        println!("{}", format!("--- Iteration {iteration} ---").bold());

        // Step 1: Compile source with current compiler
        print!("  Compiling {source} with {current_compiler}... ");
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let start = Instant::now();
        let result = Command::new(&current_compiler)
            .arg(source)
            .output();

        let ir_text = match result {
            Ok(output) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let first = stderr.lines().next().unwrap_or("?");
                    println!("{} ({})", "FAIL".red().bold(), first);
                    return 1;
                }
                let ir = String::from_utf8_lossy(&output.stdout).to_string();
                let elapsed = start.elapsed();
                let lines = ir.lines().count();
                println!("{} ({} lines, {:.1}s)", "OK".green().bold(), lines, elapsed.as_secs_f64());
                ir
            }
            Err(e) => {
                println!("{} ({})", "FAIL".red().bold(), e);
                return 1;
            }
        };

        // Step 2: Hash the IR
        let mut hasher = Sha256::new();
        // Normalize: strip comments, whitespace-only differences
        for line in ir_text.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with(';') {
                hasher.update(trimmed.as_bytes());
                hasher.update(b"\n");
            }
        }
        let hash = format!("{:x}", hasher.finalize());
        let short_hash = &hash[..16];

        println!("  IR hash: {short_hash}");

        // Step 3: Compare with previous iteration
        if let Some(ref prev) = prev_hash {
            if &hash == prev {
                println!(
                    "\n  {} Fixed-point reached at iteration {iteration}!",
                    "SUCCESS".green().bold()
                );
                println!("  Stage {iteration} output == Stage {} output", iteration - 1);
                println!("  The compiler is self-hosting.");
                return 0;
            } else {
                // Show what changed
                let module_curr = crate::ir::parse_ir(&ir_text);
                let n_funcs = module_curr.functions.len();
                let n_strings = module_curr.string_constants.len();
                println!("  Delta: hash differs ({} functions, {} strings)", n_funcs, n_strings);

                if let Some(ref prev_text) = prev_ir {
                    let module_prev = crate::ir::parse_ir(prev_text);
                    let prev_funcs = module_prev.functions.len();
                    if n_funcs != prev_funcs {
                        println!(
                            "    Functions: {} -> {}",
                            prev_funcs, n_funcs
                        );
                    }

                    // Count matching function hashes
                    let mut matched = 0;
                    let mut diverged = 0;
                    for (name, func) in &module_curr.functions {
                        if let Some(prev_func) = module_prev.functions.get(name) {
                            if func.body_hash == prev_func.body_hash {
                                matched += 1;
                            } else {
                                diverged += 1;
                            }
                        }
                    }
                    let only_new = module_curr.functions.len().saturating_sub(matched + diverged);
                    println!(
                        "    Matched: {matched}, Diverged: {diverged}, New: {only_new}"
                    );

                    if diverged > 0 && diverged <= 10 {
                        println!("    Diverged functions:");
                        for (name, func) in &module_curr.functions {
                            if let Some(prev_func) = module_prev.functions.get(name) {
                                if func.body_hash != prev_func.body_hash {
                                    println!(
                                        "      {} ({}i -> {}i)",
                                        name,
                                        prev_func.metrics.instructions,
                                        func.metrics.instructions
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Step 4: Build next-stage compiler from this IR
        if iteration < max_iters {
            print!("  Building stage{} binary... ", iteration + 1);
            std::io::Write::flush(&mut std::io::stdout()).ok();

            let ll_path = std::env::temp_dir().join(format!("culebra_fp_stage{iteration}.ll"));
            let bin_path = std::env::temp_dir().join(format!("culebra_fp_stage{}", iteration + 1));
            let _ = std::fs::write(&ll_path, &ir_text);

            // Validate IR first
            let (valid, err) = crate::ir::validate_with_llvm_as(&ir_text);
            if !valid {
                println!("{} (IR invalid: {})", "FAIL".red().bold(), err);
                let _ = std::fs::remove_file(&ll_path);
                return 1;
            }

            let mut clang = Command::new("clang");
            clang
                .arg("-O0")
                .arg("-Wno-override-module")
                .arg(ll_path.to_str().unwrap());
            if let Some(rt) = runtime {
                clang.arg(rt);
            }
            clang.arg("-o").arg(bin_path.to_str().unwrap()).arg("-lm");

            match clang.output() {
                Ok(output) if output.status.success() => {
                    let size = std::fs::metadata(&bin_path).map(|m| m.len()).unwrap_or(0);
                    println!("{} ({} bytes)", "OK".green().bold(), size);
                    current_compiler = bin_path.to_str().unwrap().to_string();
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let first = stderr.lines().next().unwrap_or("?");
                    println!("{} ({})", "FAIL".red().bold(), first);
                    let _ = std::fs::remove_file(&ll_path);
                    return 1;
                }
                Err(e) => {
                    println!("{} ({})", "FAIL".red().bold(), e);
                    let _ = std::fs::remove_file(&ll_path);
                    return 1;
                }
            }

            let _ = std::fs::remove_file(&ll_path);
        }

        prev_hash = Some(hash);
        prev_ir = Some(ir_text);
        println!();
    }

    println!(
        "{} No fixed-point after {max_iters} iterations.",
        "NOT CONVERGED".yellow().bold()
    );
    if let Some(ref hash) = prev_hash {
        println!("  Last hash: {}", &hash[..16]);
    }
    1
}
