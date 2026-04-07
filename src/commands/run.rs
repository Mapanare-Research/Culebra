use colored::Colorize;
use std::process::Command;
use std::time::Instant;

pub fn run(
    compiler: &str,
    source: &str,
    expect: Option<&str>,
    timeout: u64,
    clang_flags: Option<&str>,
    runtime: Option<&str>,
    grep_ir: Option<&str>,
) -> i32 {
    println!("{}\n", "=== Compile & Run ===".bold());

    // Step 1: Compile source through the compiler to get IR
    print!("1. Compiling {} through {}... ", source, compiler);
    std::io::Write::flush(&mut std::io::stdout()).ok();

    let start = Instant::now();
    let compile_result = Command::new(compiler)
        .arg(source)
        .output();

    let ir_text = match compile_result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let first = stderr.lines().next().unwrap_or("unknown error");
                println!("{} (exit={})\n   {}", "FAIL".red().bold(), output.status.code().unwrap_or(-1), first);
                return 1;
            }
            let ir = String::from_utf8_lossy(&output.stdout).to_string();
            let lines = ir.lines().count();
            let elapsed = start.elapsed();
            println!("{} ({} lines, {:.1}s)", "OK".green().bold(), lines, elapsed.as_secs_f64());
            ir
        }
        Err(e) => {
            println!("{} ({})", "FAIL".red().bold(), e);
            return 1;
        }
    };

    // Step 1b: Grep IR for pattern if requested
    if let Some(pattern) = grep_ir {
        println!("   Grepping IR for {:?}...", pattern);
        let matched: Vec<_> = ir_text
            .lines()
            .enumerate()
            .filter(|(_, line)| line.contains(pattern))
            .collect();
        if matched.is_empty() {
            println!("   {} no matches for {:?}", "FAIL".red().bold(), pattern);
            return 1;
        } else {
            println!(
                "   {} {} match(es) for {:?}",
                "PASS".green().bold(),
                matched.len(),
                pattern
            );
            for (i, (ln, line)) in matched.iter().enumerate() {
                if i >= 10 {
                    println!("   ... and {} more", matched.len() - 10);
                    break;
                }
                println!("   L{}: {}", ln + 1, line.trim());
            }
        }
        // If no --expect, the user only cares about IR content — skip linking/execution
        if expect.is_none() {
            return 0;
        }
    }

    // Step 2: Validate IR
    print!("2. Validating IR... ");
    std::io::Write::flush(&mut std::io::stdout()).ok();

    let module = crate::ir::parse_ir(&ir_text);
    let (valid, err) = crate::ir::validate_with_llvm_as(&ir_text);
    if valid {
        println!(
            "{} ({} functions, {} string constants)",
            "VALID".green().bold(),
            module.functions.len(),
            module.string_constants.len()
        );
    } else {
        println!("{}\n   {}", "INVALID".red().bold(), err);
        return 1;
    }

    // Quick string constant sanity check
    let mismatches: Vec<_> = module
        .string_constants
        .iter()
        .filter(|c| c.declared_size != c.actual_size)
        .collect();
    if !mismatches.is_empty() {
        println!(
            "   {} {} string byte-count mismatches!",
            "WARNING".yellow().bold(),
            mismatches.len()
        );
        for c in mismatches.iter().take(3) {
            println!(
                "   {} [{} x i8] but content is {} bytes",
                c.name, c.declared_size, c.actual_size
            );
        }
    }

    // Step 3: Compile IR to binary with clang
    print!("3. Linking with clang... ");
    std::io::Write::flush(&mut std::io::stdout()).ok();

    let ll_path = std::env::temp_dir().join("culebra_run.ll");
    let bin_path = std::env::temp_dir().join("culebra_run_bin");
    if std::fs::write(&ll_path, &ir_text).is_err() {
        println!("{}", "FAIL (write temp)".red().bold());
        return 1;
    }

    let mut clang_cmd = Command::new("clang");
    clang_cmd
        .arg("-O0")
        .arg("-Wno-override-module")
        .arg(ll_path.to_str().unwrap());

    if let Some(rt) = runtime {
        clang_cmd.arg(rt);
    }
    clang_cmd.arg("-o").arg(bin_path.to_str().unwrap()).arg("-lm");

    if let Some(flags) = clang_flags {
        for flag in flags.split_whitespace() {
            clang_cmd.arg(flag);
        }
    }

    let start = Instant::now();
    match clang_cmd.output() {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let first = stderr.lines().next().unwrap_or("unknown");
                println!("{}\n   {}", "FAIL".red().bold(), first);
                let _ = std::fs::remove_file(&ll_path);
                return 1;
            }
            let elapsed = start.elapsed();
            let size = std::fs::metadata(&bin_path).map(|m| m.len()).unwrap_or(0);
            println!("{} ({} bytes, {:.1}s)", "OK".green().bold(), size, elapsed.as_secs_f64());
        }
        Err(e) => {
            println!("{} ({})", "FAIL".red().bold(), e);
            let _ = std::fs::remove_file(&ll_path);
            return 1;
        }
    }

    // Step 4: Run the binary
    print!("4. Running binary... ");
    std::io::Write::flush(&mut std::io::stdout()).ok();

    let start = Instant::now();
    let run_result = Command::new(bin_path.to_str().unwrap())
        .output();

    let _ = std::fs::remove_file(&ll_path);

    match run_result {
        Ok(output) => {
            let elapsed = start.elapsed();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if !output.status.success() {
                println!(
                    "{} (exit={}, {:.1}s)",
                    "CRASH".red().bold(),
                    output.status.code().unwrap_or(-1),
                    elapsed.as_secs_f64()
                );
                if !stderr.is_empty() {
                    let preview: String = stderr.chars().take(200).collect();
                    println!("   stderr: {preview}");
                }
                let _ = std::fs::remove_file(&bin_path);
                return 1;
            }

            println!("{} ({:.1}s)", "OK".green().bold(), elapsed.as_secs_f64());

            // Step 5: Check output
            if let Some(expected) = expect {
                println!("5. Checking output:");
                let actual = stdout.trim();
                let expected = expected.trim();

                if actual == expected {
                    println!("   {} output matches expected", "PASS".green().bold());
                } else {
                    println!("   {} output mismatch!", "FAIL".red().bold());
                    println!("   Expected: {:?}", expected);
                    println!("   Actual:   {:?}", actual);

                    // Detailed byte-level analysis for string shift detection
                    let exp_bytes = expected.as_bytes();
                    let act_bytes = actual.as_bytes();

                    if exp_bytes.len() == act_bytes.len() {
                        let mut wrong_positions = Vec::new();
                        for (i, (a, b)) in act_bytes.iter().zip(exp_bytes.iter()).enumerate() {
                            if a != b {
                                wrong_positions.push(i);
                            }
                        }
                        if !wrong_positions.is_empty() {
                            println!(
                                "   Same length ({} bytes), {} bytes differ at positions: {:?}",
                                exp_bytes.len(),
                                wrong_positions.len(),
                                &wrong_positions[..wrong_positions.len().min(10)]
                            );

                            // Check for systematic shift
                            if act_bytes.len() > 1 {
                                for shift in [-1i64, 1, -2, 2] {
                                    let shifted_match = act_bytes.iter().enumerate().all(|(i, &b)| {
                                        let shifted_i = i as i64 + shift;
                                        if shifted_i >= 0 && (shifted_i as usize) < exp_bytes.len() {
                                            b == exp_bytes[shifted_i as usize]
                                        } else {
                                            false
                                        }
                                    });
                                    if shifted_match {
                                        println!(
                                            "   {} String pointer shifted by {} byte(s)!",
                                            "DIAGNOSIS".yellow().bold(),
                                            shift
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                    } else {
                        println!(
                            "   Length mismatch: expected {} bytes, got {} bytes",
                            exp_bytes.len(),
                            act_bytes.len()
                        );
                    }

                    // Show hex dump of actual output
                    if act_bytes.len() <= 64 {
                        print!("   Hex: ");
                        for b in act_bytes {
                            print!("{:02x} ", b);
                        }
                        println!();
                    }

                    let _ = std::fs::remove_file(&bin_path);
                    return 1;
                }
            } else {
                // No expected output — just show what we got
                if !stdout.is_empty() {
                    let preview: String = stdout.chars().take(200).collect();
                    println!("   stdout: {preview}");
                }
            }

            // Parse stderr for compiler diagnostics
            if !stderr.is_empty() {
                let preview: String = stderr.chars().take(200).collect();
                println!("   stderr: {preview}");
            }
        }
        Err(e) => {
            println!("{} ({})", "FAIL".red().bold(), e);
        }
    }

    let _ = std::fs::remove_file(&bin_path);
    0
}
