use crate::ir;
use colored::Colorize;
use std::collections::HashMap;

pub fn run(file: &str, ir_file: Option<&str>, find: Option<&str>) -> i32 {
    let data = match std::fs::read(file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    // Load IR for cross-referencing if provided
    let ir_module = ir_file.and_then(|path| {
        std::fs::read_to_string(path)
            .ok()
            .map(|text| ir::parse_ir(&text))
    });

    println!("{}", "=== Binary Analysis ===".bold());
    println!("File: {} ({} bytes)", file, data.len());

    match goblin::Object::parse(&data) {
        Ok(goblin::Object::Elf(elf)) => {
            println!(
                "Format: ELF ({})",
                if elf.is_64 { "64-bit" } else { "32-bit" }
            );
            println!("Entry:  0x{:x}", elf.entry);

            // Find .rodata section
            let mut rodata_offset: Option<(u64, u64, u64)> = None; // file_offset, vaddr, size

            println!("\nSections:");
            for sh in &elf.section_headers {
                let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("?");
                if sh.sh_size > 0 {
                    println!(
                        "  {:<20} vaddr=0x{:08x} offset=0x{:08x} size={:>8}",
                        name, sh.sh_addr, sh.sh_offset, sh.sh_size
                    );
                }
                if name == ".rodata" || name == ".rdata" {
                    rodata_offset = Some((sh.sh_offset, sh.sh_addr, sh.sh_size));
                }
            }

            // Build symbol table for string constants
            let mut string_syms: HashMap<String, u64> = HashMap::new();
            for sym in elf.syms.iter() {
                if let Some(name) = elf.strtab.get_at(sym.st_name) {
                    if name.starts_with("str.") || name.starts_with(".str.") {
                        string_syms.insert(name.to_string(), sym.st_value);
                    }
                }
            }

            // String constant analysis
            if let Some((file_off, vaddr, size)) = rodata_offset {
                println!("\n{}", "--- .rodata String Analysis ---".bold());
                let end = (file_off as usize + size as usize).min(data.len());
                let rodata = &data[file_off as usize..end];
                let strings = extract_strings(rodata, 4);
                println!("  {} readable strings (>= 4 chars)", strings.len());

                // Cross-reference with IR if available
                if let Some(ref module) = ir_module {
                    println!("\n{}", "--- IR Cross-Reference ---".bold());

                    let mut issues = 0;
                    let total = module.string_constants.len();

                    // For each IR string constant, find it in .rodata
                    for sc in &module.string_constants {
                        let ir_content = decode_llvm_string(&sc.content);
                        if ir_content.is_empty() {
                            continue;
                        }

                        // Search for the decoded content in .rodata
                        let found_offsets: Vec<usize> = find_all_bytes(rodata, &ir_content);

                        if found_offsets.is_empty() && ir_content.len() >= 4 {
                            issues += 1;
                            if issues <= 10 {
                                println!(
                                    "  {} {} [{} x i8]: not found in .rodata",
                                    "MISS".red().bold(),
                                    sc.name,
                                    sc.declared_size
                                );
                            }
                        } else if found_offsets.len() >= 1 && ir_content.len() >= 4 {
                            // Check for off-by-1: is the byte BEFORE the string different
                            // from what the GEP would give?
                            for &off in &found_offsets {
                                if off > 0 {
                                    let byte_before = rodata[off - 1];
                                    let first_byte = ir_content[0];
                                    // If byte_before == first_byte, there might be an alignment issue
                                    if byte_before == first_byte && off >= 2 {
                                        // Check if shifting by -1 produces a valid duplicate
                                        let shifted = &rodata[off - 1..off - 1 + ir_content.len().min(rodata.len() - off + 1)];
                                        if shifted == ir_content.as_slice() {
                                            issues += 1;
                                            if issues <= 10 {
                                                println!(
                                                    "  {} {} at rodata+0x{:x}: duplicate at -1 (potential shift)",
                                                    "SHIFT?".yellow().bold(),
                                                    sc.name,
                                                    off
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if issues == 0 {
                        println!(
                            "  {} All {} IR string constants found in .rodata",
                            "OK".green().bold(),
                            total
                        );
                    } else {
                        println!(
                            "\n  {} {issues} issues ({total} constants checked)",
                            if issues > 0 { "WARNING" } else { "OK" }.yellow().bold()
                        );
                    }
                }
            }

            // Find specific string
            if let Some(needle) = find {
                println!(
                    "\n{}",
                    format!("--- Searching for \"{needle}\" ---").bold()
                );
                find_string_in_binary(&data, needle, rodata_offset);
            }

            // Symbol stats
            println!("\nSymbols: {} total", elf.syms.len());
            let func_syms = elf
                .syms
                .iter()
                .filter(|s| s.st_type() == goblin::elf::sym::STT_FUNC)
                .count();
            let obj_syms = elf
                .syms
                .iter()
                .filter(|s| s.st_type() == goblin::elf::sym::STT_OBJECT)
                .count();
            println!("  Functions: {func_syms}, Objects: {obj_syms}");
            if !string_syms.is_empty() {
                println!("  String constants: {}", string_syms.len());
            }
        }
        Ok(goblin::Object::PE(pe)) => {
            println!("Format: PE (Windows)");
            println!("Entry:  0x{:x}", pe.entry);
            println!("Sections:");
            for section in &pe.sections {
                let name = String::from_utf8_lossy(&section.name).replace('\0', "");
                println!(
                    "  {:<20} vaddr=0x{:08x} size={:>8}",
                    name, section.virtual_address, section.virtual_size
                );
            }

            if let Some(needle) = find {
                find_string_in_binary(&data, needle, None);
            }
        }
        Ok(_) => {
            println!("Format: Unknown/unsupported");
        }
        Err(e) => {
            eprintln!("Failed to parse binary: {e}");
            return 1;
        }
    }

    0
}

fn decode_llvm_string(content: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            result.push(hi * 16 + lo);
            i += 3;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }
    result
}

fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

fn find_all_bytes(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    let mut results = Vec::new();
    for i in 0..=haystack.len() - needle.len() {
        if &haystack[i..i + needle.len()] == needle {
            results.push(i);
        }
    }
    results
}

fn find_string_in_binary(data: &[u8], needle: &str, rodata: Option<(u64, u64, u64)>) {
    let needle_bytes = needle.as_bytes();
    let mut found = false;

    for (i, window) in data.windows(needle_bytes.len()).enumerate() {
        if window == needle_bytes {
            found = true;
            let in_rodata = rodata
                .map(|(off, _vaddr, size)| {
                    let off = off as usize;
                    let end = off + size as usize;
                    i >= off && i < end
                })
                .unwrap_or(false);

            let section = if in_rodata { ".rodata" } else { "other" };
            println!(
                "  Found at file offset 0x{:08x} ({})",
                i, section
            );

            // Show context bytes with highlighting
            let start = i.saturating_sub(4);
            let end = (i + needle_bytes.len() + 4).min(data.len());
            let context = &data[start..end];
            print!("  Context: ");
            for (j, b) in context.iter().enumerate() {
                let abs_pos = start + j;
                if abs_pos >= i && abs_pos < i + needle_bytes.len() {
                    print!("{}", format!("{:02x}", b).green());
                } else {
                    print!("{:02x}", b);
                }
                print!(" ");
            }
            println!();

            // Check for off-by-1: does the PREVIOUS byte match the first byte?
            if i > 0 && data[i - 1] == needle_bytes[0] {
                println!(
                    "  {} Byte at offset-1 (0x{:02x}) matches first byte — possible pointer shift!",
                    "SHIFT?".yellow().bold(),
                    data[i - 1]
                );
            }
        }
    }

    if !found {
        println!("  {} not found in binary", "NOT FOUND".red().bold());
    }
}

fn extract_strings(data: &[u8], min_len: usize) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let mut current = String::new();
    let mut start = 0;

    for (i, &byte) in data.iter().enumerate() {
        if byte.is_ascii_graphic() || byte == b' ' {
            if current.is_empty() {
                start = i;
            }
            current.push(byte as char);
        } else {
            if current.len() >= min_len {
                results.push((start, current.clone()));
            }
            current.clear();
        }
    }
    if current.len() >= min_len {
        results.push((start, current));
    }
    results
}
