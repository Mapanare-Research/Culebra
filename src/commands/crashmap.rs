use colored::Colorize;
use regex::Regex;

use crate::ir;

pub fn run(file: &str, offset: usize, struct_name: Option<&str>) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: Failed to read {}: {}", "error".red().bold(), file, e);
            return 1;
        }
    };

    let module = ir::parse_ir(&content);

    if let Some(name) = struct_name {
        // Map offset into a specific struct
        map_struct_offset(&module, &content, name, offset);
    } else {
        // Show all structs and their layouts
        if module.struct_types.is_empty() {
            println!("{} No struct types found in {}", "culebra".green().bold(), file);
            return 0;
        }

        println!(
            "{} Struct types in {} ({} total):",
            "culebra".green().bold(),
            file,
            module.struct_types.len()
        );
        println!();

        for st in &module.struct_types {
            let size = compute_struct_layout_size(&st.fields);
            println!("  {} ({} bytes, {} fields)", st.name.bold(), size, st.fields.len());
        }

        if offset > 0 {
            println!();
            println!("  Checking offset 0x{:x} ({}) across all structs:", offset, offset);
            for st in &module.struct_types {
                if let Some((idx, field_offset, field_ty)) = find_field_at_offset(&st.fields, offset) {
                    println!(
                        "    {} field {} at byte {} — type: {}",
                        st.name.cyan(),
                        idx,
                        field_offset,
                        field_ty
                    );
                }
            }
        }
    }

    0
}

fn map_struct_offset(module: &ir::IRModule, source: &str, name: &str, offset: usize) {
    // Find the struct — try with and without % prefix
    let search_names = [
        format!("%struct.{}", name),
        format!("%{}", name),
        format!("%enum.{}", name),
        name.to_string(),
    ];

    let found = module.struct_types.iter().find(|st| {
        search_names.iter().any(|n| st.name == *n)
    });

    // Also try parsing directly from source for inline types
    let st = if let Some(s) = found {
        s.clone()
    } else {
        // Try to find it by scanning for type definitions
        let re = Regex::new(&format!(
            r"(?m)^(%(?:struct|enum)\.{}\S*)\s*=\s*type\s+(\{{.+\}})",
            regex::escape(name)
        )).unwrap();

        if let Some(caps) = re.captures(source) {
            let def = caps[2].to_string();
            let fields = parse_struct_fields(&def);
            ir::StructType {
                name: caps[1].to_string(),
                definition: def,
                fields,
                estimated_size: 0,
            }
        } else {
            eprintln!(
                "{}: Struct '{}' not found. Available: {}",
                "error".red().bold(),
                name,
                module.struct_types.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")
            );
            return;
        }
    };

    let total_size = compute_struct_layout_size(&st.fields);

    println!(
        "{} Crash map: offset 0x{:x} ({}) in {} ({} bytes)",
        "culebra".green().bold(),
        offset, offset,
        st.name.bold(),
        total_size
    );
    println!();

    // Print full layout
    println!("  {:>6}  {:>6}  {}", "Offset", "Size", "Type");
    println!("  {}", "-".repeat(60));

    let mut current_offset = 0;
    for (i, field) in st.fields.iter().enumerate() {
        let size = estimate_field_size(field);
        let aligned_offset = align_to(current_offset, alignment_of(field));

        let marker = if aligned_offset <= offset && offset < aligned_offset + size {
            format!("  ◄── 0x{:x} HERE (field {})", offset, i).red().bold().to_string()
        } else {
            String::new()
        };

        println!(
            "  {:>6}  {:>6}  {}{}",
            aligned_offset,
            size,
            field.dimmed(),
            marker
        );

        current_offset = aligned_offset + size;
    }

    // Direct answer
    if let Some((idx, field_offset, field_ty)) = find_field_at_offset(&st.fields, offset) {
        println!();
        println!(
            "  {} offset 0x{:x} = field {} (byte {}) — {}",
            "→".green().bold(),
            offset,
            idx,
            field_offset,
            field_ty.yellow().bold()
        );

        if offset == 0 || offset == 0x20 || offset == 0x10 || offset == 0x8 {
            println!(
                "  {} small offset (0x{:x}) suggests null/zero pointer dereference",
                "hint:".cyan().bold(),
                offset
            );
            println!("    The struct itself may be null or zeroinitializer.");
        }
    } else {
        println!();
        println!(
            "  {} offset 0x{:x} is beyond struct size ({})",
            "⚠".yellow().bold(),
            offset,
            total_size
        );
    }
}

fn parse_struct_fields(def: &str) -> Vec<String> {
    // Handle nested braces: split by comma at depth 0
    let inner = def.trim().trim_start_matches('{').trim_end_matches('}');
    let mut fields = Vec::new();
    let mut depth = 0;
    let mut current = String::new();

    for ch in inner.chars() {
        match ch {
            '{' | '[' => { depth += 1; current.push(ch); }
            '}' | ']' => { depth -= 1; current.push(ch); }
            ',' if depth == 0 => {
                let field = current.trim().to_string();
                if !field.is_empty() {
                    fields.push(field);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let last = current.trim().to_string();
    if !last.is_empty() {
        fields.push(last);
    }
    fields
}

fn estimate_field_size(ty: &str) -> usize {
    let ty = ty.trim();
    if ty == "i1" { return 1; }
    if ty == "i8" { return 1; }
    if ty == "i16" { return 2; }
    if ty == "i32" || ty == "float" { return 4; }
    if ty == "i64" || ty == "double" || ty == "ptr" { return 8; }
    if ty.starts_with('{') {
        let fields = parse_struct_fields(ty);
        return compute_struct_layout_size(&fields);
    }
    if ty.starts_with('[') {
        let re = Regex::new(r"^\[(\d+)\s*x\s*(.+)\]$").unwrap();
        if let Some(caps) = re.captures(ty) {
            let n: usize = caps[1].parse().unwrap_or(1);
            return n * estimate_field_size(&caps[2]);
        }
    }
    if ty.starts_with('%') { return 8; } // pointer-sized for named types
    8
}

fn alignment_of(ty: &str) -> usize {
    let ty = ty.trim();
    if ty == "i1" || ty == "i8" { return 1; }
    if ty == "i16" { return 2; }
    if ty == "i32" || ty == "float" { return 4; }
    8 // i64, ptr, double, structs
}

fn align_to(offset: usize, align: usize) -> usize {
    if align == 0 { return offset; }
    (offset + align - 1) & !(align - 1)
}

fn compute_struct_layout_size(fields: &[String]) -> usize {
    let mut offset = 0;
    for f in fields {
        let align = alignment_of(f);
        offset = align_to(offset, align);
        offset += estimate_field_size(f);
    }
    align_to(offset, 8) // struct alignment
}

fn find_field_at_offset(fields: &[String], target: usize) -> Option<(usize, usize, String)> {
    let mut offset = 0;
    for (i, f) in fields.iter().enumerate() {
        let align = alignment_of(f);
        offset = align_to(offset, align);
        let size = estimate_field_size(f);
        if target >= offset && target < offset + size {
            return Some((i, offset, f.clone()));
        }
        offset += size;
    }
    None
}
