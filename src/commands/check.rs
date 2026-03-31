use crate::ir;
use colored::Colorize;

pub fn run(file: &str) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read {file}: {e}");
            return 1;
        }
    };

    let (valid, err) = ir::validate_with_llvm_as(&text);
    if valid {
        println!("{} {}", "VALID".green().bold(), file);
        if !err.is_empty() {
            println!("  ({})", err);
        }
        0
    } else {
        println!("{} {}", "INVALID".red().bold(), file);
        println!("  {}", err);
        1
    }
}
