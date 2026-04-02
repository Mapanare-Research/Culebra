use colored::Colorize;
use regex::Regex;
use std::collections::HashMap;

const SESSION_LOG: &str = ".culebra-session.jsonl";

#[derive(serde::Deserialize)]
struct LogEntry {
    timestamp: u64,
    command: String,
    stdout: String,
    stderr: String,
    exit_code: i32,
    duration_ms: u64,
    category: String,
}

pub fn run(log_file: Option<&str>, verbose: bool) -> i32 {
    let path = log_file.unwrap_or(SESSION_LOG);

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), path, e);
            eprintln!("  Run commands with 'culebra wrap -- <cmd>' to start logging.");
            return 1;
        }
    };

    let entries: Vec<LogEntry> = content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    if entries.is_empty() {
        println!("{} No entries in {}", "culebra".green().bold(), path);
        return 0;
    }

    println!(
        "{} Analyzing {} log entries from {}",
        "culebra".green().bold(),
        entries.len(),
        path
    );
    println!();

    // Categorize entries
    let total = entries.len();
    let failures: Vec<&LogEntry> = entries.iter().filter(|e| e.exit_code != 0).collect();
    let crashes: Vec<&LogEntry> = entries.iter().filter(|e| e.exit_code == 139 || e.exit_code == 134).collect();

    println!("  {}", "Session stats:".bold());
    println!("    Total commands: {}", total);
    println!("    Failures:       {} ({}%)", failures.len(), failures.len() * 100 / total.max(1));
    println!("    Crashes:        {}", crashes.len());
    println!();

    // Extract patterns from failures

    // 1. Clang/llvm-as errors
    let clang_err_re = Regex::new(r"error: (.+)").unwrap();
    let llvm_type_re = Regex::new(r"defined with type '(.+)' but expected '(.+)'").unwrap();
    let llvm_insertvalue_re = Regex::new(r"invalid indices for insertvalue").unwrap();
    let phi_re = Regex::new(r"PHINode should have one entry for each predecessor").unwrap();

    // 2. Valgrind patterns
    let invalid_read_re = Regex::new(r"Invalid read of size (\d+)").unwrap();
    let invalid_write_re = Regex::new(r"Invalid write of size (\d+)").unwrap();
    let address_re = Regex::new(r"Address (0x[0-9a-fA-F]+) is").unwrap();
    let stack_overflow_re = Regex::new(r"stack overflow|Stack overflow").unwrap();

    // 3. Crash patterns
    let segfault_re = Regex::new(r"Segmentation fault|SIGSEGV").unwrap();
    let abort_re = Regex::new(r"Aborted|SIGABRT").unwrap();

    // 4. Function/struct name extraction
    let func_re = Regex::new(r"in (\w+) \(").unwrap();
    let struct_re = Regex::new(r"%struct\.(\w+)").unwrap();

    let mut error_types: HashMap<String, usize> = HashMap::new();
    let mut crash_functions: HashMap<String, usize> = HashMap::new();
    let mut affected_structs: HashMap<String, usize> = HashMap::new();

    for entry in &failures {
        let combined = format!("{}\n{}", entry.stdout, entry.stderr);

        // Classify the error
        if segfault_re.is_match(&combined) {
            *error_types.entry("SIGSEGV (segfault)".to_string()).or_default() += 1;
        }
        if abort_re.is_match(&combined) {
            *error_types.entry("SIGABRT (abort)".to_string()).or_default() += 1;
        }
        if stack_overflow_re.is_match(&combined) {
            *error_types.entry("Stack overflow".to_string()).or_default() += 1;
        }
        if phi_re.is_match(&combined) {
            *error_types.entry("PHI predecessor mismatch".to_string()).or_default() += 1;
        }
        if llvm_insertvalue_re.is_match(&combined) {
            *error_types.entry("Invalid insertvalue indices".to_string()).or_default() += 1;
        }
        if let Some(caps) = llvm_type_re.captures(&combined) {
            *error_types.entry(format!("Type mismatch: {} vs {}", &caps[1], &caps[2])).or_default() += 1;
        }
        if invalid_read_re.is_match(&combined) {
            *error_types.entry("Invalid memory read (valgrind)".to_string()).or_default() += 1;
        }
        if invalid_write_re.is_match(&combined) {
            *error_types.entry("Invalid memory write (valgrind)".to_string()).or_default() += 1;
        }

        // Extract crash functions
        for caps in func_re.captures_iter(&combined) {
            let fname = &caps[1];
            if fname != "in" && fname.len() > 2 {
                *crash_functions.entry(fname.to_string()).or_default() += 1;
            }
        }

        // Extract affected structs
        for caps in struct_re.captures_iter(&combined) {
            *affected_structs.entry(caps[1].to_string()).or_default() += 1;
        }

        // Generic clang errors
        for caps in clang_err_re.captures_iter(&entry.stderr) {
            let err = caps[1].trim();
            if err.len() > 10 && err.len() < 200 {
                *error_types.entry(format!("clang: {}", truncate(err, 80))).or_default() += 1;
            }
        }
    }

    // Report error types
    if !error_types.is_empty() {
        let mut sorted: Vec<_> = error_types.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        println!("  {}", "Error patterns detected:".bold());
        for (pattern, count) in &sorted {
            let severity = if pattern.contains("SIGSEGV") || pattern.contains("overflow") {
                "CRIT".red().bold().to_string()
            } else if pattern.contains("mismatch") || pattern.contains("Invalid") {
                "HIGH".yellow().bold().to_string()
            } else {
                "MED ".cyan().to_string()
            };
            println!("    [{}] {} ({}x)", severity, pattern, count);
        }
        println!();
    }

    // Report crash functions
    if !crash_functions.is_empty() {
        let mut sorted: Vec<_> = crash_functions.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        println!("  {}", "Functions involved in failures:".bold());
        for (func, count) in sorted.iter().take(10) {
            println!("    {} ({}x)", func.yellow(), count);
        }
        println!();
    }

    // Report affected structs
    if !affected_structs.is_empty() {
        let mut sorted: Vec<_> = affected_structs.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        println!("  {}", "Structs mentioned in errors:".bold());
        for (s, count) in sorted.iter().take(10) {
            println!("    %struct.{} ({}x)", s.cyan(), count);
        }
        println!();
    }

    // Suggest templates
    println!("  {}", "Template suggestions:".bold());
    let mut suggestions = 0;

    for entry in &failures {
        let combined = format!("{}\n{}", entry.stdout, entry.stderr);

        if llvm_type_re.is_match(&combined) && suggestions < 5 {
            println!(
                "    {} Type mismatch in insertvalue/extractvalue — check 'culebra field-index-audit'",
                "→".green()
            );
            suggestions += 1;
        }
        if phi_re.is_match(&combined) && suggestions < 5 {
            println!(
                "    {} PHI predecessor issue — check 'culebra scan --id phi-predecessor-mismatch'",
                "→".green()
            );
            suggestions += 1;
        }
        if segfault_re.is_match(&combined) && suggestions < 5 {
            if let Some(caps) = address_re.captures(&combined) {
                let addr = &caps[1];
                if addr == "0x0" || addr.starts_with("0x2") || addr.starts_with("0x1") {
                    println!(
                        "    {} Null/small-offset crash at {} — check 'culebra crashmap --offset {}'",
                        "→".green(), addr, addr
                    );
                    suggestions += 1;
                }
            }
        }
        if stack_overflow_re.is_match(&combined) && suggestions < 5 {
            println!(
                "    {} Stack overflow — check 'culebra scan --id loop-counter-no-exit'",
                "→".green()
            );
            suggestions += 1;
        }
    }

    if suggestions == 0 {
        println!("    (no specific suggestions — errors may be new patterns)");
    }

    // Verbose: show individual failures
    if verbose {
        println!();
        println!("  {}", "Individual failures:".bold());
        for (i, entry) in failures.iter().enumerate().take(20) {
            let cmd_short = truncate(&entry.command, 60);
            println!(
                "    {}. [exit {}] {} ({}ms)",
                i + 1,
                entry.exit_code,
                cmd_short.dimmed(),
                entry.duration_ms
            );
            // Show first error line
            let first_err = entry.stderr.lines()
                .find(|l| l.contains("error") || l.contains("Error") || l.contains("SIGSEGV"))
                .unwrap_or("");
            if !first_err.is_empty() {
                println!("       {}", truncate(first_err, 80).red());
            }
        }
    }

    0
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
