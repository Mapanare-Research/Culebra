use colored::Colorize;
use regex::Regex;
use std::io::Write;
use std::process::Command;

use crate::ir;

pub fn run(
    file: &str,
    function: &str,
    args: &[String],
    timeout: u64,
    compiler: &str,
) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    // Find target function
    let func = match module.functions.values().find(|f| f.name == function || f.name.contains(function)) {
        Some(f) => f,
        None => {
            eprintln!("{}: Function '{}' not found", "error".red().bold(), function);
            let close: Vec<_> = module.functions.keys()
                .filter(|k| k.contains(&function[..function.len().min(4).max(1)]))
                .take(10).collect();
            if !close.is_empty() {
                eprintln!("  Similar: {}", close.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
            return 1;
        }
    };

    // Parse function signature to determine return type and param types
    let (ret_type, param_types) = parse_signature(&func.signature);

    println!(
        "{} Eval: @{}({}) → {}",
        "culebra".green().bold(),
        func.name.cyan(),
        args.join(", "),
        ret_type.yellow()
    );

    // Check if function has sret (returns via pointer)
    let has_sret = func.signature.contains("sret(");

    // Generate wrapper IR
    let wrapper = generate_wrapper(&content, &func.name, &ret_type, &param_types, args, has_sret);

    // Write to temp file
    let tmp_dir = std::env::temp_dir();
    let wrapper_path = tmp_dir.join("culebra_eval_wrapper.ll");
    let binary_path = tmp_dir.join("culebra_eval_binary");

    match std::fs::write(&wrapper_path, &wrapper) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}: Failed to write wrapper: {}", "error".red().bold(), e);
            return 1;
        }
    }

    // Try lli first (LLVM interpreter — no compilation needed)
    println!("  Trying lli...");
    let lli_result = Command::new("lli")
        .arg(wrapper_path.to_str().unwrap())
        .output();

    match lli_result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            print_result(&stdout, &stderr, &func.name, &ret_type);
            return 0;
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("error") || stderr.contains("Error") {
                eprintln!("  lli failed: {}", stderr.lines().next().unwrap_or("unknown error"));
            }
        }
        Err(_) => {
            eprintln!("  lli not found, trying clang...");
        }
    }

    // Fallback: compile with clang and run
    println!("  Compiling with {}...", compiler);
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
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("{}: Compilation failed:", "error".red().bold());
            for line in stderr.lines().take(10) {
                eprintln!("  {}", line);
            }
            return 1;
        }
        Err(e) => {
            eprintln!("{}: Failed to run {}: {}", "error".red().bold(), compiler, e);
            return 1;
        }
    }

    // Run the binary
    let run_result = Command::new(binary_path.to_str().unwrap())
        .output();

    match run_result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !output.status.success() {
                eprintln!(
                    "  {} (exit code {})",
                    "crashed".red().bold(),
                    output.status.code().unwrap_or(-1)
                );
                if !stderr.is_empty() {
                    eprintln!("  {}", stderr.lines().next().unwrap_or(""));
                }
                return 1;
            }
            print_result(&stdout, &stderr, &func.name, &ret_type);
        }
        Err(e) => {
            eprintln!("{}: Failed to run binary: {}", "error".red().bold(), e);
            return 1;
        }
    }

    // Cleanup
    let _ = std::fs::remove_file(&wrapper_path);
    let _ = std::fs::remove_file(&binary_path);

    0
}

fn parse_signature(sig: &str) -> (String, Vec<String>) {
    // Extract return type: "define <ret_type> @name(<params>)"
    let ret_re = Regex::new(r"define\s+(?:internal\s+)?(?:dso_local\s+)?(?:void\s+)?(.+?)\s+@").unwrap();
    let param_re = Regex::new(r"@\w+\((.+)\)").unwrap();

    let ret_type = ret_re.captures(sig)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_else(|| "i64".to_string());

    let ret_type = if sig.contains("sret(") || ret_type == "void" {
        // sret functions return void; actual return is via pointer
        "void".to_string()
    } else {
        ret_type
    };

    let params: Vec<String> = if let Some(caps) = param_re.captures(sig) {
        let param_str = &caps[1];
        parse_param_types(param_str)
    } else {
        Vec::new()
    };

    (ret_type, params)
}

fn parse_param_types(params: &str) -> Vec<String> {
    let mut types = Vec::new();
    let mut depth = 0;
    let mut current = String::new();

    for ch in params.chars() {
        match ch {
            '{' | '(' | '[' => { depth += 1; current.push(ch); }
            '}' | ')' | ']' => { depth -= 1; current.push(ch); }
            ',' if depth == 0 => {
                let ty = extract_type_from_param(current.trim());
                if !ty.is_empty() {
                    types.push(ty);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let ty = extract_type_from_param(current.trim());
    if !ty.is_empty() {
        types.push(ty);
    }
    types
}

fn extract_type_from_param(param: &str) -> String {
    let param = param.trim();
    // Skip sret/byref/byval attributes
    if param.starts_with("ptr sret(") || param.starts_with("ptr byref(") {
        return "ptr".to_string();
    }
    // "i64 %name" -> "i64", "{ptr, i64} %name" -> "{ptr, i64}"
    let re = Regex::new(r"^(.+?)\s+%").unwrap();
    if let Some(caps) = re.captures(param) {
        return caps[1].trim().to_string();
    }
    // Might be just a type with no name
    param.split_whitespace().next().unwrap_or("i64").to_string()
}

fn generate_wrapper(
    original_ir: &str,
    func_name: &str,
    ret_type: &str,
    _param_types: &[String],
    args: &[String],
    has_sret: bool,
) -> String {
    let mut wrapper = String::new();

    // Include the original IR (minus any existing main)
    for line in original_ir.lines() {
        // Skip existing main function
        if line.starts_with("define") && line.contains("@main(") {
            // Skip until closing brace
            continue;
        }
        wrapper.push_str(line);
        wrapper.push('\n');
    }

    // Add printf declaration
    wrapper.push_str("\ndeclare i32 @printf(ptr, ...)\n");

    // Build argument string for the call
    let mut call_args = String::new();
    for (i, arg) in args.iter().enumerate() {
        if i > 0 { call_args.push_str(", "); }

        // Detect argument type
        if arg.starts_with('"') && arg.ends_with('"') {
            // String argument: create {ptr, i64} struct
            let s = &arg[1..arg.len()-1];
            let len = s.len();
            wrapper.push_str(&format!(
                "@.culebra_str_{} = private constant [{} x i8] c\"{}\\00\"\n",
                i, len + 1, escape_llvm_string(s)
            ));
            call_args.push_str(&format!("{{ ptr, i64 }} @.culebra_str_arg_{}", i));
        } else if arg.parse::<i64>().is_ok() {
            // Integer
            call_args.push_str(&format!("i64 {}", arg));
        } else if arg.parse::<f64>().is_ok() {
            // Float
            call_args.push_str(&format!("double {}", arg));
        } else if arg == "true" {
            call_args.push_str("i1 1");
        } else if arg == "false" {
            call_args.push_str("i1 0");
        } else {
            // Assume i64
            call_args.push_str(&format!("i64 {}", arg));
        }
    }

    // Generate main that calls the function and prints result
    wrapper.push_str(&format!("\n@.culebra_fmt_i64 = private constant [22 x i8] c\"culebra_result: %lld\\0A\\00\"\n"));
    wrapper.push_str(&format!("@.culebra_fmt_str = private constant [22 x i8] c\"culebra_result: %.*s\\0A\\00\"\n"));

    wrapper.push_str("\ndefine i32 @main() {\nentry:\n");

    if has_sret {
        // sret: allocate return buffer, pass as first arg
        wrapper.push_str("  %retbuf = alloca [1024 x i8], align 8\n");
        wrapper.push_str(&format!(
            "  call void @{}(ptr %retbuf{}{})\n",
            func_name,
            if call_args.is_empty() { "" } else { ", " },
            call_args
        ));
        wrapper.push_str("  ; sret return — print first i64 of result\n");
        wrapper.push_str("  %rv = load i64, ptr %retbuf\n");
        wrapper.push_str("  call i32 (ptr, ...) @printf(ptr @.culebra_fmt_i64, i64 %rv)\n");
    } else if ret_type == "i64" || ret_type == "i32" || ret_type == "i1" {
        wrapper.push_str(&format!(
            "  %rv = call {} @{}({})\n",
            ret_type, func_name, call_args
        ));
        if ret_type == "i1" {
            wrapper.push_str("  %rv64 = zext i1 %rv to i64\n");
            wrapper.push_str("  call i32 (ptr, ...) @printf(ptr @.culebra_fmt_i64, i64 %rv64)\n");
        } else {
            wrapper.push_str(&format!(
                "  %rv64 = sext {} %rv to i64\n",
                ret_type
            ));
            wrapper.push_str("  call i32 (ptr, ...) @printf(ptr @.culebra_fmt_i64, i64 %rv64)\n");
        }
    } else if ret_type == "void" {
        wrapper.push_str(&format!(
            "  call void @{}({})\n",
            func_name, call_args
        ));
        wrapper.push_str("  call i32 (ptr, ...) @printf(ptr @.culebra_fmt_i64, i64 0)\n");
    } else {
        // Complex return type — call and print first field
        wrapper.push_str(&format!(
            "  %rv = call {} @{}({})\n",
            ret_type, func_name, call_args
        ));
        wrapper.push_str("  %rv0 = extractvalue ");
        wrapper.push_str(ret_type);
        wrapper.push_str(" %rv, 0\n");
        // Try to print as i64
        wrapper.push_str("  ; printing first field of complex return\n");
        wrapper.push_str("  call i32 (ptr, ...) @printf(ptr @.culebra_fmt_i64, i64 0)\n");
    }

    wrapper.push_str("  ret i32 0\n}\n");

    wrapper
}

fn escape_llvm_string(s: &str) -> String {
    let mut result = String::new();
    for ch in s.bytes() {
        if ch == b'\\' {
            result.push_str("\\5C");
        } else if ch == b'"' {
            result.push_str("\\22");
        } else if ch < 0x20 || ch > 0x7e {
            result.push_str(&format!("\\{:02X}", ch));
        } else {
            result.push(ch as char);
        }
    }
    result
}

fn print_result(stdout: &str, stderr: &str, func_name: &str, ret_type: &str) {
    // Look for our marker line
    for line in stdout.lines() {
        if line.starts_with("culebra_result: ") {
            let value = &line["culebra_result: ".len()..];
            println!(
                "  {} @{} → {} {}",
                "→".green().bold(),
                func_name.cyan(),
                value.yellow().bold(),
                format!("({})", ret_type).dimmed()
            );
            return;
        }
    }
    // No marker found — print raw output
    if !stdout.is_empty() {
        println!("  stdout: {}", stdout.trim());
    }
    if !stderr.is_empty() {
        println!("  stderr: {}", stderr.trim().dimmed());
    }
}
