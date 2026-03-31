use colored::Colorize;
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub fn run(patterns: &str, dir: &str, cmd: &[String]) -> i32 {
    if cmd.is_empty() {
        eprintln!("No command specified. Usage: culebra watch -- <command>");
        return 1;
    }

    let extensions: Vec<String> = patterns
        .split(',')
        .map(|p| p.trim().trim_start_matches("*.").to_string())
        .collect();

    println!(
        "{} Watching {} for [{}] changes",
        "WATCH".cyan().bold(),
        dir,
        extensions.join(", ")
    );
    println!("  Command: {}", cmd.join(" "));
    println!("  Press Ctrl+C to stop\n");

    // Run once immediately
    run_command(cmd);

    let (tx, rx) = mpsc::channel();

    let mut watcher = match notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create watcher: {e}");
            return 1;
        }
    };

    if watcher
        .watch(Path::new(dir), RecursiveMode::Recursive)
        .is_err()
    {
        eprintln!("Failed to watch directory: {dir}");
        return 1;
    }

    let mut last_run = Instant::now();
    let debounce = Duration::from_millis(500);

    loop {
        match rx.recv() {
            Ok(event) => {
                // Only trigger on modify/create events
                if !matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_)
                ) {
                    continue;
                }

                // Check if any changed file matches our extensions
                let matched = event.paths.iter().any(|p| {
                    if let Some(ext) = p.extension() {
                        extensions.iter().any(|e| ext.to_str() == Some(e.as_str()))
                    } else {
                        false
                    }
                });

                if !matched {
                    continue;
                }

                // Debounce
                if last_run.elapsed() < debounce {
                    continue;
                }
                last_run = Instant::now();

                // Show which files changed
                for p in &event.paths {
                    if let Some(name) = p.file_name() {
                        println!(
                            "\n{} {} changed",
                            "-->".cyan(),
                            name.to_string_lossy()
                        );
                    }
                }

                run_command(cmd);
            }
            Err(_) => break,
        }
    }

    0
}

fn run_command(cmd: &[String]) {
    let start = Instant::now();
    println!("{} {}", "RUN".cyan().bold(), cmd.join(" "));

    let result = Command::new(&cmd[0]).args(&cmd[1..]).status();

    let elapsed = start.elapsed();
    match result {
        Ok(status) => {
            if status.success() {
                println!(
                    "{} ({:.1}s)\n",
                    "DONE".green().bold(),
                    elapsed.as_secs_f64()
                );
            } else {
                println!(
                    "{} (exit={}, {:.1}s)\n",
                    "FAIL".red().bold(),
                    status.code().unwrap_or(-1),
                    elapsed.as_secs_f64()
                );
            }
        }
        Err(e) => {
            println!("{} ({})\n", "ERROR".red().bold(), e);
        }
    }
}
