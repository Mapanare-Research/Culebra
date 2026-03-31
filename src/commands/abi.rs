use crate::ir;
use colored::Colorize;
use regex::Regex;
use std::collections::HashMap;

pub fn run(file: &str, header: Option<&str>) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    let module = ir::parse_ir(&text);

    println!("{}", "=== ABI Analysis ===".bold());
    println!();

    // Show all struct types with estimated sizes
    println!("{} struct types found:\n", module.struct_types.len());
    for st in &module.struct_types {
        println!("  {} = {} (~{} bytes, {} fields)",
            st.name, st.definition, st.estimated_size, st.fields.len());
    }

    // Check function signatures for ABI patterns
    let sret_re = Regex::new(r"ptr sret\(([^)]+)\)").unwrap();
    let byref_re = Regex::new(r"ptr %\w+\.byref").unwrap();
    let byval_re = Regex::new(r"ptr byval\(([^)]+)\)").unwrap();

    let mut sret_funcs = Vec::new();
    let mut byref_funcs = Vec::new();
    let mut byval_funcs = Vec::new();
    let mut large_retval = Vec::new();

    for func in module.functions.values() {
        if sret_re.is_match(&func.signature) {
            sret_funcs.push(&func.name);
        }
        if byref_re.is_match(&func.signature) {
            byref_funcs.push(&func.name);
        }
        if byval_re.is_match(&func.signature) {
            byval_funcs.push(&func.name);
        }

        // Check for large struct returns (>16 bytes returned by value)
        let ret_re = Regex::new(r"define\s+(?:internal\s+)?(?:dso_local\s+)?(\{[^}]+\})\s+@").unwrap();
        if let Some(caps) = ret_re.captures(&func.signature) {
            let ret_type = &caps[1];
            let field_count = ret_type.matches(',').count() + 1;
            if field_count > 2 {
                large_retval.push((&func.name, ret_type.to_string(), field_count));
            }
        }
    }

    println!("\n{}", "--- Calling Convention Patterns ---".bold());
    println!("  sret functions:     {}", sret_funcs.len());
    println!("  byref parameters:   {}", byref_funcs.len());
    println!("  byval parameters:   {}", byval_funcs.len());

    if !large_retval.is_empty() {
        println!(
            "\n{} functions return large structs by value (potential ABI issue):",
            large_retval.len()
        );
        for (name, ret_type, fields) in large_retval.iter().take(15) {
            let preview: String = ret_type.chars().take(60).collect();
            println!("  {} -> {} ({} fields)", name, preview, fields);
        }
    }

    // If a C header is provided, cross-reference
    if let Some(hdr_path) = header {
        println!("\n{}", "--- C Header Cross-Reference ---".bold());
        match std::fs::read_to_string(hdr_path) {
            Ok(hdr_text) => {
                let c_struct_re = Regex::new(
                    r"typedef\s+struct\s*\{([^}]+)\}\s*(\w+)\s*;"
                ).unwrap();

                let mut c_structs: HashMap<String, Vec<String>> = HashMap::new();
                for caps in c_struct_re.captures_iter(&hdr_text) {
                    let fields: Vec<String> = caps[1]
                        .lines()
                        .filter(|l| l.contains(';'))
                        .map(|l| l.trim().to_string())
                        .collect();
                    c_structs.insert(caps[2].to_string(), fields);
                }

                for (name, fields) in &c_structs {
                    let c_size: usize = fields.len() * 8; // rough estimate
                    let ir_match = module.struct_types.iter().find(|s| {
                        s.name.contains(name) || s.name.to_lowercase().contains(&name.to_lowercase())
                    });
                    match ir_match {
                        Some(ir_st) => {
                            let status = if ir_st.estimated_size == c_size {
                                "OK".green().to_string()
                            } else {
                                format!("SIZE MISMATCH (IR: ~{}, C: ~{})", ir_st.estimated_size, c_size)
                                    .red().to_string()
                            };
                            println!("  {} ({} fields): {}", name, fields.len(), status);
                        }
                        None => {
                            println!("  {} ({} fields): {}", name, fields.len(), "not found in IR".yellow());
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to read header {hdr_path}: {e}");
            }
        }
    }

    0
}
