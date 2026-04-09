use colored::Colorize;
use regex::Regex;
use std::io::{self, Write};

use crate::ir;
use crate::template::engine::{self, Finding};
use crate::template::loader;
use crate::template::schema::FileType;

pub fn run(
    file: &str,
    template_id: &str,
    dry_run: bool,
    interactive: bool,
    template_path: Option<&str>,
) -> i32 {
    let templates_dir = if let Some(path) = template_path {
        std::path::PathBuf::from(path)
    } else {
        match loader::find_templates_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("{}: No templates directory found.", "error".red().bold());
                return 1;
            }
        }
    };

    let templates = loader::load_templates(&templates_dir);
    let matching: Vec<_> = templates.iter().filter(|t| t.id == template_id).collect();
    if matching.is_empty() {
        eprintln!("{}: Template '{}' not found.", "error".red().bold(), template_id);
        return 1;
    }

    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let is_c = file.ends_with(".c") || file.ends_with(".h");
    let module = if is_c {
        ir::parse_ir_from_raw(&content)
    } else {
        ir::parse_ir(&content)
    };

    // Run the template to find findings
    let template = matching[0];
    if is_c && template.scope.file_type == FileType::LlvmIr {
        eprintln!("{}: Template targets IR but file is C source.", "error".red().bold());
        return 1;
    }
    if !is_c && template.scope.file_type == FileType::CSource {
        eprintln!("{}: Template targets C source but file is IR.", "error".red().bold());
        return 1;
    }

    let findings = engine::run_template(template, &module);
    if findings.is_empty() {
        println!("{} No findings for '{}'.", "OK".green().bold(), template_id);
        return 0;
    }

    println!(
        "{} {} finding(s) for '{}'",
        "fix".cyan().bold(),
        findings.len(),
        template_id
    );

    // Try built-in quick-fixes first, fall back to template autofix
    let result = match template_id {
        "byte-count-mismatch" => apply_byte_count_fix(&content, &findings, file, dry_run, interactive),
        "unaligned-string-constant" => apply_align_fix(&content, &findings, file, dry_run, interactive),
        "break-inside-nested-control" => {
            println!("  {} This template requires .mn source rewriting.", "note".yellow());
            println!("  Suggested pattern: replace break with done-flag pattern.");
            println!("  Use 'culebra explain {} {}' to see affected locations.", file, template_id);
            return 0;
        }
        _ => {
            // Fall back to template's autofix if available
            let fixable: Vec<_> = findings.iter().filter(|f| f.autofix.is_some()).collect();
            if fixable.is_empty() {
                println!("  {} No autofix available for '{}'.", "note".yellow(), template_id);
                return 0;
            }
            apply_template_autofix(&content, &findings, file, dry_run, interactive)
        }
    };

    result
}

/// Fix byte-count-mismatch: recalculate [N x i8] from actual c"..." content
fn apply_byte_count_fix(
    content: &str,
    findings: &[Finding],
    file: &str,
    dry_run: bool,
    interactive: bool,
) -> i32 {
    let re = Regex::new(r#"@([.\w]+)\s*=\s*(?:private\s+)?(?:unnamed_addr\s+)?constant\s+\[(\d+)\s+x\s+i8\]\s+c"((?:[^"\\]|\\[0-9A-Fa-f]{2})*)""#).unwrap();
    let mut result = content.to_string();
    let mut fix_count = 0;

    let finding_lines: std::collections::HashSet<usize> = findings.iter().map(|f| f.line).collect();

    for (line_idx, line) in content.lines().enumerate() {
        if !finding_lines.contains(&(line_idx + 1)) {
            continue;
        }
        if let Some(caps) = re.captures(line) {
            let declared: usize = caps[2].parse().unwrap_or(0);
            let raw_str = &caps[3];
            // Count actual bytes: each \XX is 1 byte, each normal char is 1 byte
            let actual = count_c_string_bytes(raw_str);
            if declared != actual {
                let old_decl = format!("[{} x i8]", declared);
                let new_decl = format!("[{} x i8]", actual);
                let new_line = line.replace(&old_decl, &new_decl);

                if interactive && !prompt_fix(file, line_idx + 1, line, &new_line) {
                    continue;
                }

                if dry_run {
                    print_diff(file, line_idx + 1, line, &new_line);
                } else {
                    result = result.replacen(line, &new_line, 1);
                    fix_count += 1;
                }
            }
        }
    }

    if dry_run {
        println!("\n  {} Dry run — {} fix(es) would be applied.", "fix".cyan().bold(), fix_count.max(findings.len()));
        0
    } else if fix_count > 0 {
        match std::fs::write(file, &result) {
            Ok(_) => {
                println!("  {} Applied {} fix(es) to {}", "fix".cyan().bold(), fix_count, file);
                0
            }
            Err(e) => {
                eprintln!("  Failed to write: {}", e);
                1
            }
        }
    } else {
        println!("  No fixes applied.");
        0
    }
}

/// Fix unaligned-string-constant: add align 1 to string constants missing alignment
fn apply_align_fix(
    content: &str,
    findings: &[Finding],
    file: &str,
    dry_run: bool,
    interactive: bool,
) -> i32 {
    let re = Regex::new(r#"(constant\s+\[\d+\s+x\s+i8\]\s+c"[^"]*")\s*$"#).unwrap();
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut fix_count = 0;

    let finding_lines: std::collections::HashSet<usize> = findings.iter().map(|f| f.line).collect();

    for (line_idx, line) in content.lines().enumerate() {
        if !finding_lines.contains(&(line_idx + 1)) {
            continue;
        }
        if !line.contains(", align") && re.is_match(line) {
            let new_line = format!("{}, align 1", line);

            if interactive && !prompt_fix(file, line_idx + 1, line, &new_line) {
                continue;
            }

            if dry_run {
                print_diff(file, line_idx + 1, line, &new_line);
            } else {
                lines[line_idx] = new_line;
                fix_count += 1;
            }
        }
    }

    if dry_run {
        println!("\n  {} Dry run — {} fix(es) would be applied.", "fix".cyan().bold(), fix_count.max(findings.len()));
        0
    } else if fix_count > 0 {
        let result = lines.join("\n");
        match std::fs::write(file, &result) {
            Ok(_) => {
                println!("  {} Applied {} fix(es) to {}", "fix".cyan().bold(), fix_count, file);
                0
            }
            Err(e) => {
                eprintln!("  Failed to write: {}", e);
                1
            }
        }
    } else {
        println!("  No fixes applied.");
        0
    }
}

/// Fall back to template-defined autofix (line_replace regex)
fn apply_template_autofix(
    content: &str,
    findings: &[Finding],
    file: &str,
    dry_run: bool,
    interactive: bool,
) -> i32 {
    let fixable: Vec<_> = findings.iter().filter(|f| f.autofix.is_some()).collect();

    if interactive {
        let mut result = content.to_string();
        let mut fix_count = 0;
        for f in &fixable {
            let autofix = f.autofix.as_ref().unwrap();
            if let Ok(re) = Regex::new(&autofix.match_pattern) {
                let preview = re.replace_all(&result, autofix.replace.as_str()).to_string();
                if preview != result {
                    let loc = if let Some(ref func) = f.function {
                        format!("{}:{} ({})", file, f.line, func)
                    } else {
                        format!("{}:{}", file, f.line)
                    };
                    println!("\n  {} {}", f.template_id.bold(), loc.dimmed());
                    println!("  {} {}", "match:".dimmed(), f.matched_text.trim());
                    println!("  {} {} → {}", "fix:".cyan(), autofix.match_pattern.dimmed(), autofix.replace.dimmed());

                    if prompt_yn("  Apply this fix?") {
                        result = preview;
                        fix_count += 1;
                    }
                }
            }
        }
        if fix_count > 0 && !dry_run {
            match std::fs::write(file, &result) {
                Ok(_) => println!("  {} Applied {} fix(es) to {}", "fix".cyan().bold(), fix_count, file),
                Err(e) => {
                    eprintln!("  Failed to write: {}", e);
                    return 1;
                }
            }
        }
        return 0;
    }

    let fixed = engine::apply_autofixes(content, &fixable.iter().cloned().cloned().collect::<Vec<_>>());

    if dry_run {
        println!("\n  {} Dry run — {} fix(es) would be applied:", "fix".cyan().bold(), fixable.len());
        for (i, (orig, new)) in content.lines().zip(fixed.lines()).enumerate() {
            if orig != new {
                print_diff(file, i + 1, orig, new);
            }
        }
        0
    } else {
        match std::fs::write(file, &fixed) {
            Ok(_) => {
                println!("  {} Applied {} fix(es) to {}", "fix".cyan().bold(), fixable.len(), file);
                0
            }
            Err(e) => {
                eprintln!("  Failed to write: {}", e);
                1
            }
        }
    }
}

fn count_c_string_bytes(raw: &str) -> usize {
    let mut count = 0;
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 2 < chars.len() {
            // \XX hex escape = 1 byte
            i += 3;
        } else {
            i += 1;
        }
        count += 1;
    }
    count
}

fn print_diff(file: &str, line: usize, old: &str, new: &str) {
    println!("    {}:{}", file, line);
    println!("    {} {}", "-".red(), old.trim());
    println!("    {} {}", "+".green(), new.trim());
}

fn prompt_fix(file: &str, line: usize, old: &str, new: &str) -> bool {
    println!("\n    {}:{}", file, line);
    println!("    {} {}", "-".red(), old.trim());
    println!("    {} {}", "+".green(), new.trim());
    prompt_yn("    Apply?")
}

fn prompt_yn(msg: &str) -> bool {
    print!("{} [y/n] ", msg);
    io::stdout().flush().ok();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        input.trim().to_lowercase().starts_with('y')
    } else {
        false
    }
}
