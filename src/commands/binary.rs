use colored::Colorize;

pub fn run(file: &str, find: Option<&str>) -> i32 {
    let data = match std::fs::read(file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    println!("{}", "=== Binary Analysis ===".bold());
    println!("File: {} ({} bytes)", file, data.len());

    match goblin::Object::parse(&data) {
        Ok(goblin::Object::Elf(elf)) => {
            println!("Format: ELF ({})", if elf.is_64 { "64-bit" } else { "32-bit" });
            println!("Entry:  0x{:x}", elf.entry);
            println!("Sections:");

            let mut rodata_range: Option<(usize, usize)> = None;

            for sh in &elf.section_headers {
                let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("?");
                if sh.sh_size > 0 {
                    println!(
                        "  {:<20} offset=0x{:08x} size={:>8} flags=0x{:x}",
                        name, sh.sh_offset, sh.sh_size, sh.sh_flags
                    );
                }
                if name == ".rodata" {
                    rodata_range = Some((sh.sh_offset as usize, sh.sh_size as usize));
                }
            }

            // String constant analysis in .rodata
            if let Some((offset, size)) = rodata_range {
                println!("\n{}", "--- .rodata String Constants ---".bold());
                let end = (offset + size).min(data.len());
                let rodata = &data[offset..end];
                let strings = extract_strings(rodata, 4);
                println!("  {} readable strings found (>= 4 chars)", strings.len());
                for (str_offset, s) in strings.iter().take(20) {
                    let addr = offset + str_offset;
                    let preview: String = s.chars().take(60).collect();
                    println!("    0x{:08x}: \"{}{}\"", addr, preview,
                        if s.len() > 60 { "..." } else { "" });
                }
                if strings.len() > 20 {
                    println!("    ... and {} more", strings.len() - 20);
                }
            }

            // Find specific string
            if let Some(needle) = find {
                println!("\n{}", format!("--- Searching for \"{needle}\" ---").bold());
                let needle_bytes = needle.as_bytes();
                let mut found = false;
                for (i, window) in data.windows(needle_bytes.len()).enumerate() {
                    if window == needle_bytes {
                        println!("  Found at file offset 0x{:08x}", i);
                        // Show surrounding bytes
                        let start = i.saturating_sub(4);
                        let end = (i + needle_bytes.len() + 4).min(data.len());
                        let context = &data[start..end];
                        print!("  Context: ");
                        for (j, b) in context.iter().enumerate() {
                            if j >= i - start && j < i - start + needle_bytes.len() {
                                print!("{}", format!("{:02x}", b).green());
                            } else {
                                print!("{:02x}", b);
                            }
                            print!(" ");
                        }
                        println!();
                        found = true;
                    }
                }
                if !found {
                    println!("  {} not found in binary", "NOT FOUND".red().bold());
                    return 1;
                }
            }

            // Symbol count
            println!("\nSymbols: {} total", elf.syms.len());
            let func_syms = elf.syms.iter().filter(|s| s.st_type() == goblin::elf::sym::STT_FUNC).count();
            println!("  Functions: {func_syms}");
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
                println!("\n{}", format!("--- Searching for \"{needle}\" ---").bold());
                let needle_bytes = needle.as_bytes();
                let mut found = false;
                for (i, window) in data.windows(needle_bytes.len()).enumerate() {
                    if window == needle_bytes {
                        println!("  Found at file offset 0x{:08x}", i);
                        found = true;
                    }
                }
                if !found {
                    println!("  {}", "NOT FOUND".red().bold());
                    return 1;
                }
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
