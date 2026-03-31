# Culebra

Compiler diagnostics for self-hosting languages that target LLVM. One binary, no dependencies, catches ABI mismatches, IR bugs, binary corruption, and bootstrap divergence before they become mysteries.

[![Rust](https://img.shields.io/badge/Built%20with-Rust-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![LLVM](https://img.shields.io/badge/Targets-LLVM%20IR-262D3A?logo=llvm&logoColor=white)](https://llvm.org/)
[![Born from Mapanare](https://img.shields.io/badge/Born%20from-Mapanare-8B4513)](https://github.com/Mapanare-Research)

---

## Why this exists

Most languages bootstrapped on top of a mature compiler:

- **Rust** started in OCaml before self-hosting about a year later.
- **Go** was written in C until v1.5, then used an automated C-to-Go translator.
- **C++** bootstrapped through Cfront, which translated C++ to C and let C compilers handle code generation.

[Mapanare](https://github.com/Mapanare-Research) doesn't have that luxury. It's an AI-native compiled language targeting LLVM IR, building its own backend from scratch: lexer, AST, type inference, LLVM IR emission. The bootstrap compiler (Stage 0) is written in Python, but there's no mature compiler underneath to fall back on.

That means every ABI mismatch, every string byte-count error, every struct layout divergence between IR and C, every bootstrap stage regression hits directly with no safety net.

**Culebra is the safety net.**

It exists because Mapanare needed it to survive its own bootstrap. It turns out every compiler project that targets LLVM needs the same thing, but nobody packaged it before.

> The name: *Mapanare* is a Venezuelan pit viper. *Culebra* is the common snake. Same family, different role. Mapanare is the language, Culebra is the utility tool any compiler developer can pick up.

---

## En español

Culebra es una herramienta de diagnóstico para compiladores que generan LLVM IR. Nació del proyecto Mapanare, un lenguaje de programación compilado creado en Venezuela. Si estás construyendo un lenguaje, auto-hospedando un compilador, o peleando con ABI y convenciones de llamada, Culebra te ayuda a encontrar los bugs que nadie más detecta.

🇻🇪 *Hecho con orgullo venezolano*

---

## Quick Start

### Install

```bash
cargo install --git https://github.com/Mapanare-Research/Culebra
```

Or build from source:

```bash
git clone https://github.com/Mapanare-Research/Culebra.git
cd Culebra
cargo build --release
# Binary at target/release/culebra
```

### Real workflow

You just emitted `stage2.ll` from your compiler and something is wrong at runtime. Here's how you hunt it down:

```bash
# 1. Is the IR even valid?
culebra check stage2.ll

# 2. Are string constants correct? (catches byte-count mismatches from escape sequences)
culebra strings stage2.ll

# 3. Any known pathologies? (empty switch, ret mismatch, alloca aliasing)
culebra audit stage2.ll

# 4. ABI problems? (sret/byref misuse, large structs returned by value)
culebra abi stage2.ll

# 5. What changed between stage1 and stage2?
culebra diff stage1.ll stage2.ll

# 6. Drill into one function
culebra extract stage2.ll my_broken_function

# 7. Inspect the compiled binary's .rodata for off-by-1 string pointers
culebra binary ./my_compiler --ir stage2.ll --find "hello world"

# 8. Run the full pipeline end-to-end
culebra pipeline
```

---

## Why Culebra?

These are real bugs that Culebra catches. Every one of them has wasted hours of debugging time in real compiler development.

### String byte-count mismatch

Your escape-sequence handler emits `\n` as two bytes instead of one but the `[N x i8]` type says `N`. The IR assembles, the binary links, and the string silently contains garbage at the end.

```bash
$ culebra strings stage2.ll
ERROR: @.str.47 declared [14 x i8] but content is 13 bytes
  → c"Hello, world!\00"
  Fix: change to [13 x i8]
```

### PHI fix script deleting all functions

You wrote a Python script to fix broken PHI nodes. It works on small files. On your full compiler IR, it silently outputs an empty module.

```bash
$ culebra phi-check stage2.ll --fix-cmd "python3 scripts/fix_phis.py -"
ERROR: transform deleted 47 of 47 functions
  → Input had 47 functions, output has 0
```

### ABI struct layout mismatch

Your IR passes a struct by value. The C runtime expects it via `sret` pointer. It compiles, links, and segfaults at runtime.

```bash
$ culebra abi stage2.ll
WARNING: @create_string returns {i8*, i64} (16 bytes) by value
  → Consider sret for structs > 8 bytes on x86_64
```

### Bootstrap stage divergence

Stage 2 and Stage 3 should produce identical output (fixed-point). They don't, and you can't tell where the divergence started.

```bash
$ culebra diff stage2.ll stage3.ll
DIVERGED: 3 functions differ
  → @emit_call: 47 vs 52 instructions
  → @type_check: register allocation differs
  → @codegen_if: missing branch in stage3
```

---

## Layers

Culebra organizes diagnostics into layers, from low-level IR analysis up to full bootstrap orchestration:

| Layer | Commands | What it covers |
|---|---|---|
| **IR** | `strings`, `audit`, `check`, `diff`, `extract`, `table` | Byte-level IR validation, pathology detection, structural comparison |
| **ABI** | `abi` | Calling convention mismatches, sret/byref analysis, struct layout |
| **Binary** | `binary` | ELF/PE inspection, .rodata cross-referencing against IR |
| **Pipeline** | `phi-check`, `pipeline`, `fixedpoint` | Transform validation, stage orchestration, convergence detection |
| **Runtime** | `run`, `test` | Compile-and-run, expected-output diffing |
| **Bootstrap** | `status` | Self-hosting progress tracking |
| **Config** | `init`, `watch` | Project setup, file-watching |

---

## All Commands

| Command | What it does |
|---|---|
| `culebra strings file.ll` | Validates every `[N x i8] c"..."` constant. Catches byte-count mismatches from escape-sequence miscounting. |
| `culebra audit file.ll` | Detects IR pathologies: empty switch, ret mismatch, alloca alias, and more. |
| `culebra check file.ll` | Runs `llvm-as` validation with structured error output. |
| `culebra phi-check file.ll` | Validates that transform scripts don't destroy the IR. |
| `culebra diff a.ll b.ll` | Per-function structural diff, register-normalized. |
| `culebra extract file.ll fn` | Dumps a single function from a massive IR file. |
| `culebra table file.ll` | Per-function metrics table (instructions, allocas, calls, etc.). |
| `culebra abi file.ll` | Detects sret/byref misuse, flags large-return-by-value. |
| `culebra binary ./binary` | ELF/PE string inspection, .rodata analysis, optional IR cross-referencing. |
| `culebra run compiler source.mn` | Compiles a source file, runs the binary, checks expected output. |
| `culebra test` | Runs all `[[tests]]` from `culebra.toml`. Compile, execute, diff output. |
| `culebra watch` | Watches files and re-runs a command on change. |
| `culebra pipeline` | Builds and tests a full stage pipeline end-to-end via `culebra.toml`. |
| `culebra fixedpoint compiler source` | Detects fixed-point: runs stage N output through itself, checks if output stabilizes. |
| `culebra status` | Shows self-hosting progress from `culebra.toml`. |
| `culebra init` | Generates a `culebra.toml` template for your project. |

---

## Configuration: `culebra.toml`

Run `culebra init` to generate a starter config. Here's what it looks like:

```toml
# Culebra config
# https://github.com/Mapanare-Research/Culebra

[project]
name = "my-compiler"
source_lang = "my-lang"
target = "llvm"                    # llvm, wasm, native
compiler = "./my-compiler"         # path to compiler binary (used by `culebra test`)
runtime = "runtime/my_runtime.c"   # C runtime to link (optional)

# Bootstrap stages.
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

# Runtime tests: `culebra test` compiles each, runs it, checks stdout
[[tests]]
name = "hello"
source = 'fn main() { print("hello") }'
expect = "hello"

[[tests]]
name = "math"
source = "fn main() { print(2 + 3) }"
expect = "5"
```

---

## What stays in your project

Culebra handles generic LLVM IR analysis, the kind of diagnostics every compiler project needs. It does not try to replace your project-specific tooling.

**Culebra handles:**
- IR validation and pathology detection
- ABI and calling convention analysis
- Binary inspection and string cross-referencing
- Bootstrap stage orchestration and fixed-point detection
- Transform script validation

**Your project keeps:**
- Golden test suites specific to your language
- Custom build scripts and Makefiles
- Language-specific semantic checks
- Project-specific CI/CD pipelines

Culebra slots into your existing workflow. It reads `.ll` files and `culebra.toml`, it doesn't try to own your build system.

---

## Built for

- Anyone building a language that targets LLVM IR
- Anyone self-hosting a compiler
- Anyone debugging ABI and calling convention issues between IR and native code
- Anyone running a multi-stage bootstrap and needing to know where divergence starts
- Anyone who's lost hours to a string byte-count being off by one

---

## License

MIT. See [LICENSE](LICENSE) for details.

---

<p align="center">
  Born from <a href="https://github.com/Mapanare-Research">Mapanare</a>, the language that needed a safety net to survive its own bootstrap.
</p>
