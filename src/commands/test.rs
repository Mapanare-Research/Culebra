use colored::Colorize;
use std::process::Command;

pub fn run(config_path: &str, filter: Option<&str>, timeout: u64) -> i32 {
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

    let compiler = config
        .get("project")
        .and_then(|p| p.get("compiler"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let runtime = config
        .get("project")
        .and_then(|p| p.get("runtime"))
        .and_then(|v| v.as_str());

    if compiler.is_empty() {
        eprintln!("No project.compiler defined in {config_path}");
        return 1;
    }

    let tests = match config.get("tests").and_then(|t| t.as_array()) {
        Some(t) => t,
        None => {
            eprintln!("No [[tests]] defined in {config_path}");
            return 1;
        }
    };

    println!("{}\n", "=== Test Suite ===".bold());

    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for test in tests {
        let name = test.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let source = test.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let expect = test.get("expect").and_then(|v| v.as_str());
        let source_file = test.get("source_file").and_then(|v| v.as_str());

        if let Some(f) = filter {
            if !name.contains(f) {
                skipped += 1;
                continue;
            }
        }

        print!("  {:<30} ", name);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        // Write source to temp file
        let tmp_src = std::env::temp_dir().join(format!("culebra_test_{name}.mn"));
        if let Some(sf) = source_file {
            if !std::path::Path::new(sf).exists() {
                println!("{} (source file not found: {sf})", "SKIP".yellow());
                skipped += 1;
                continue;
            }
            let _ = std::fs::copy(sf, &tmp_src);
        } else {
            if std::fs::write(&tmp_src, source).is_err() {
                println!("{}", "FAIL (write temp)".red());
                failed += 1;
                continue;
            }
        }

        // Compile
        let compile = Command::new(compiler)
            .arg(tmp_src.to_str().unwrap())
            .output();

        let ir_text = match compile {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).to_string()
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let first = stderr.lines().next().unwrap_or("?");
                println!("{} (compile: {})", "FAIL".red().bold(), first);
                failed += 1;
                let _ = std::fs::remove_file(&tmp_src);
                continue;
            }
            Err(e) => {
                println!("{} ({})", "FAIL".red().bold(), e);
                failed += 1;
                let _ = std::fs::remove_file(&tmp_src);
                continue;
            }
        };

        // Link
        let ll_path = std::env::temp_dir().join(format!("culebra_test_{name}.ll"));
        let bin_path = std::env::temp_dir().join(format!("culebra_test_{name}_bin"));
        let _ = std::fs::write(&ll_path, &ir_text);

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
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let first = stderr.lines().next().unwrap_or("?");
                println!("{} (link: {})", "FAIL".red().bold(), first);
                failed += 1;
                cleanup(&[&tmp_src, &ll_path, &bin_path]);
                continue;
            }
            Err(e) => {
                println!("{} (link: {})", "FAIL".red().bold(), e);
                failed += 1;
                cleanup(&[&tmp_src, &ll_path, &bin_path]);
                continue;
            }
        }

        // Run
        let run_result = Command::new(bin_path.to_str().unwrap()).output();

        match run_result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let actual = stdout.trim();

                if !output.status.success() {
                    println!(
                        "{} (exit={})",
                        "CRASH".red().bold(),
                        output.status.code().unwrap_or(-1)
                    );
                    failed += 1;
                } else if let Some(exp) = expect {
                    if actual == exp.trim() {
                        println!("{}", "PASS".green().bold());
                        passed += 1;
                    } else {
                        // Detect string shift
                        let diagnosis = detect_shift(actual.as_bytes(), exp.trim().as_bytes());
                        println!("{} (got {:?}){}", "FAIL".red().bold(), truncate(actual, 30), diagnosis);
                        failed += 1;
                    }
                } else {
                    println!("{}", "OK".green().bold());
                    passed += 1;
                }
            }
            Err(e) => {
                println!("{} ({})", "FAIL".red().bold(), e);
                failed += 1;
            }
        }

        cleanup(&[&tmp_src, &ll_path, &bin_path]);
    }

    println!(
        "\n{} passed, {} failed, {} skipped ({} total)",
        passed, failed, skipped, passed + failed + skipped
    );

    if failed > 0 { 1 } else { 0 }
}

fn detect_shift(actual: &[u8], expected: &[u8]) -> String {
    if actual.len() != expected.len() {
        return String::new();
    }
    for shift in [-1i64, 1, -2, 2] {
        let matches = actual.iter().enumerate().all(|(i, &b)| {
            let si = i as i64 + shift;
            si >= 0 && (si as usize) < expected.len() && b == expected[si as usize]
        });
        if matches {
            return format!(" [STRING_SHIFT by {}]", shift);
        }
    }
    String::new()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

fn cleanup(paths: &[&std::path::Path]) {
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
}
