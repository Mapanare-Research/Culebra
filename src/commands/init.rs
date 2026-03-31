use colored::Colorize;

const TEMPLATE: &str = r#"# Culebra — compiler forge config
# https://github.com/Mapanare-Research/Culebra

[project]
name = "my-compiler"
source_lang = "my-lang"
target = "llvm"                    # llvm, wasm, native
compiler = "./my-compiler"         # path to compiler binary (used by `culebra test`)
runtime = "runtime/my_runtime.c"   # C runtime to link (optional)

# Define your bootstrap stages.
# Each stage compiles the next. Fixed-point = stage N output == stage N+1 output.

[[stages]]
name = "bootstrap"
cmd = "python bootstrap/compile.py {input}"
input = "src/compiler.ml"
output = "/tmp/stage1.ll"
validate = true         # run llvm-as on output

[[stages]]
name = "stage1"
cmd = "{prev_output} {input}"
input = "src/compiler.ml"
output = "/tmp/stage2.ll"
validate = true

[[stages]]
name = "stage2"
cmd = "{prev_output} {input}"
input = "src/compiler.ml"
output = "/tmp/stage3.ll"
validate = true

# Runtime tests — `culebra test` compiles each, runs it, checks stdout
[[tests]]
name = "hello"
source = 'fn main() { print("hello") }'
expect = "hello"

[[tests]]
name = "math"
source = "fn main() { print(2 + 3) }"
expect = "5"

# [[tests]]
# name = "from_file"
# source_file = "tests/golden/01_hello.mn"
# expect = "hello world"
"#;

pub fn run() -> i32 {
    let path = "culebra.toml";
    if std::path::Path::new(path).exists() {
        eprintln!("{} already exists", path);
        return 1;
    }

    match std::fs::write(path, TEMPLATE) {
        Ok(()) => {
            println!("{} Created {}", "OK".green().bold(), path);
            println!("Edit the stages and tests to match your compiler pipeline.");
            0
        }
        Err(e) => {
            eprintln!("Failed to write {path}: {e}");
            1
        }
    }
}
