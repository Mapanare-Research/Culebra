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

    println!("{}", "=== Bootstrap Status ===".bold());
    println!("Project: {project_name}");

    // Stage outputs
    let mut stage_outputs: Vec<(&str, String, bool)> = Vec::new();

    if let Some(stage_arr) = config.get("stages").and_then(|s| s.as_array()) {
        println!("Stages:  {}\n", stage_arr.len());

        for (i, stage) in stage_arr.iter().enumerate() {
            let name = stage.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let output = stage.get("output").and_then(|v| v.as_str());

            let (status, exists) = if let Some(out_path) = output {
                if std::path::Path::new(out_path).exists() {
                    let meta = std::fs::metadata(out_path).ok();
                    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);

                    // Parse and analyze the IR if it's an .ll file
                    let detail = if out_path.ends_with(".ll") {
                        if let Ok(text) = std::fs::read_to_string(out_path) {
                            let module = crate::ir::parse_ir(&text);
                            let pathologies = crate::ir::run_all_detectors(&module);
                            let errors =
                                pathologies.iter().filter(|p| p.severity == "error").count();
                            format!(
                                "{} ({} bytes, {} fn, {} globals, {} errors)",
                                "EXISTS".green(),
                                size,
                                module.functions.len(),
                                module.globals.len(),
                                errors
                            )
                        } else {
                            format!("{} ({} bytes)", "EXISTS".green(), size)
                        }
                    } else {
                        format!("{} ({} bytes)", "EXISTS".green(), size)
                    };

                    (detail, true)
                } else {
                    ("NOT BUILT".yellow().to_string(), false)
                }
            } else {
                ("no output defined".dimmed().to_string(), false)
            };

            println!("  {}. {:<20} {}", i + 1, name, status);
            if let Some(out) = output {
                stage_outputs.push((name, out.to_string(), exists));
            }
        }

        // Fixed-point analysis: compare consecutive stage outputs
        let existing_outputs: Vec<_> = stage_outputs
            .iter()
            .filter(|(_, _, exists)| *exists)
            .collect();

        if existing_outputs.len() >= 2 {
            println!("\n{}", "--- Fixed-Point Analysis ---".bold());
            for pair in existing_outputs.windows(2) {
                let (name_a, path_a, _) = pair[0];
                let (name_b, path_b, _) = pair[1];

                let hash_a = file_ir_hash(path_a);
                let hash_b = file_ir_hash(path_b);

                match (&hash_a, &hash_b) {
                    (Some(ha), Some(hb)) => {
                        if ha == hb {
                            println!(
                                "  {} {} == {} — {}!",
                                "FIXED".green().bold(),
                                name_a,
                                name_b,
                                "self-hosting achieved".green().bold()
                            );
                        } else {
                            // Detailed diff
                            let text_a = std::fs::read_to_string(path_a).unwrap_or_default();
                            let text_b = std::fs::read_to_string(path_b).unwrap_or_default();
                            let mod_a = crate::ir::parse_ir(&text_a);
                            let mod_b = crate::ir::parse_ir(&text_b);

                            let mut matched = 0;
                            let mut diverged = 0;
                            for (fname, fa) in &mod_a.functions {
                                if let Some(fb) = mod_b.functions.get(fname) {
                                    if fa.body_hash == fb.body_hash {
                                        matched += 1;
                                    } else {
                                        diverged += 1;
                                    }
                                }
                            }
                            let only_a = mod_a.functions.len().saturating_sub(matched + diverged);
                            let only_b = mod_b.functions.len().saturating_sub(matched + diverged);
                            let total = mod_a.functions.len().max(mod_b.functions.len());
                            let pct = if total > 0 {
                                matched * 100 / total
                            } else {
                                0
                            };

                            println!(
                                "  {} {} != {} — {matched}/{total} functions match ({pct}%), {diverged} diverged",
                                "DIFFER".yellow().bold(),
                                name_a,
                                name_b,
                            );
                            if only_a > 0 || only_b > 0 {
                                println!("    Only in {name_a}: {only_a}, Only in {name_b}: {only_b}");
                            }
                        }
                    }
                    _ => {
                        println!("  {} {name_a} vs {name_b}: cannot compare", "SKIP".dimmed());
                    }
                }
            }
        }
    }

    // Test suite status
    if let Some(tests) = config.get("tests").and_then(|t| t.as_array()) {
        println!("\nTests: {} defined", tests.len());
    }

    0
}

fn file_ir_hash(path: &str) -> Option<String> {
    use sha2::{Digest, Sha256};
    let text = std::fs::read_to_string(path).ok()?;
    let mut hasher = Sha256::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with(';') {
            hasher.update(trimmed.as_bytes());
            hasher.update(b"\n");
        }
    }
    Some(format!("{:x}", hasher.finalize()))
}
