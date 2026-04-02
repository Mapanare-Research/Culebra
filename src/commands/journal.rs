use colored::Colorize;
use std::io::Write;
use std::time::SystemTime;

const JOURNAL_FILE: &str = ".culebra-journal.jsonl";

#[derive(serde::Serialize, serde::Deserialize)]
struct JournalEntry {
    timestamp: u64,
    action: String, // "note", "bug", "fix", "milestone"
    message: String,
    tags: Vec<String>,
    ir_file: Option<String>,
    function: Option<String>,
}

pub fn run_add(
    action: &str,
    message: &str,
    tags: &[String],
    ir_file: Option<&str>,
    function: Option<&str>,
    journal_path: Option<&str>,
) -> i32 {
    let path = journal_path.unwrap_or(JOURNAL_FILE);

    let entry = JournalEntry {
        timestamp: unix_timestamp(),
        action: action.to_string(),
        message: message.to_string(),
        tags: tags.to_vec(),
        ir_file: ir_file.map(|s| s.to_string()),
        function: function.map(|s| s.to_string()),
    };

    let json = match serde_json::to_string(&entry) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("{}: Failed to serialize: {}", "error".red().bold(), e);
            return 1;
        }
    };

    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: Failed to write {}: {}", "error".red().bold(), path, e);
            return 1;
        }
    };

    let _ = writeln!(file, "{}", json);

    let icon = match action {
        "bug" => "🐛",
        "fix" => "✓",
        "milestone" => "★",
        _ => "•",
    };

    println!(
        "  {} [{}] {} → {}",
        icon,
        action.cyan(),
        message,
        path.dimmed()
    );

    0
}

pub fn run_show(query: Option<&str>, journal_path: Option<&str>, last_n: usize) -> i32 {
    let path = journal_path.unwrap_or(JOURNAL_FILE);

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            println!("{} No journal at {}. Add entries with 'culebra journal add \"message\"'", "culebra".green().bold(), path);
            return 0;
        }
    };

    let entries: Vec<JournalEntry> = content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    if entries.is_empty() {
        println!("{} Journal is empty.", "culebra".green().bold());
        return 0;
    }

    // Filter by query if provided
    let filtered: Vec<&JournalEntry> = if let Some(q) = query {
        let q_lower = q.to_lowercase();
        entries.iter().filter(|e| {
            e.message.to_lowercase().contains(&q_lower)
                || e.action.to_lowercase().contains(&q_lower)
                || e.tags.iter().any(|t| t.to_lowercase().contains(&q_lower))
                || e.function.as_deref().unwrap_or("").to_lowercase().contains(&q_lower)
        }).collect()
    } else {
        entries.iter().collect()
    };

    // Show last N entries
    let start = if filtered.len() > last_n { filtered.len() - last_n } else { 0 };
    let shown = &filtered[start..];

    println!(
        "{} Journal: {} entries{} (showing {})",
        "culebra".green().bold(),
        entries.len(),
        query.map(|q| format!(" matching '{}'", q)).unwrap_or_default(),
        shown.len()
    );
    println!();

    for entry in shown {
        let icon = match entry.action.as_str() {
            "bug" => "✗".red().to_string(),
            "fix" => "✓".green().to_string(),
            "milestone" => "★".yellow().to_string(),
            _ => "•".dimmed().to_string(),
        };

        let time_str = format_timestamp(entry.timestamp);
        let tags_str = if entry.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", entry.tags.join(", ")).dimmed().to_string()
        };

        let func_str = entry.function.as_deref()
            .map(|f| format!(" in {}", f.cyan()))
            .unwrap_or_default();

        println!(
            "  {} {} {}{}{}",
            icon,
            time_str.dimmed(),
            entry.message,
            func_str,
            tags_str
        );
    }

    // Summary
    let bugs = entries.iter().filter(|e| e.action == "bug").count();
    let fixes = entries.iter().filter(|e| e.action == "fix").count();
    let milestones = entries.iter().filter(|e| e.action == "milestone").count();

    println!();
    println!(
        "  {} bugs, {} fixes, {} milestones",
        bugs.to_string().red(),
        fixes.to_string().green(),
        milestones.to_string().yellow()
    );

    0
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn format_timestamp(ts: u64) -> String {
    // Simple relative time
    let now = unix_timestamp();
    let diff = now.saturating_sub(ts);

    if diff < 60 { return "just now".to_string(); }
    if diff < 3600 { return format!("{}m ago", diff / 60); }
    if diff < 86400 { return format!("{}h ago", diff / 3600); }
    format!("{}d ago", diff / 86400)
}
