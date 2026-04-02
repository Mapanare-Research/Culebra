use colored::Colorize;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime};

const SESSION_LOG: &str = ".culebra-session.jsonl";

pub fn run(command: &[String], log_file: Option<&str>) -> i32 {
    if command.is_empty() {
        eprintln!("{}: No command provided. Usage: culebra wrap -- <command> [args...]", "error".red().bold());
        return 1;
    }

    let log_path = log_file.unwrap_or(SESSION_LOG);
    let cmd_str = command.join(" ");

    let start = Instant::now();
    let timestamp = unix_timestamp();

    // Spawn the command with piped stdout/stderr
    let mut child = match Command::new(&command[0])
        .args(&command[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to run '{}': {}", "error".red().bold(), cmd_str, e);
            // Log the failure
            log_entry(log_path, &LogEntry {
                timestamp,
                command: cmd_str,
                stdout: String::new(),
                stderr: format!("Failed to spawn: {}", e),
                exit_code: -1,
                duration_ms: start.elapsed().as_millis() as u64,
                category: categorize_command(&command[0]),
            });
            return 1;
        }
    };

    // Read stdout and stderr, passing through to terminal
    let mut stdout_buf = String::new();
    let mut stderr_buf = String::new();

    // Read stdout
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("{}", line);
                stdout_buf.push_str(&line);
                stdout_buf.push('\n');
            }
        }
    }

    // Read stderr
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("{}", line);
                stderr_buf.push_str(&line);
                stderr_buf.push('\n');
            }
        }
    }

    let status = child.wait().unwrap_or_else(|_| {
        std::process::Command::new("true").status().unwrap()
    });

    let exit_code = status.code().unwrap_or(-1);
    let duration = start.elapsed().as_millis() as u64;

    // Log the entry
    let entry = LogEntry {
        timestamp,
        command: cmd_str.clone(),
        stdout: truncate_log(&stdout_buf, 10000),
        stderr: truncate_log(&stderr_buf, 5000),
        exit_code,
        duration_ms: duration,
        category: categorize_command(&command[0]),
    };

    log_entry(log_path, &entry);

    // Show a subtle indicator that we logged
    if exit_code != 0 {
        eprintln!(
            "  {} logged (exit {}, {}ms) → {}",
            "culebra".dimmed(),
            exit_code,
            duration,
            log_path.dimmed()
        );
    }

    exit_code
}

#[derive(serde::Serialize)]
struct LogEntry {
    timestamp: u64,
    command: String,
    stdout: String,
    stderr: String,
    exit_code: i32,
    duration_ms: u64,
    category: String,
}

fn log_entry(path: &str, entry: &LogEntry) {
    let json = match serde_json::to_string(entry) {
        Ok(j) => j,
        Err(_) => return,
    };

    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(f) => f,
        Err(_) => return, // Silently fail — don't break the proxied command
    };

    let _ = writeln!(file, "{}", json);
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn truncate_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut result = s[..max].to_string();
        result.push_str(&format!("\n... ({} bytes truncated)", s.len() - max));
        result
    }
}

fn categorize_command(cmd: &str) -> String {
    let base = cmd.rsplit('/').next().unwrap_or(cmd);
    match base {
        "clang" | "gcc" | "cc" => "compile".to_string(),
        "llvm-as" | "llc" | "opt" => "llvm".to_string(),
        "valgrind" => "valgrind".to_string(),
        "ld" | "lld" => "link".to_string(),
        "python3" | "python" => "python".to_string(),
        "culebra" => "culebra".to_string(),
        "wc" | "grep" | "awk" | "sed" | "head" | "tail" | "cat" | "diff" => "inspect".to_string(),
        "objdump" | "readelf" | "nm" | "file" => "binary-inspect".to_string(),
        _ if base.starts_with("mnc-") => "compiler".to_string(),
        _ => "other".to_string(),
    }
}
