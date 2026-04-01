<div align="center">

# Culebra

**/koo-LEH-brah/**

**Compiler diagnostics for self-hosting languages that target LLVM.**

*ABI. IR. Binary. Bootstrap. One binary catches what no debugger will.*

Born from [Mapanare](https://github.com/Mapanare-Research/Mapanare)'s bootstrap, where every bug was a mystery with no safety net. Culebra ships a Nuclei-style template engine so every compiler bug you survive becomes a pattern nobody else has to debug.

English | [Espanol](docs/README.es.md) | [中文版](docs/README.zh-CN.md) | [Portugues](docs/README.pt.md)

<br>

![Rust](https://img.shields.io/badge/Rust-2021_Edition-dea584?style=for-the-badge&logo=rust&logoColor=white)
![LLVM](https://img.shields.io/badge/LLVM-IR_Analysis-262D3A?style=for-the-badge&logo=llvm&logoColor=white)
![Platform](https://img.shields.io/badge/Linux%20%7C%20macOS%20%7C%20Windows-grey?style=for-the-badge)

[![License](https://img.shields.io/badge/license-MIT-green.svg?style=flat-square)](LICENSE)
[![Version](https://img.shields.io/badge/version-1.1.0-blue.svg?style=flat-square)](Cargo.toml)
[![Templates](https://img.shields.io/badge/templates-29_shipped-orange.svg?style=flat-square)]()
[![GitHub Stars](https://img.shields.io/github/stars/Mapanare-Research/Culebra?style=flat-square&color=f5c542)](https://github.com/Mapanare-Research/Culebra/stargazers)

<br>

[Why Culebra?](#why-culebra) · [Install](#install) · [Quick Start](#quick-start) · [Template Engine](#template-engine) · [All Commands](#all-commands) · [Shipped Templates](#shipped-templates) · [Configuration](#configuration-culebratoml) · [Architecture](#architecture) · [Full Docs](docs.md) · [Contributing](#contributing)

</div>

---

## Why Culebra?

Most languages bootstrapped on top of a mature compiler:

- **Rust** started in OCaml before self-hosting about a year later.
- **Go** was written in C until v1.5, then used an automated C-to-Go translator.
- **C++** bootstrapped through Cfront, which translated C++ to C and let C compilers handle code generation.

[Mapanare](https://github.com/Mapanare-Research/Mapanare) doesn't have that luxury. It's an AI-native compiled language targeting LLVM IR, building its own backend from scratch: lexer, AST, type inference, LLVM IR emission. The bootstrap compiler (Stage 0) is written in Python, but there's no mature compiler underneath to fall back on.

That means every ABI mismatch, every string byte-count error, every struct layout divergence between IR and C, every bootstrap stage regression hits directly with no safety net.

**Culebra is the safety net.**

It exists because Mapanare needed it to survive its own bootstrap. It turns out every compiler project that targets LLVM needs the same thing, but nobody packaged it before.

We didn't just build a linter. We built a pattern engine. Every compiler bug we survived became a template so nobody else has to.

> The name: *Mapanare* is a Venezuelan pit viper. *Culebra* is the common snake. Same family, different role. Mapanare is the language, Culebra is the utility tool any compiler developer can pick up.

---

## Install

### Linux / macOS

```bash
cargo install --git https://github.com/Mapanare-Research/Culebra
```

### Windows

```powershell
cargo install --git https://github.com/Mapanare-Research/Culebra
```

### From source

```bash
git clone https://github.com/Mapanare-Research/Culebra.git
cd Culebra
cargo build --release
# Binary at target/release/culebra
```

Verify:

```bash
culebra --version
```

---

## Quick Start

You just emitted `stage2.ll` from your compiler and something is wrong at runtime. Here's how you hunt it down:

```bash
# 1. Scan for all known bug patterns at once
culebra scan stage2.ll

# 2. Focus on critical ABI bugs only
culebra scan stage2.ll --tags abi --severity critical

# 3. Auto-fix what can be fixed
culebra scan stage2.ll --autofix --dry-run   # preview
culebra scan stage2.ll --autofix             # apply

# 4. Is the IR even valid?
culebra check stage2.ll

# 5. Are string constants correct?
culebra strings stage2.ll

# 6. Any known pathologies?
culebra audit stage2.ll

# 7. What changed between stage1 and stage2?
culebra diff stage1.ll stage2.ll

# 8. Drill into one function
culebra extract stage2.ll my_broken_function

# 9. Cross-reference struct layouts against C runtime
culebra abi stage2.ll --header runtime/mapanare_core.c

# 10. Inspect the compiled binary's .rodata
culebra binary ./my_compiler --ir stage2.ll --find "hello world"

# 11. Run the full pipeline end-to-end
culebra pipeline
```

---

## Real bugs Culebra catches

These are real bugs from Mapanare's bootstrap. Every one wasted hours of debugging.

### Unaligned string constant (the bootstrap killer)

String constants without `align 2` land at odd addresses. Pointer tagging shifts the pointer by -1 byte. Every string comparison fails silently. Tokenizer produces 0 tokens. Compiler outputs empty IR. No crash, no error.

```bash
$ culebra scan stage2.ll --id unaligned-string-constant
CRITICAL [unaligned-string-constant] String constant missing alignment -- stage2.ll:47
  @.str.0 is a 6-byte string constant without alignment.
  If your runtime uses pointer bit-tagging, this constant's
  pointer will be silently shifted by -1 byte.
  fix: Add ', align 2' to all [N x i8] constant declarations
```

### String byte-count mismatch

Your escape-sequence handler emits `\n` as two bytes instead of one but the `[N x i8]` type says `N`. The IR assembles, the binary links, and the string silently contains garbage.

```bash
$ culebra strings stage2.ll
ERROR: @.str.47 declared [14 x i8] but content is 13 bytes
  -> c"Hello, world!\00"
  Fix: change to [13 x i8]
```

### List push without writeback (alias analysis trap)

Pushing to a list via GEP directly into a struct field. LLVM caches the pre-push struct state. The mutation is lost. Stage 1 works, stage 2 accumulates 0 lines.

```bash
$ culebra scan stage2.ll --id direct-push-no-writeback
HIGH [direct-push-no-writeback] List push without temp writeback -- stage2.ll:142 (in emit_line)
  List push at struct field 2 goes directly through GEP without
  temp alloca + writeback. LLVM may optimize away the mutation at -O1+.
```

### ABI struct layout mismatch

Your IR passes a struct by value. The C runtime expects it via `sret` pointer. It compiles, links, and segfaults at runtime.

```bash
$ culebra abi stage2.ll --header runtime/mapanare_core.c
WARNING: @create_string returns {i8*, i64} (16 bytes) by value
  -> Consider sret for structs > 8 bytes on x86_64
```

### Bootstrap stage divergence

Stage 2 and Stage 3 should produce identical output (fixed-point). They don't, and you can't tell where the divergence started.

```bash
$ culebra diff stage2.ll stage3.ll
DIVERGED: 3 functions differ
  -> @emit_call: 47 vs 52 instructions
  -> @type_check: register allocation differs
  -> @codegen_if: missing branch in stage3
```

---

## Template Engine

Culebra ships a Nuclei-style pattern engine. Bug patterns are YAML templates. The Rust binary is the engine. The templates are the knowledge base.

Every template in the initial pack comes from a real bug hit during Mapanare's bootstrap. Not hypothetical patterns -- documented battlefield scars with commit references, impact descriptions, and proven remediations.

### Scan

```bash
# Run all templates
culebra scan file.ll

# Filter by tag, severity, or specific template
culebra scan file.ll --tags abi,string
culebra scan file.ll --severity critical,high
culebra scan file.ll --id unaligned-string-constant

# Cross-file ABI check
culebra scan file.ll --header runtime.c

# Auto-fix
culebra scan file.ll --autofix --dry-run
culebra scan file.ll --autofix

# Custom template
culebra scan file.ll --template ./my-check.yaml

# Output formats
culebra scan file.ll --format json
culebra scan file.ll --format sarif     # GitHub Code Scanning
culebra scan file.ll --format markdown  # CI reports
```

### Browse templates

```bash
culebra templates list
culebra templates list --tags abi
culebra templates show unaligned-string-constant
```

### Run workflows

Workflows chain templates with stop conditions for multi-step validation:

```bash
culebra workflow bootstrap-health-check \
  --input stage1_output=stage1.ll

culebra workflow pre-commit \
  --input ir_file=main.ll

culebra workflow ci-full \
  --input ir_file=main.ll --format sarif
```

### Write your own templates

Templates are YAML files in `culebra-templates/`. A minimal example:

```yaml
id: my-custom-check
info:
  name: My custom check
  severity: high
  author: yourname
  description: Catches a specific bug pattern.
  tags:
    - ir
    - custom

scope:
  file_type: llvm-ir
  section: functions

match:
  matchers:
    - type: regex
      name: pattern_name
      pattern:
        - 'some regex pattern'
  condition: or

remediation:
  suggestion: "How to fix this"
```

Anyone building a language targeting LLVM can open a PR adding their own bug template. The engine never changes, the knowledge base grows. Same model that made Nuclei dominant in security scanning.

See [docs.md](docs.md) for the full template specification, matcher types (regex, sequence, cross-reference, byte scanner), extractors, autofix, and workflow definitions.

---

## Shipped Templates

29 templates across 4 categories, every one from a real Mapanare bug.

| Category | ID | Severity | What it catches |
|---|---|---|---|
| **ABI** | `unaligned-string-constant` | Critical | String constants at odd addresses corrupt pointer tagging |
| **ABI** | `struct-layout-mismatch` | Critical | IR struct vs C header field count/type divergence |
| **ABI** | `return-type-divergence` | Critical | Runtime function return type differs between stages (e.g., ptr vs {i64, i64}) |
| **ABI** | `direct-push-no-writeback` | High | List push through GEP without temp alloca writeback |
| **ABI** | `sret-input-output-alias` | High | sret pointer aliasing input corrupts data mid-computation |
| **ABI** | `tagged-pointer-odd-address` | High | Odd-sized constants without alignment break pointer tagging |
| **ABI** | `missing-byval-large-struct` | Medium | Large structs passed as bare ptr without byval |
| **ABI** | `large-struct-by-value` | High | Structs >56 bytes passed by value via load/store instead of sret/memcpy |
| **ABI** | `list-element-size-undercount` | High | `__mn_list_new(N)` with N smaller than actual element struct |
| **IR** | `empty-switch-body` | Critical | Switch with 0 cases -- match arms not generated |
| **IR** | `break-inside-nested-control` | Critical | Break inside if-inside-for dropped — infinite loop |
| **IR** | `option-type-pun-zeroinit` | Critical | Option discriminant clobbered by inner type store over zeroinitializer |
| **IR** | `ret-type-mismatch` | Critical | Return type doesn't match function signature |
| **IR** | `byte-count-mismatch` | High | `[N x i8]` declared size vs actual content differs |
| **IR** | `phi-predecessor-mismatch` | High | PHI node references non-existent predecessor block |
| **IR** | `internal-linkage-dce` | High | Internal-linkage functions stripped by LLVM -O1 optimizer |
| **IR** | `dynamic-alloca-non-entry` | High | Allocas in non-entry blocks misalign RSP, crash libc SSE calls |
| **IR** | `return-inside-nested-block` | High | Return inside match/if doesn't terminate -- execution falls through |
| **IR** | `phi-operand-type-mismatch` | High | PHI operand type differs from declared type (dead if_result PHIs) |
| **IR** | `raw-control-byte-in-constant` | Medium | Raw control bytes in c"..." break line-based tooling |
| **IR** | `unreachable-after-branch` | Medium | Instructions after terminator (dead code) |
| **IR** | `dropped-else-branch` | Medium | if_then without corresponding else block -- branch silently dropped |
| **Binary** | `missing-symbol` | Critical | Runtime symbol missing from binary symbol table |
| **Binary** | `odd-address-rodata` | High | String at odd address in .rodata section |
| **Bootstrap** | `function-count-drop` | Critical | Stage N+1 has fewer functions than Stage N |
| **Bootstrap** | `stage-output-divergence` | High | Stage output doesn't converge toward fixed-point |
| **Bootstrap** | `fixed-point-delta` | High | Compiler output doesn't stabilize after N iterations |
| **Bootstrap** | `call-count-divergence` | High | Function calls runtime helper fewer times than stage1 (branches dropped) |
| **Bootstrap** | `body-size-shrinkage` | High | Function body drastically smaller in self-compiled output |

4 shipped workflows: `bootstrap-health-check`, `pre-commit`, `ci-full`, `playground-mapanare`.

---

## All Commands

| Command | What it does |
|---|---|
| `culebra scan file.ll` | Scan IR with YAML pattern templates. `--tags`, `--severity`, `--id`, `--format`, `--autofix`. |
| `culebra templates list` | List all available scan templates with severity and tags. |
| `culebra templates show <id>` | Show full details of a template: description, impact, remediation, CWE. |
| `culebra workflow <id>` | Run a multi-step scan workflow with stop conditions. |
| `culebra strings file.ll` | Validate `[N x i8] c"..."` byte counts. Catches escape-sequence miscounting. |
| `culebra audit file.ll` | Detect IR pathologies: empty switch, ret mismatch, missing `%`, duplicate case. |
| `culebra check file.ll` | Validate IR with `llvm-as`. |
| `culebra phi-check file.ll` | Validate transform scripts preserve IR structure. |
| `culebra diff a.ll b.ll` | Per-function structural diff, register-normalized. |
| `culebra extract file.ll fn` | Extract a single function from a massive IR file. |
| `culebra table file.ll` | Per-function metrics table (instructions, allocas, calls, etc.). |
| `culebra abi file.ll` | Detect sret/byref misuse, struct layout validation, C header cross-ref. |
| `culebra binary ./binary` | ELF/PE inspection, .rodata analysis, IR cross-referencing. |
| `culebra run compiler source` | Compile, run, check expected output. |
| `culebra test` | Run all `[[tests]]` from `culebra.toml`. |
| `culebra watch` | Watch files, re-run a command on change. |
| `culebra pipeline` | Run full stage pipeline end-to-end via `culebra.toml`. |
| `culebra triage file.ll` | Group findings by root cause, deduplicate, show actionable summary. `--format json` for AI. |
| `culebra compare a.ll b.ll` | Per-function metric comparison. `--metric calls/blocks/pushes`, `--threshold 0.3`. |
| `culebra explain file.ll <id>` | Show matched IR in context with template description + remediation. `--function <name>`. |
| `culebra bisect a.ll b.ll` | Find divergent functions between stages, ranked by impact (callers * delta). |
| `culebra verify file.ll <id>` | Verify a specific fix — re-scan one template, PASS/FAIL output. `--function <name>`. |
| `culebra fixedpoint compiler source` | Detect fixed-point convergence in self-hosting compilers. |
| `culebra status` | Show bootstrap self-hosting progress. |
| `culebra init` | Generate a `culebra.toml` template. |

---

## Layers

| Layer | Commands | What it covers |
|---|---|---|
| **Scan** | `scan`, `templates`, `workflow` | Template-driven pattern matching, autofix, SARIF output |
| **IR** | `strings`, `audit`, `check`, `diff`, `extract`, `table` | Byte-level IR validation, pathology detection, structural comparison |
| **ABI** | `abi` | Calling convention mismatches, sret/byref analysis, struct layout |
| **Binary** | `binary` | ELF/PE inspection, .rodata cross-referencing against IR |
| **Pipeline** | `phi-check`, `pipeline`, `fixedpoint` | Transform validation, stage orchestration, convergence detection |
| **Runtime** | `run`, `test` | Compile-and-run, expected-output diffing |
| **Bootstrap** | `status` | Self-hosting progress tracking |
| **Config** | `init`, `watch` | Project setup, file-watching |

---

## Architecture

```
                        culebra scan file.ll --tags abi
                                    |
                    +---------------+---------------+
                    |                               |
             Template Loader                   IR Parser
          (culebra-templates/)               (ir.rs -> IRModule)
                    |                               |
             Filter by tags,              Parse functions, globals,
            severity, id                  string constants, structs
                    |                               |
                    +----------- Engine ------------+
                                    |
                    +---------------+---------------+
                    |               |               |
              Regex Matcher   Sequence Matcher  Cross-Ref Matcher
             (single-line)   (multi-line with   (IR vs C header)
                             captures, absence)
                    |               |               |
                    +----------- Findings ----------+
                                    |
                    +---------------+---------------+
                    |               |               |
                  Text           JSON            SARIF
                (colored)    (structured)    (GitHub Code
                                             Scanning)
```

**Template directory resolution:**

1. `./culebra-templates/` (project-local)
2. `<binary_dir>/culebra-templates/` (next to binary)
3. `~/.culebra/templates/` (user-global)

**Template directory structure:**

```
culebra-templates/
  abi/
    unaligned-string-constant.yaml
    direct-push-no-writeback.yaml
    sret-input-output-alias.yaml
    missing-byval-large-struct.yaml
    tagged-pointer-odd-address.yaml
    struct-layout-mismatch.yaml
    return-type-divergence.yaml        # NEW — playground
    large-struct-by-value.yaml         # NEW — playground
    list-element-size-undercount.yaml  # NEW — playground
  ir/
    byte-count-mismatch.yaml
    empty-switch-body.yaml
    ret-type-mismatch.yaml
    raw-control-byte.yaml
    phi-predecessor-mismatch.yaml
    unreachable-after-branch.yaml
    dropped-else-branch.yaml           # NEW — playground
    option-type-pun-zeroinit.yaml      # NEW — playground
    internal-linkage-dce.yaml          # NEW — playground
    dynamic-alloca-non-entry.yaml      # NEW — playground
    return-inside-nested-block.yaml    # NEW — playground
    phi-operand-type-mismatch.yaml     # NEW — playground
    break-inside-nested-control.yaml   # NEW — playground
  binary/
    odd-address-rodata.yaml
    missing-symbol.yaml
  bootstrap/
    stage-output-divergence.yaml
    function-count-drop.yaml
    fixed-point-delta.yaml
    call-count-divergence.yaml         # NEW — v2.2.0 playground
    body-size-shrinkage.yaml           # NEW — v2.2.0 playground
  workflows/
    bootstrap-health-check.yaml
    pre-commit.yaml
    ci-full.yaml
    playground-mapanare.yaml           # NEW — v2.2.0 playground
```

---

## Configuration: `culebra.toml`

Run `culebra init` to generate a starter config:

```toml
[project]
name = "my-compiler"
source_lang = "my-lang"
target = "llvm"
compiler = "./my-compiler"
runtime = "runtime/my_runtime.c"

[[stages]]
name = "bootstrap"
cmd = "python bootstrap/compile.py {input}"
input = "src/compiler.ml"
output = "/tmp/stage1.ll"
validate = true

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

## CI/CD Integration

### GitHub Actions with SARIF

```yaml
name: Culebra Scan
on: [push, pull_request]

jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Culebra
        run: cargo install --git https://github.com/Mapanare-Research/Culebra

      - name: Run scan
        run: culebra scan output.ll --format sarif > culebra.sarif

      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: culebra.sarif
```

### Pre-commit hook

```bash
#!/bin/bash
culebra scan output.ll --severity critical,high
```

---

## Built for

- Anyone building a language that targets LLVM IR
- Anyone self-hosting a compiler
- Anyone debugging ABI and calling convention issues between IR and native code
- Anyone running a multi-stage bootstrap and needing to know where divergence starts
- Anyone who's lost hours to a string byte-count being off by one
- Anyone who wants their hard-won compiler bugs turned into reusable detection templates

---

## Contributing

Contributions welcome. Two ways to contribute:

1. **Code** -- Rust engine improvements, new matcher types, output formats
2. **Templates** -- Add YAML templates for compiler bugs you've encountered

Every bug you've hit with your LLVM-targeting compiler can become a template. The tool gets smarter without touching Rust code.

---

## License

MIT License -- see [LICENSE](LICENSE) for details.

---

<div align="center">

**Culebra** -- The safety net your compiler needs.

[Full Documentation](docs.md) · [Report Bug](https://github.com/Mapanare-Research/Culebra/issues) · [Mapanare](https://github.com/Mapanare-Research/Mapanare)

Made with care by [Juan Denis](https://juandenis.com)

</div>
