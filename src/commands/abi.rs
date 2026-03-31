use crate::ir;
use colored::Colorize;
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug)]
struct CField {
    ty: String,
    name: String,
    size: usize,
    offset: usize,
}

#[derive(Debug)]
struct CStruct {
    name: String,
    fields: Vec<CField>,
    total_size: usize,
}

pub fn run(file: &str, header: Option<&str>) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    let module = ir::parse_ir(&text);

    println!("{}\n", "=== ABI Analysis ===".bold());

    // Show all struct types with estimated sizes
    println!(
        "{} struct types, {} functions\n",
        module.struct_types.len(),
        module.functions.len()
    );

    // IR struct layouts
    println!("{}", "--- IR Struct Layouts ---".bold());
    for st in &module.struct_types {
        let mut offset = 0;
        print!("  {} ({} bytes):", st.name, st.estimated_size);
        for (i, field_ty) in st.fields.iter().enumerate() {
            let fsize = ir_field_size(field_ty);
            if i < 6 {
                print!(" [{}:{}B]", field_ty.trim(), fsize);
            }
            offset += fsize;
        }
        if st.fields.len() > 6 {
            print!(" ...+{} more", st.fields.len() - 6);
        }
        println!();
    }

    // Check function signatures for ABI patterns
    let sret_re = Regex::new(r"ptr sret\(([^)]+)\)").unwrap();
    let byref_re = Regex::new(r"ptr %\w+\.byref").unwrap();
    let byval_re = Regex::new(r"ptr byval\(([^)]+)\)").unwrap();

    let mut sret_funcs: Vec<(&String, String)> = Vec::new();
    let mut byref_count = 0;
    let mut byval_count = 0;
    let mut large_retval = Vec::new();

    let ret_re =
        Regex::new(r"define\s+(?:internal\s+)?(?:dso_local\s+)?(\{[^}]+\})\s+@").unwrap();

    for func in module.functions.values() {
        if let Some(caps) = sret_re.captures(&func.signature) {
            sret_funcs.push((&func.name, caps[1].to_string()));
        }
        if byref_re.is_match(&func.signature) {
            byref_count += 1;
        }
        if byval_re.is_match(&func.signature) {
            byval_count += 1;
        }

        if let Some(caps) = ret_re.captures(&func.signature) {
            let ret_type = &caps[1];
            let field_count = ret_type.matches(',').count() + 1;
            if field_count > 2 {
                let est_size: usize = ret_type
                    .trim_matches(|c| c == '{' || c == '}')
                    .split(',')
                    .map(|f| ir_field_size(f))
                    .sum();
                large_retval.push((&func.name, ret_type.to_string(), est_size));
            }
        }
    }

    println!("\n{}", "--- Calling Convention Patterns ---".bold());
    println!("  sret functions:     {}", sret_funcs.len());
    println!("  byref parameters:   {}", byref_count);
    println!("  byval parameters:   {}", byval_count);

    if !large_retval.is_empty() {
        large_retval.sort_by(|a, b| b.2.cmp(&a.2));
        println!(
            "\n  {} {} functions return large structs by value:",
            "WARNING".yellow().bold(),
            large_retval.len()
        );
        for (name, _ret_type, size) in large_retval.iter().take(15) {
            let needs_sret = sret_funcs.iter().any(|(n, _)| n == name);
            let status = if needs_sret {
                "has sret".green().to_string()
            } else {
                format!("NO sret — ~{size}B by value").red().to_string()
            };
            println!("    {} (~{size}B): {}", name, status);
        }
    }

    // C header cross-reference
    if let Some(hdr_path) = header {
        println!("\n{}", "--- C Header Cross-Reference ---".bold());
        let hdr_text = match std::fs::read_to_string(hdr_path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Failed to read header {hdr_path}: {e}");
                return 1;
            }
        };

        let c_structs = parse_c_structs(&hdr_text);
        let mut issues = 0;

        for cs in &c_structs {
            // Find matching IR struct
            let ir_match = module.struct_types.iter().find(|s| {
                let ir_name = s.name.trim_start_matches('%').to_lowercase();
                let ir_name = ir_name.trim_start_matches("struct.");
                ir_name.contains(&cs.name.to_lowercase())
                    || cs.name.to_lowercase().contains(ir_name)
            });

            match ir_match {
                Some(ir_st) => {
                    let size_match = ir_st.estimated_size == cs.total_size;
                    let field_match = ir_st.fields.len() == cs.fields.len();

                    if size_match && field_match {
                        println!("  {} {} — {} fields, {} bytes", "OK".green().bold(), cs.name, cs.fields.len(), cs.total_size);
                    } else {
                        issues += 1;
                        println!("  {} {}", "MISMATCH".red().bold(), cs.name);
                        if !field_match {
                            println!(
                                "    Fields: IR has {}, C has {}",
                                ir_st.fields.len(),
                                cs.fields.len()
                            );
                        }
                        if !size_match {
                            println!(
                                "    Size: IR ~{} bytes, C ~{} bytes",
                                ir_st.estimated_size, cs.total_size
                            );
                        }

                        // Field-by-field comparison
                        let max_fields = ir_st.fields.len().max(cs.fields.len());
                        for i in 0..max_fields.min(20) {
                            let ir_field = ir_st.fields.get(i).map(|f| f.as_str()).unwrap_or("-");
                            let ir_fsize = ir_st.fields.get(i).map(|f| ir_field_size(f)).unwrap_or(0);

                            if i < cs.fields.len() {
                                let cf = &cs.fields[i];
                                let size_ok = ir_fsize == cf.size;
                                let marker = if size_ok { " " } else { "!" };
                                println!(
                                    "    {marker} [{i}] IR: {} ({ir_fsize}B) | C: {} {} ({cf_size}B, offset {cf_off})",
                                    ir_field.trim(),
                                    cf.ty,
                                    cf.name,
                                    cf_size = cf.size,
                                    cf_off = cf.offset,
                                );
                            } else {
                                println!("    ! [{i}] IR: {} ({ir_fsize}B) | C: -", ir_field.trim());
                            }
                        }
                    }
                }
                None => {
                    println!(
                        "  {} {} ({} fields, ~{}B) — not found in IR",
                        "MISS".yellow().bold(),
                        cs.name,
                        cs.fields.len(),
                        cs.total_size
                    );
                }
            }
        }

        if issues > 0 {
            println!(
                "\n{} {issues} struct layout mismatches found",
                "FAIL".red().bold()
            );
            return 1;
        }
    }

    0
}

fn ir_field_size(ty: &str) -> usize {
    let ty = ty.trim();
    if ty == "ptr" || ty.starts_with("ptr") {
        return 8;
    }
    if ty.starts_with("i1") {
        return 1;
    }
    if ty.starts_with("i8") {
        return 1;
    }
    if ty.starts_with("i16") {
        return 2;
    }
    if ty.starts_with("i32") {
        return 4;
    }
    if ty.starts_with("i64") || ty == "double" {
        return 8;
    }
    if ty == "float" {
        return 4;
    }
    if ty.starts_with('{') {
        return (ty.matches(',').count() + 1) * 8;
    }
    if ty.starts_with('[') {
        let arr_re = Regex::new(r"^\[(\d+)\s*x\s*(.+)\]$").unwrap();
        if let Some(caps) = arr_re.captures(ty) {
            let n: usize = caps[1].parse().unwrap_or(1);
            return n * ir_field_size(&caps[2]);
        }
    }
    if ty.starts_with('%') {
        return 8; // struct pointer / opaque
    }
    8
}

fn c_type_size(ty: &str) -> usize {
    let ty = ty.trim().trim_end_matches('*');
    match ty {
        "char" | "int8_t" | "uint8_t" | "bool" | "_Bool" => 1,
        "short" | "int16_t" | "uint16_t" => 2,
        "int" | "int32_t" | "uint32_t" | "float" => 4,
        "long" | "long long" | "int64_t" | "uint64_t" | "size_t" | "ssize_t" | "double"
        | "intptr_t" | "uintptr_t" | "ptrdiff_t" => 8,
        _ => {
            if ty.contains('*') || ty.starts_with("const ") && ty.contains('*') {
                8 // pointer
            } else {
                8 // unknown, assume 8
            }
        }
    }
}

fn parse_c_structs(text: &str) -> Vec<CStruct> {
    let mut results = Vec::new();

    // Match: typedef struct { ... } Name;
    // and:   struct Name { ... };
    let typedef_re = Regex::new(
        r"typedef\s+struct\s*(?:\w+\s*)?\{([^}]+)\}\s*(\w+)\s*;"
    ).unwrap();
    let struct_re = Regex::new(
        r"struct\s+(\w+)\s*\{([^}]+)\}\s*;"
    ).unwrap();

    for caps in typedef_re.captures_iter(text) {
        let body = &caps[1];
        let name = caps[2].to_string();
        let fields = parse_c_fields(body);
        let total_size = fields.last().map(|f| f.offset + f.size).unwrap_or(0);
        results.push(CStruct {
            name,
            fields,
            total_size,
        });
    }

    for caps in struct_re.captures_iter(text) {
        let name = caps[1].to_string();
        let body = &caps[2];
        let fields = parse_c_fields(body);
        let total_size = fields.last().map(|f| f.offset + f.size).unwrap_or(0);
        results.push(CStruct {
            name,
            fields,
            total_size,
        });
    }

    results
}

fn parse_c_fields(body: &str) -> Vec<CField> {
    let mut fields = Vec::new();
    let mut offset = 0;

    let field_re = Regex::new(r"([\w\s*]+?)\s+(\w+)\s*;").unwrap();

    for caps in field_re.captures_iter(body) {
        let ty = caps[1].trim().to_string();
        let name = caps[2].to_string();
        let is_ptr = ty.contains('*');
        let size = if is_ptr { 8 } else { c_type_size(&ty) };

        // Align offset
        let align = size.min(8);
        if align > 0 {
            offset = (offset + align - 1) / align * align;
        }

        fields.push(CField {
            ty,
            name,
            size,
            offset,
        });
        offset += size;
    }

    fields
}
