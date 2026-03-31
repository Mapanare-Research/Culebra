use crate::ir;
use colored::Colorize;
use std::process::Command;

pub fn run(file: &str, fix_cmd: &str) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    let mod_before = ir::parse_ir(&text);
    let n_lines_before = text.lines().count();

    // Run fix command
    let parts: Vec<&str> = fix_cmd.split_whitespace().collect();
    if parts.is_empty() {
        eprintln!("Empty fix command");
        return 1;
    }

    let result = Command::new(parts[0])
        .args(&parts[1..])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(text.as_bytes());
            }
            child.wait_with_output()
        });

    let output = match result {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Fix command failed: {e}");
            return 1;
        }
    };

    let fixed = String::from_utf8_lossy(&output.stdout).to_string();
    let log = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !log.is_empty() {
        println!("Fix log: {log}");
    }

    let mod_after = ir::parse_ir(&fixed);
    let n_lines_after = fixed.lines().count();

    // Compare
    println!(
        "\n{:<25} {:>10} {:>10} {:>10}  Status",
        "Metric", "Before", "After", "Delta"
    );
    println!("{}", "-".repeat(70));

    let checks = [
        ("Lines", n_lines_before, n_lines_after),
        ("Functions", mod_before.functions.len(), mod_after.functions.len()),
        ("Declares", mod_before.declares.len(), mod_after.declares.len()),
        ("Globals", mod_before.globals.len(), mod_after.globals.len()),
        ("Struct types", mod_before.struct_types.len(), mod_after.struct_types.len()),
        ("String constants", mod_before.string_constants.len(), mod_after.string_constants.len()),
    ];

    let mut issues = 0;
    for (label, before, after) in &checks {
        let delta = *after as i64 - *before as i64;
        let status = if *label == "Functions" && *after == 0 && *before > 0 {
            issues += 1;
            "CRITICAL — all functions deleted!".red().bold().to_string()
        } else if *label == "Functions" && (*after as f64) < (*before as f64 * 0.9) {
            issues += 1;
            format!("WARNING — lost {} functions", before - after)
        } else if delta == 0 {
            "OK".green().to_string()
        } else {
            "changed".yellow().to_string()
        };
        println!(
            "{:<25} {:>10} {:>10} {:>+10}  {}",
            label, before, after, delta, status
        );
    }

    // Lost/gained functions
    let before_names: std::collections::HashSet<_> = mod_before.functions.keys().collect();
    let after_names: std::collections::HashSet<_> = mod_after.functions.keys().collect();

    let lost: Vec<_> = before_names.difference(&after_names).collect();
    let gained: Vec<_> = after_names.difference(&before_names).collect();

    if !lost.is_empty() {
        println!("\nLost functions ({}):", lost.len());
        for name in lost.iter().take(20) {
            println!("  - {}", name);
        }
        if lost.len() > 20 {
            println!("  ... and {} more", lost.len() - 20);
        }
        issues += 1;
    }
    if !gained.is_empty() {
        println!("\nGained functions ({}):", gained.len());
        for name in gained.iter().take(10) {
            println!("  + {}", name);
        }
    }

    // Validate fixed IR
    print!("\nValidating fixed IR... ");
    let (valid, err) = ir::validate_with_llvm_as(&fixed);
    if valid {
        println!("{}", "VALID".green().bold());
    } else {
        println!("{}", "INVALID".red().bold());
        println!("  {err}");
        issues += 1;
    }

    let verdict = if issues == 0 {
        "PASS".green().bold()
    } else {
        "FAIL".red().bold()
    };
    println!("\n{verdict}: {issues} issues found");
    if issues == 0 { 0 } else { 1 }
}
