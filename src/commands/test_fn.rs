use colored::Colorize;
use std::process::Command;

use crate::ir;

pub fn run(
    file: &str,
    function: &str,
    args: &[String],
    expect_ret: Option<i64>,
    compiler: &str,
    timeout: u64,
) -> i32 {
    // Reuse eval to get the result, then check against expected
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

    let has_sret = func.signature.contains("sret(");

    println!(
        "{} Test: @{}({}){}",
        "culebra".green().bold(),
        func.name.cyan(),
        args.join(", "),
        expect_ret.map(|v| format!(" → expect {}", v)).unwrap_or_default().yellow().to_string()
    );

    // Generate a minimal test wrapper
    let wrapper = generate_test_wrapper(&content, &func.name, args, has_sret);

    let tmp_dir = std::env::temp_dir();
    let wrapper_path = tmp_dir.join("culebra_test_fn.ll");
    let binary_path = tmp_dir.join("culebra_test_fn_binary");

    if let Err(e) = std::fs::write(&wrapper_path, &wrapper) {
        eprintln!("{}: Failed to write: {}", "error".red().bold(), e);
        return 1;
    }

    // Compile
    let compile = Command::new(compiler)
        .args([
            "-O0",
            "-Wno-override-module",
            wrapper_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
            "-lm",
        ])
        .output();

    match compile {
        Ok(output) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("  {} compilation failed:", "FAIL".red().bold());
            for line in stderr.lines().take(5) {
                eprintln!("    {}", line);
            }
            return 1;
        }
        Err(e) => {
            eprintln!("{}: {} not found: {}", "error".red().bold(), compiler, e);
            return 1;
        }
        _ => {}
    }

    // Run
    let run_result = Command::new("timeout")
        .args([&timeout.to_string(), binary_path.to_str().unwrap()])
        .output()
        .or_else(|_| Command::new(binary_path.to_str().unwrap()).output());

    let (actual_ret, crashed) = match run_result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);

            if !output.status.success() {
                let code = output.status.code().unwrap_or(-1);
                if code == 139 {
                    println!("  {} SIGSEGV", "CRASH".red().bold());
                } else if code == 137 {
                    println!("  {} timeout ({}s)", "TIMEOUT".yellow().bold(), timeout);
                } else {
                    println!("  {} exit code {}", "FAIL".red().bold(), code);
                }
                (None, true)
            } else {
                // Parse result
                let value = stdout.lines()
                    .find(|l| l.starts_with("culebra_result: "))
                    .and_then(|l| l["culebra_result: ".len()..].trim().parse::<i64>().ok());
                (value, false)
            }
        }
        Err(e) => {
            eprintln!("{}: Failed to run: {}", "error".red().bold(), e);
            (None, true)
        }
    };

    // Cleanup
    let _ = std::fs::remove_file(&wrapper_path);
    let _ = std::fs::remove_file(&binary_path);

    // Report
    if crashed {
        return 1;
    }

    if let Some(expected) = expect_ret {
        if let Some(actual) = actual_ret {
            if actual == expected {
                println!(
                    "  {} @{} returned {} (expected {})",
                    "PASS".green().bold(),
                    func.name,
                    actual,
                    expected
                );
                0
            } else {
                println!(
                    "  {} @{} returned {} (expected {})",
                    "FAIL".red().bold(),
                    func.name,
                    actual.to_string().red().bold(),
                    expected.to_string().green()
                );
                1
            }
        } else {
            println!(
                "  {} could not parse return value",
                "FAIL".red().bold()
            );
            1
        }
    } else {
        if let Some(actual) = actual_ret {
            println!(
                "  {} @{} → {}",
                "OK".green().bold(),
                func.name,
                actual.to_string().yellow().bold()
            );
        } else {
            println!("  {} @{} completed (void)", "OK".green().bold(), func.name);
        }
        0
    }
}

fn generate_test_wrapper(ir: &str, func_name: &str, args: &[String], has_sret: bool) -> String {
    let mut wrapper = String::new();

    // Include original IR
    for line in ir.lines() {
        wrapper.push_str(line);
        wrapper.push('\n');
    }

    // Printf
    wrapper.push_str("\ndeclare i32 @printf(ptr, ...)\n");
    wrapper.push_str("@.test_fmt = private constant [22 x i8] c\"culebra_result: %lld\\0A\\00\"\n");

    // Build call args
    let call_args: Vec<String> = args.iter().enumerate().map(|(i, arg)| {
        if let Ok(v) = arg.parse::<i64>() {
            format!("i64 {}", v)
        } else if arg == "true" {
            "i1 1".to_string()
        } else if arg == "false" {
            "i1 0".to_string()
        } else {
            format!("i64 {}", arg)
        }
    }).collect();

    wrapper.push_str("\ndefine i32 @main() {\nentry:\n");

    if has_sret {
        wrapper.push_str("  %retbuf = alloca [1024 x i8], align 8\n");
        wrapper.push_str(&format!(
            "  call void @{}(ptr %retbuf{}{})\n",
            func_name,
            if call_args.is_empty() { "" } else { ", " },
            call_args.join(", ")
        ));
        wrapper.push_str("  %rv = load i64, ptr %retbuf\n");
    } else {
        wrapper.push_str(&format!(
            "  %rv = call i64 @{}({})\n",
            func_name,
            call_args.join(", ")
        ));
    }

    wrapper.push_str("  call i32 (ptr, ...) @printf(ptr @.test_fmt, i64 %rv)\n");
    wrapper.push_str("  ret i32 0\n}\n");

    wrapper
}
