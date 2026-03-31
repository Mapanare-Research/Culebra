use colored::Colorize;

pub fn run(config_path: &str) -> i32 {
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

    let project_name = config
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let stages = config
        .get("stages")
        .and_then(|s| s.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    println!("{}", "=== Bootstrap Status ===".bold());
    println!("Project: {project_name}");
    println!("Stages:  {stages}");

    // Check for stage outputs
    if let Some(stage_arr) = config.get("stages").and_then(|s| s.as_array()) {
        println!();
        for (i, stage) in stage_arr.iter().enumerate() {
            let name = stage.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let output = stage.get("output").and_then(|v| v.as_str());

            let status = if let Some(out_path) = output {
                if std::path::Path::new(out_path).exists() {
                    let meta = std::fs::metadata(out_path).ok();
                    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                    format!("{} ({} bytes)", "EXISTS".green(), size)
                } else {
                    "NOT BUILT".yellow().to_string()
                }
            } else {
                "no output defined".dimmed().to_string()
            };

            println!("  {}. {:<20} {}", i + 1, name, status);
        }
    }

    0
}
