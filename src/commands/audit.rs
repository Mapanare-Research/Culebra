use crate::ir;
use colored::Colorize;

pub fn run(file: &str, only: Option<&str>, _baseline: Option<&str>) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    let mut module = ir::parse_ir(&text);

    if let Some(filter) = only {
        module.functions.retain(|name, _| name.contains(filter));
    }

    let pathologies = ir::run_all_detectors(&module);

    if pathologies.is_empty() {
        println!(
            "{} No pathologies found in {} functions.",
            "OK".green().bold(),
            module.functions.len()
        );
        return 0;
    }

    let errors = pathologies.iter().filter(|p| p.severity == "error").count();
    let warnings = pathologies
        .iter()
        .filter(|p| p.severity == "warning")
        .count();

    println!(
        "Found {} issues ({} errors, {} warnings) in {} functions:\n",
        pathologies.len(),
        errors,
        warnings,
        module.functions.len()
    );

    for p in &pathologies {
        let icon = match p.severity.as_str() {
            "error" => "ERR".red().bold(),
            "warning" => "WRN".yellow().bold(),
            _ => "INF".blue().bold(),
        };
        println!("  [{}] {} @ L{}: {}", icon, p.code, p.line, p.function);
        println!("       {}", p.message);
        if !p.detail.is_empty() {
            println!("       {}", p.detail.dimmed());
        }
    }

    if errors > 0 { 1 } else { 0 }
}
