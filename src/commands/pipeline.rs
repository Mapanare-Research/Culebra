use colored::Colorize;
use std::process::Command;
use std::time::Instant;

pub fn run(config_path: &str, timeout: u64) -> i32 {
    let config_text = match std::fs::read_to_string(config_path) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("No {} found. Run: culebra init", config_path);
            return 1;
        }
    };

    let config: toml::Value = match config_text.parse() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse {config_path}: {e}");
            return 1;
        }
    };

    let stages = match config.get("stages").and_then(|s| s.as_array()) {
        Some(s) => s,
        None => {
            eprintln!("No [[stages]] defined in {config_path}");
            return 1;
        }
    };

    println!("{}\n", "=== Pipeline Run ===".bold());

    let mut prev_output: Option<String> = None;
    let mut all_ok = true;

    for (i, stage) in stages.iter().enumerate() {
        let name = stage
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed");
        let cmd = stage.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
        let input = stage.get("input").and_then(|v| v.as_str());
        let output_file = stage.get("output").and_then(|v| v.as_str());
        let validate = stage
            .get("validate")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let expect_str = stage.get("expect").and_then(|v| v.as_str());

        print!("{}. {}... ", i + 1, name);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let start = Instant::now();

        // Build command
        let mut full_cmd = cmd.to_string();
        if let Some(inp) = input {
            full_cmd = full_cmd.replace("{input}", inp);
        }
        if let Some(prev) = &prev_output {
            full_cmd = full_cmd.replace("{prev_output}", prev);
        }
        if let Some(out) = output_file {
            full_cmd = full_cmd.replace("{output}", out);
        }

        let parts: Vec<&str> = full_cmd.split_whitespace().collect();
        if parts.is_empty() {
            println!("{}", "SKIP (empty command)".yellow());
            continue;
        }

        let result = Command::new(parts[0])
            .args(&parts[1..])
            .output();

        let elapsed = start.elapsed();

        match result {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);

                    // Check expected output
                    if let Some(expected) = expect_str {
                        if stdout.contains(expected) {
                            println!("{} ({:.1}s)", "OK".green().bold(), elapsed.as_secs_f64());
                        } else {
                            let preview: String = stdout.chars().take(80).collect();
                            println!(
                                "{} — expected '{}', got '{}'",
                                "WRONG OUTPUT".red().bold(),
                                expected,
                                preview
                            );
                            all_ok = false;
                        }
                    } else {
                        let lines = stdout.lines().count();
                        println!("{} ({} lines, {:.1}s)", "OK".green().bold(), lines, elapsed.as_secs_f64());
                    }

                    // Validate IR if requested
                    if validate && !stdout.is_empty() {
                        let module = crate::ir::parse_ir(&stdout);
                        let pathologies = crate::ir::run_all_detectors(&module);
                        let errors = pathologies.iter().filter(|p| p.severity == "error").count();
                        if errors > 0 {
                            println!(
                                "   {} IR pathologies ({} errors)",
                                pathologies.len(),
                                errors
                            );
                        }
                    }

                    // Save output path for next stage
                    if let Some(out) = output_file {
                        let _ = std::fs::write(out, stdout.as_bytes());
                        prev_output = Some(out.to_string());
                    } else {
                        prev_output = None;
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let first_line = stderr.lines().next().unwrap_or("unknown error");
                    println!(
                        "{} (exit={}, {:.1}s)\n   {}",
                        "FAIL".red().bold(),
                        output.status.code().unwrap_or(-1),
                        elapsed.as_secs_f64(),
                        first_line
                    );
                    all_ok = false;
                    break;
                }
            }
            Err(e) => {
                println!("{} ({})", "ERROR".red().bold(), e);
                all_ok = false;
                break;
            }
        }
    }

    let verdict = if all_ok {
        "PASS".green().bold()
    } else {
        "FAIL".red().bold()
    };
    let msg = if all_ok { "completed" } else { "failed" };
    println!("\n{verdict}: pipeline {msg}");
    if all_ok { 0 } else { 1 }
}
