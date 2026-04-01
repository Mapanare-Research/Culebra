use colored::Colorize;
use regex::Regex;
use std::collections::{HashMap, HashSet};

use crate::ir;
use crate::template::engine::{self, Finding};
use crate::template::loader;
use crate::template::schema::{FileType, Severity};

/// Run everything in one shot: triage + missing-types + field-index-audit + health
pub fn run(file: &str, struct_filter: Option<&str>) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);
    let mut exit_code = 0;

    // Header
    let total_insns: usize = module.functions.values().map(|f| f.metrics.instructions).sum();
    println!(
        "{} Summary: {} ({} functions, {} instructions, {} types)",
        "culebra".green().bold(),
        file,
        module.functions.len(),
        total_insns,
        module.struct_types.len()
    );
    println!();

    // --- 1. Triage (brief) ---
    let templates_dir = match loader::find_templates_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("  {}: No templates directory", "warn".yellow());
            return 1;
        }
    };
    let templates = loader::load_templates(&templates_dir);
    let mut findings: Vec<Finding> = Vec::new();
    for t in &templates {
        if t.scope.file_type == FileType::CrossReference { continue; }
        findings.extend(engine::run_template(t, &module));
    }
    findings.sort_by(|a, b| a.severity.cmp(&b.severity));

    let crit = findings.iter().filter(|f| f.severity == Severity::Critical).count();
    let high = findings.iter().filter(|f| f.severity == Severity::High).count();
    let med = findings.iter().filter(|f| f.severity == Severity::Medium).count();

    // Group by template
    let mut groups: HashMap<String, Vec<&Finding>> = HashMap::new();
    for f in &findings {
        groups.entry(f.template_id.clone()).or_default().push(f);
    }
    let mut sorted_groups: Vec<_> = groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| {
        let sa = a.1.iter().map(|f| &f.severity).min().unwrap();
        let sb = b.1.iter().map(|f| &f.severity).min().unwrap();
        sa.cmp(sb).then_with(|| b.1.len().cmp(&a.1.len()))
    });

    let crit_names: Vec<_> = sorted_groups.iter()
        .filter(|(_, fs)| fs[0].severity == Severity::Critical)
        .map(|(id, _)| id.as_str())
        .collect();

    print!("  {} ", "Scan:".bold());
    if findings.is_empty() {
        println!("{}", "clean".green().bold());
    } else {
        let mut parts = Vec::new();
        if !crit_names.is_empty() {
            parts.push(format!("{} critical ({})", crit_names.len(), crit_names.join(", ")).red().bold().to_string());
        }
        if high > 0 { parts.push(format!("{} high", sorted_groups.iter().filter(|(_, fs)| fs[0].severity == Severity::High).count()).yellow().to_string()); }
        if med > 0 { parts.push(format!("{} medium", sorted_groups.iter().filter(|(_, fs)| fs[0].severity == Severity::Medium).count())); }
        println!("{} root causes, {} findings: {}", sorted_groups.len(), findings.len(), parts.join(", "));
        if crit > 0 { exit_code = 1; }
    }

    // --- 2. Missing types ---
    let type_def_re = Regex::new(r"^(%(?:struct|enum)\.\w+)\s*=\s*type\s").unwrap();
    let type_use_re = Regex::new(r"(%(?:struct|enum)\.\w+)").unwrap();
    let mut defined: HashSet<String> = HashSet::new();
    for line in content.lines() {
        if let Some(caps) = type_def_re.captures(line) {
            defined.insert(caps[1].to_string());
        }
    }
    let mut used: HashSet<String> = HashSet::new();
    for line in content.lines() {
        for caps in type_use_re.captures_iter(line) {
            used.insert(caps[1].to_string());
        }
    }
    let missing: Vec<_> = used.difference(&defined).cloned().collect();

    print!("  {} ", "Types:".bold());
    if missing.is_empty() {
        println!("{} ({} defined)", "all defined".green(), defined.len());
    } else {
        println!(
            "{} — {}",
            format!("{} missing", missing.len()).red().bold(),
            missing.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
        );
        if missing.len() > 5 {
            print!(" +{} more", missing.len() - 5);
        }
        println!();
        exit_code = 1;
    }

    // --- 3. Field index audit ---
    let ev_re = Regex::new(r"extractvalue (%(?:struct|enum)\.\w+) .+, (\d+)").unwrap();
    let mut struct_indices: HashMap<String, HashMap<usize, usize>> = HashMap::new();

    for func in module.functions.values() {
        for line in func.body.lines() {
            if let Some(caps) = ev_re.captures(line.trim()) {
                let name = caps[1].to_string();
                let idx: usize = caps[2].parse().unwrap_or(0);
                if let Some(filter) = struct_filter {
                    if !name.contains(filter) { continue; }
                }
                *struct_indices.entry(name).or_default().entry(idx).or_default() += 1;
            }
        }
    }

    let stuck_zero: Vec<_> = struct_indices.iter()
        .filter(|(_, indices)| {
            let total: usize = indices.values().sum();
            indices.len() == 1 && indices.contains_key(&0) && total > 3
        })
        .map(|(name, indices)| (name.clone(), indices.values().sum::<usize>()))
        .collect();

    print!("  {} ", "Fields:".bold());
    if stuck_zero.is_empty() {
        println!("{}", "no index-0 anomalies".green());
    } else {
        println!(
            "{} — {}",
            format!("{} structs stuck at index 0", stuck_zero.len()).red().bold(),
            stuck_zero.iter().take(3).map(|(n, c)| format!("{} ({}x)", n, c)).collect::<Vec<_>>().join(", ")
        );
        exit_code = 1;
    }

    // --- 4. Struct health ---
    let phi_zeroinit_re = Regex::new(r"= phi (%struct\.\w+) .+zeroinitializer").unwrap();
    let store_zeroinit_re = Regex::new(r"store (%struct\.\w+) zeroinitializer").unwrap();
    let sret_zeroinit_re = Regex::new(r"store %struct\.\w+ zeroinitializer, ptr %__sret__").unwrap();

    let mut phi_zeroinit_count = 0;
    let mut store_zeroinit_count = 0;
    let mut sret_zeroinit_count = 0;

    for func in module.functions.values() {
        for line in func.body.lines() {
            let t = line.trim();
            if phi_zeroinit_re.is_match(t) { phi_zeroinit_count += 1; }
            if store_zeroinit_re.is_match(t) { store_zeroinit_count += 1; }
            if sret_zeroinit_re.is_match(t) { sret_zeroinit_count += 1; }
        }
    }

    print!("  {} ", "Health:".bold());
    let health_issues = phi_zeroinit_count + sret_zeroinit_count;
    if health_issues == 0 {
        println!("{}", "no zeroinit corruption patterns".green());
    } else {
        let mut parts = Vec::new();
        if phi_zeroinit_count > 0 { parts.push(format!("{} PHI zeroinit", phi_zeroinit_count)); }
        if sret_zeroinit_count > 0 { parts.push(format!("{} sret zeroinit", sret_zeroinit_count)); }
        if store_zeroinit_count > 0 { parts.push(format!("{} store zeroinit", store_zeroinit_count)); }
        println!("{}", parts.join(", ").yellow());
    }

    // --- Health score ---
    let score = if crit == 0 && high == 0 && missing.is_empty() && stuck_zero.is_empty() {
        100
    } else if crit == 0 && missing.is_empty() {
        (80.0 - (high as f64 * 2.0).min(30.0)) as i32
    } else {
        let penalties = (crit as f64 * 10.0) + (missing.len() as f64 * 5.0) + (stuck_zero.len() as f64 * 15.0);
        (50.0 - penalties.min(50.0)) as i32
    };
    let score = score.max(0);

    println!();
    let score_str = if score >= 80 {
        format!("{}%", score).green().bold().to_string()
    } else if score >= 50 {
        format!("{}%", score).yellow().bold().to_string()
    } else {
        format!("{}%", score).red().bold().to_string()
    };
    println!("  Score: {}", score_str);

    exit_code
}
