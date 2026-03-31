# Culebra Documentation

Complete reference for Culebra -- compiler diagnostics and template-driven pattern scanning for LLVM IR.

---

## Table of Contents

- [Installation](#installation)
- [Commands Reference](#commands-reference)
  - [scan](#scan)
  - [templates](#templates)
  - [workflow](#workflow)
  - [strings](#strings)
  - [audit](#audit)
  - [check](#check)
  - [phi-check](#phi-check)
  - [diff](#diff)
  - [extract](#extract)
  - [table](#table)
  - [abi](#abi)
  - [binary](#binary)
  - [run](#run)
  - [test](#test)
  - [watch](#watch)
  - [pipeline](#pipeline)
  - [fixedpoint](#fixedpoint)
  - [status](#status)
  - [init](#init)
- [Template Engine](#template-engine)
  - [How it works](#how-it-works)
  - [Template specification](#template-specification)
  - [Matcher types](#matcher-types)
  - [Extractors](#extractors)
  - [Autofix](#autofix)
  - [Writing custom templates](#writing-custom-templates)
- [Workflows](#workflows)
  - [Workflow specification](#workflow-specification)
  - [Shipped workflows](#shipped-workflows)
- [Shipped Templates](#shipped-templates)
  - [ABI templates](#abi-templates)
  - [IR templates](#ir-templates)
  - [Binary templates](#binary-templates)
  - [Bootstrap templates](#bootstrap-templates)
- [Output Formats](#output-formats)
- [Configuration: culebra.toml](#configuration-culebratoml)
- [Template Directory](#template-directory)
- [Exit Codes](#exit-codes)
- [CI/CD Integration](#cicd-integration)

---

## Installation

### From crates.io (when published)

```bash
cargo install culebra
```

### From GitHub

```bash
cargo install --git https://github.com/Mapanare-Research/Culebra
```

### From source

```bash
git clone https://github.com/Mapanare-Research/Culebra.git
cd Culebra
cargo build --release
# Binary at target/release/culebra
```

### WSL / Linux

```bash
# Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Build and install
cd /path/to/Culebra
cargo install --path .
```

---

## Commands Reference

### scan

Scan IR files with YAML pattern templates. This is the Nuclei-style template engine.

```
culebra scan <FILE> [OPTIONS]
```

**Arguments:**
- `<FILE>` -- Path to `.ll` file

**Options:**
| Flag | Description |
|---|---|
| `--tags <TAGS>` | Filter by tags, comma-separated (e.g., `abi,string`) |
| `--severity <SEVERITY>` | Filter by severity, comma-separated (`critical,high,medium,low,info`) |
| `--id <ID>` | Run a specific template by ID |
| `--template <PATH>` | Path to custom template file or directory |
| `--header <PATH>` | Path to C header for cross-reference checks |
| `--format <FORMAT>` | Output format: `text` (default), `json`, `sarif`, `markdown` |
| `--autofix` | Apply auto-fixes from matching templates |
| `--dry-run` | Show fixes without applying (use with `--autofix`) |

**Examples:**

```bash
# Run all templates against an IR file
culebra scan stage2.ll

# Only ABI and string-related checks
culebra scan stage2.ll --tags abi,string

# Only critical severity
culebra scan stage2.ll --severity critical

# Run one specific template
culebra scan stage2.ll --id unaligned-string-constant

# Cross-reference IR struct layouts against C header
culebra scan stage2.ll --header runtime/mapanare_core.c

# Output as JSON for programmatic consumption
culebra scan stage2.ll --format json

# Output as SARIF for GitHub Code Scanning
culebra scan stage2.ll --format sarif > results.sarif

# Preview auto-fixes without applying
culebra scan stage2.ll --autofix --dry-run

# Apply auto-fixes
culebra scan stage2.ll --autofix

# Use a custom template directory
culebra scan stage2.ll --template ./my-templates/
```

**Exit codes:**
- `0` -- No critical or high findings
- `1` -- One or more critical or high findings detected

---

### templates

Browse and inspect available scan templates.

#### templates list

```bash
culebra templates list [--tags <TAGS>]
```

Lists all templates with their ID, severity, name, and tags.

```bash
# List all
culebra templates list

# Filter by tag
culebra templates list --tags abi
culebra templates list --tags bootstrap
```

**Example output:**

```
culebra 17 templates in culebra-templates

  ID                             SEVERITY   NAME                                     TAGS
  ----------------------------------------------------------------------------------------------------
  empty-switch-body              critical   Switch statement with zero cases         ir, control-flow
  unaligned-string-constant      critical   String constant missing alignment        abi, string
  direct-push-no-writeback       high       List push without temp writeback         abi, sret
  ...
```

#### templates show

```bash
culebra templates show <ID>
```

Shows full details of a template: description, impact, remediation steps, CWE reference, related templates.

```bash
culebra templates show unaligned-string-constant
```

**Example output:**

```
Template: unaligned-string-constant

  Name:           String constant missing alignment
  Severity:       critical
  Author:         juandenis
  CWE:            CWE-188
  Tags:           abi, string, bootstrap-killer, silent-failure

  Description:
    String constants without align 2+ can land at odd addresses.
    Runtimes using pointer tagging will mangle these pointers,
    causing silent string comparison failures.

  Impact:
    All string comparisons return false. Tokenizer produces 0 tokens.
    No crash, no error, just silent empty output.

  Remediation:
    Add ', align 2' to all [N x i8] constant declarations
    >> auto-fixable (line_replace)
    Difficulty: trivial

  Related:
    - raw-control-byte-in-constant
    - tagged-pointer-odd-address
```

---

### workflow

Run a multi-step scan workflow. Workflows chain templates with stop conditions.

```
culebra workflow <WORKFLOW_ID> [OPTIONS]
```

**Arguments:**
- `<WORKFLOW_ID>` -- Workflow ID to run

**Options:**
| Flag | Description |
|---|---|
| `--input <KEY=VALUE>` | Input files as key=value pairs (repeatable) |
| `--format <FORMAT>` | Output format: `text`, `json`, `sarif`, `markdown` |

**Examples:**

```bash
# Run bootstrap health check
culebra workflow bootstrap-health-check \
  --input stage1_output=stage1.ll

# Pre-commit validation
culebra workflow pre-commit \
  --input ir_file=main.ll

# Full CI scan with SARIF output
culebra workflow ci-full \
  --input ir_file=main.ll \
  --format sarif > results.sarif
```

---

### strings

Validate `[N x i8] c"..."` string constant byte counts in LLVM IR.

```
culebra strings <FILE> [--verbose] [--json]
```

Catches byte-count mismatches caused by escape-sequence miscounting. The declared `[N x i8]` says N bytes, but the actual `c"..."` content may have a different count when `\xx` hex escapes are properly counted.

**Options:**
| Flag | Description |
|---|---|
| `--verbose` / `-v` | Show duplicate string details |
| `--json` | Output as JSON |

---

### audit

Detect known IR pathologies.

```
culebra audit <FILE> [--only <FILTER>] [--baseline <FILE>]
```

Built-in detectors:
- **EMPTY_SWITCH** -- `switch` with 0 cases (match arms not generated)
- **RET_TYPE_MISMATCH** -- `ret` type doesn't match function signature
- **MISSING_PERCENT** -- Bare identifiers missing `%` prefix
- **DUPLICATE_CASE** -- Duplicate values in switch case list

**Options:**
| Flag | Description |
|---|---|
| `--only <FILTER>` | Filter functions by substring |
| `--baseline <FILE>` | Save baseline for delta tracking |

---

### check

Validate IR with `llvm-as`.

```
culebra check <FILE>
```

Runs `llvm-as` (auto-detects versions 15-18) and reports structured errors. Returns 0 if valid, 1 if invalid.

---

### phi-check

Validate that a transform script preserves IR structure.

```
culebra phi-check <FILE> [--fix-cmd <CMD>]
```

Runs the transform command, then compares before/after: function counts, line counts, structural changes. Catches scripts that silently delete functions or produce empty output.

**Options:**
| Flag | Description |
|---|---|
| `--fix-cmd <CMD>` | Transform command (default: `python3 scripts/fix_stage2_phis.py -`) |

---

### diff

Per-function structural diff between two IR files.

```
culebra diff <FILE_A> <FILE_B> [--verbose]
```

Uses register-normalized structural hashing to compare functions. Reports:
- **Matched** -- Same structure (ignoring register names)
- **Diverged** -- Different structure with metric deltas
- **Only in A / Only in B** -- Functions present in one file but not the other

**Options:**
| Flag | Description |
|---|---|
| `--verbose` / `-v` | Show per-instruction diffs for divergent functions |

---

### extract

Extract one function's IR from a file.

```
culebra extract <FILE> <FUNC_NAME>
```

Searches by exact name or substring match. Outputs the full function definition with metrics (instructions, allocas, calls, etc.).

---

### table

Per-function metrics table.

```
culebra table <FILE> [--top <N>] [--sort-by <METRIC>]
```

Displays a table of per-function metrics. Useful for finding the most complex functions.

**Options:**
| Flag | Description |
|---|---|
| `--top <N>` | Show top N functions only |
| `--sort-by <METRIC>` | Sort column: `instructions` (default), `allocas`, `calls`, `blocks`, `stores`, `loads`, `geps` |

---

### abi

Validate struct layouts and calling conventions.

```
culebra abi <FILE> [--header <C_HEADER>]
```

Detects:
- Structs larger than 16 bytes returned by value (should use sret)
- sret/byref attribute misuse
- Field count/type mismatches between IR `%struct.*` types and C `typedef struct`

**Options:**
| Flag | Description |
|---|---|
| `--header <PATH>` | Path to C header or source file for cross-reference |

---

### binary

Inspect compiled binary and cross-reference against IR.

```
culebra binary <FILE> [--ir <IR_FILE>] [--find <STRING>]
```

Parses ELF or PE binaries using the `goblin` crate. Extracts readable strings from `.rodata`, inspects the symbol table, and optionally cross-references string addresses against IR GEP targets.

**Options:**
| Flag | Description |
|---|---|
| `--ir <PATH>` | Path to `.ll` file for cross-referencing GEP targets |
| `--find <STRING>` | Verify a specific string exists at correct address |

---

### run

Compile and run a single test.

```
culebra run <COMPILER> <SOURCE> [OPTIONS]
```

Compiles a source file through the specified compiler, links with clang, runs the binary, and optionally checks stdout against expected output.

**Options:**
| Flag | Description |
|---|---|
| `--expect <OUTPUT>` | Expected stdout (fail if different) |
| `--timeout <SECS>` | Timeout in seconds (default: 30) |
| `--clang-flags <FLAGS>` | Extra flags passed to clang when linking |
| `--runtime <PATH>` | Path to C runtime to link |

---

### test

Run all tests from `culebra.toml`.

```
culebra test [--config <FILE>] [--filter <NAME>] [--timeout <SECS>]
```

Reads `[[tests]]` entries from the config, compiles each source, runs the binary, and diffs stdout against expected output.

**Options:**
| Flag | Description |
|---|---|
| `--config` / `-c` | Config file (default: `culebra.toml`) |
| `--filter` | Filter tests by name substring |
| `--timeout` | Timeout per test in seconds (default: 30) |

---

### watch

Watch files and re-run a command on change.

```
culebra watch [--patterns <GLOBS>] [--dir <DIR>] <CMD...>
```

Uses filesystem notifications (inotify on Linux, ReadDirectoryChanges on Windows) to detect changes and re-run the specified command.

**Options:**
| Flag | Description |
|---|---|
| `--patterns` / `-p` | Glob patterns to watch, comma-separated (default: `*.ll,*.mn`) |
| `--dir` / `-d` | Directory to watch (default: `.`) |

---

### pipeline

Run a full bootstrap stage pipeline.

```
culebra pipeline [--config <FILE>] [--timeout <SECS>]
```

Reads `[[stages]]` from `culebra.toml` and runs them sequentially. Each stage's command can reference `{input}`, `{output}`, and `{prev_output}`. Optionally validates each stage's output with `llvm-as`.

**Options:**
| Flag | Description |
|---|---|
| `--config` / `-c` | Config file (default: `culebra.toml`) |
| `--timeout` | Per-step timeout in seconds (default: 30) |

---

### fixedpoint

Detect fixed-point convergence in self-hosting compilers.

```
culebra fixedpoint <COMPILER> <SOURCE> [OPTIONS]
```

Compiles the source N times, each time using the previous output as the compiler. Compares outputs between iterations. A self-hosting compiler should reach a fixed point (identical output) within 2-3 iterations.

**Options:**
| Flag | Description |
|---|---|
| `--max-iters <N>` | Maximum iterations before giving up (default: 3) |
| `--timeout <SECS>` | Timeout per compilation (default: 120) |
| `--runtime <PATH>` | Path to C runtime to link |

---

### status

Show bootstrap self-hosting progress.

```
culebra status [--config <FILE>]
```

Reads `culebra.toml` and displays project info, stage names, and test counts.

---

### init

Generate a `culebra.toml` starter config.

```
culebra init
```

Creates a `culebra.toml` with example `[project]`, `[[stages]]`, and `[[tests]]` sections.

---

## Template Engine

### How it works

Culebra's template engine is inspired by [Nuclei](https://github.com/projectdiscovery/nuclei). Bug patterns are defined as YAML templates. The Rust binary loads templates, parses IR, runs matching logic, and reports findings.

**Flow:**

1. Templates are loaded from `culebra-templates/` (or `--template` path)
2. Templates are filtered by `--tags`, `--severity`, `--id`
3. The input `.ll` file is parsed into an IR module (functions, globals, string constants, struct types)
4. Each template's matchers run against the appropriate section of the IR
5. Findings are collected, sorted by severity, and output in the requested format
6. If `--autofix` is set, applicable fixes are applied

### Template specification

Every template is a YAML file with the following structure:

```yaml
# Required: unique identifier
id: my-template-id

# Required: metadata
info:
  name: Human-readable name              # Required
  severity: critical|high|medium|low|info # Required
  author: yourname                        # Optional
  description: |                          # Optional, multiline
    Detailed description of the bug pattern.
  impact: |                               # Optional, multiline
    What happens when this bug is present.
  tags:                                   # Optional, for filtering
    - abi
    - string
  references:                             # Optional, URLs
    - https://example.com/related-issue
  cwe: CWE-188                           # Optional, CWE identifier
  created: "2025-03-28"                   # Optional
  updated: "2025-03-28"                   # Optional

# Optional: what files/sections this template applies to
scope:
  file_type: llvm-ir       # llvm-ir | elf-binary | c-header | cross-reference
  section: globals         # globals | functions | declarations | metadata | all

# Required: matching logic
match:
  # ... (see Matcher Types below)

# Optional: extract data from matches for reporting
extractors:
  - type: regex
    name: variable_name
    pattern: 'regex with (capture group)'
    group: 1

# Optional: how to display findings
report:
  format: |
    Use {variable_name} from extractors in the message.
  evidence:
    show_line: true
    show_context: 2

# Optional: how to fix the issue
remediation:
  suggestion: "Human-readable fix instructions"
  autofix:                 # Optional, enables --autofix
    type: line_replace
    match: 'regex to match'
    replace: 'replacement string'
  difficulty: trivial|moderate|complex

# Optional: related template IDs
related:
  - other-template-id
```

### Matcher types

#### Regex matcher

Matches single-line regex patterns against the selected section.

```yaml
match:
  matchers:
    - type: regex
      name: descriptive_name
      pattern:
        - 'first regex pattern'
        - 'alternative pattern'    # any match triggers
      # Optional: negate the match
      condition: not_contains
      # Optional: extract inline
      extractor:
        name: captured_value
        group: 1
  condition: or    # or | and -- how to combine multiple matchers
```

**Section targeting:**
- `globals` -- Only lines starting with `@` that contain `constant` or `global`
- `functions` -- All lines within function bodies
- `declarations` -- Lines starting with `declare`
- `metadata` -- Lines starting with `!`
- `all` -- Every line in the file

#### Sequence matcher

Multi-line pattern matching within function bodies. Supports captures, forward references, and absence checks.

```yaml
match:
  type: sequence
  steps:
    # Step 1: Find a GEP instruction, capture its output
    - id: gep_to_field
      pattern: '(%[\w.]+) = getelementptr.* ptr (%[\w.]+), i32 0, i32 (\d+)'
      capture:
        gep_result: 1      # capture group 1 -> variable "gep_result"
        struct_ptr: 2
        field_index: 3

    # Step 2: Find a call using the captured value, within 10 lines
    - id: direct_push
      pattern: 'call void @__mn_list_push\(ptr %{gep_result}'  # {gep_result} is interpolated
      after: gep_to_field       # search starts after step 1's match
      within_lines: 10          # only search within 10 lines

    # Step 3: Absence check -- this pattern must NOT appear
    - id: missing_writeback
      type: absent              # "absent" means this must NOT match
      pattern: 'store .+ ptr %{gep_result}'
      after: direct_push
      within_lines: 15

  condition: all    # all steps must match (including absence checks)
```

**Key features:**
- **Captures** propagate between steps via `{variable_name}` interpolation
- **`after`** constrains where the search starts (after a previous step's match position)
- **`within_lines`** limits the search window
- **`type: absent`** inverts the match -- the step succeeds if the pattern is NOT found

#### Cross-reference matcher

Compares patterns across two files (e.g., IR struct types vs C header structs). Requires `--header` flag.

```yaml
scope:
  file_type: cross-reference
  inputs:
    ir_file: "*.ll"
    c_header: "*.h|*.c"

match:
  type: cross_reference
  steps:
    - id: ir_struct
      file: ir_file
      pattern: '%struct\.(\w+) = type \{([^}]+)\}'
      capture:
        struct_name: 1
        ir_fields: 2

    - id: c_struct
      file: c_header
      pattern: 'typedef struct \{([^}]+)\} (\w+);'
      capture:
        c_fields: 1
        c_name: 2

    - id: compare
      type: layout_compare
      type_map:
        "ptr": ["char *", "void *"]
        "i64": ["int64_t", "size_t"]
```

#### Byte scanner

Scans for raw byte values in string constants.

```yaml
match:
  matchers:
    - type: byte_scan
      name: raw_control_chars
      byte_range: [1, 31]      # scan for bytes 0x01-0x1F
      exclude: [10]            # exclude newline (0x0A)
  condition: or
```

### Extractors

Extractors pull named values from matched text for use in report templates.

```yaml
extractors:
  # Regex extractor -- captures a group from the matched text
  - type: regex
    name: constant_name
    pattern: '(@[\w.]+) = '
    group: 1

  # Computed extractor (stub -- for future engine-level logic)
  - type: computed
    name: address_parity
    method: layout_simulation
```

Extracted values are available in report format strings as `{constant_name}`.

### Autofix

Templates can include auto-fix definitions that modify the source file.

```yaml
remediation:
  autofix:
    type: line_replace
    match: '(constant \[\d+ x i8\] c"[^"]*")\s*$'    # regex to match
    replace: '\1, align 2'                              # replacement (supports backrefs)
```

**Usage:**

```bash
# Preview fixes
culebra scan file.ll --autofix --dry-run

# Apply fixes
culebra scan file.ll --autofix
```

The dry-run mode shows a diff-like output of what would change:

```
autofix Dry run -- 2 fixes would be applied:
  file.ll:7
    - @.str.0 = private constant [6 x i8] c"hello\00"
    + @.str.0 = private constant [6 x i8] c"hello\00", align 2
```

### Writing custom templates

1. Create a YAML file in `culebra-templates/` (or any subdirectory)
2. Define the template following the specification above
3. Run `culebra scan file.ll` -- your template is automatically loaded

**Minimal template:**

```yaml
id: my-check
info:
  name: My custom check
  severity: medium
  tags:
    - custom

scope:
  file_type: llvm-ir
  section: functions

match:
  matchers:
    - type: regex
      name: pattern
      pattern:
        - 'some pattern to detect'
  condition: or

remediation:
  suggestion: "How to fix this"
```

**Testing your template:**

```bash
# Run only your template
culebra scan file.ll --id my-check

# Or point to a single file
culebra scan file.ll --template ./my-check.yaml
```

---

## Workflows

Workflows chain multiple template scans with stop conditions. They're defined as YAML files in `culebra-templates/workflows/`.

### Workflow specification

```yaml
id: my-workflow
info:
  name: My workflow name
  severity: critical
  description: What this workflow does.
  tags:
    - workflow

workflow:
  # Step 1: Run IR and string templates
  - templates:
      tags:
        - ir
        - string
    input: "{stage1_output}"     # {var} references resolved from --input flags
    stop_on: critical            # stop workflow if critical findings appear

  # Step 2: Run ABI templates (only if step 1 passed)
  - templates:
      tags:
        - abi
    input: "{stage1_output}"
    stop_on: critical

  # Step 3: Run bootstrap templates
  - templates:
      tags:
        - bootstrap
    input: "{stage1_output}"
```

**Key features:**
- **`templates.tags`** -- Select templates by tag for this step
- **`templates.ids`** -- Or select specific template IDs
- **`input`** -- File path with `{variable}` interpolation from `--input` flags
- **`stop_on`** -- Stop the entire workflow if findings at this severity or worse are found

### Shipped workflows

| Workflow | Purpose | Steps |
|---|---|---|
| `bootstrap-health-check` | Full bootstrap validation | IR/string checks -> ABI checks -> bootstrap divergence |
| `pre-commit` | Quick pre-commit gate | IR checks (stop on critical) -> ABI/string checks |
| `ci-full` | Comprehensive CI scan | IR -> ABI/string -> bootstrap (no stop conditions) |

---

## Shipped Templates

### ABI templates

#### `unaligned-string-constant` (Critical)

String constants without `align 2+` can land at odd addresses. Runtimes using pointer tagging (bit 0 masking) will mangle these pointers, causing silent string comparison failures.

**Auto-fixable:** Yes -- appends `, align 2` to constant declarations.

#### `direct-push-no-writeback` (High)

Pushing to a list via GEP directly into a struct field without loading to a temp alloca first. LLVM alias analysis may cache the struct load from before the push, losing the mutation.

**Matcher:** Sequence -- finds GEP -> list_push -> missing store-back pattern.

#### `sret-input-output-alias` (High)

sret output pointer aliasing an input struct pointer. Writes to the return struct corrupt the input mid-computation.

#### `missing-byval-large-struct` (Medium)

Structs larger than 2 registers passed as bare `ptr` without `byval` attribute. Callee may corrupt caller's data.

#### `tagged-pointer-odd-address` (High)

Odd-sized constants without alignment. Pointer tagging systems will misinterpret these.

**Auto-fixable:** Yes.

#### `struct-layout-mismatch` (Critical)

IR struct type and C header struct have different field counts or types. Requires `--header` flag for cross-reference matching.

---

### IR templates

#### `byte-count-mismatch` (High)

`[N x i8]` declaration says N bytes but actual `c"..."` content differs. Causes buffer overread or silent truncation.

#### `empty-switch-body` (Critical)

Switch with 0 cases -- all values fall through to default. Match arms were not generated.

#### `ret-type-mismatch` (Critical)

`ret` instruction type doesn't match function signature. LLVM will reject during verification.

#### `raw-control-byte-in-constant` (Medium)

Raw control bytes (0x01-0x1F) in `c"..."` constants break line-based tooling.

**Matcher:** Byte scanner for control characters.

#### `phi-predecessor-mismatch` (High)

PHI nodes referencing non-existent predecessor blocks. Often caused by IR transforms that modify control flow without updating PHIs.

#### `unreachable-after-branch` (Medium)

Instructions after a terminator (br, ret, switch, unreachable). Dead code that LLVM will reject.

**Matcher:** Sequence -- finds terminator followed by instruction within 3 lines.

---

### Binary templates

#### `odd-address-rodata` (High)

String at odd virtual address in `.rodata` section. Binary-level confirmation of alignment bugs.

#### `missing-symbol` (Critical)

Expected runtime symbol (e.g., `__mn_*`) missing from binary symbol table.

---

### Bootstrap templates

#### `stage-output-divergence` (High)

Flags function/global definitions for stage comparison. Use with `culebra diff` for per-function analysis.

#### `function-count-drop` (Critical)

Flags function definitions to enable count comparison between stages. A drop in function count between stages is a critical bootstrap bug.

#### `fixed-point-delta` (High)

Flags definitions for fixed-point convergence analysis. Use with `culebra fixedpoint` for automatic detection.

---

## Output Formats

### Text (default)

Colored terminal output with severity badges, template IDs, locations, and fix suggestions.

```bash
culebra scan file.ll
```

### JSON

Structured JSON with all finding details, extractions, and metadata.

```bash
culebra scan file.ll --format json
```

```json
{
  "file": "file.ll",
  "findings_count": 2,
  "findings": [
    {
      "template_id": "unaligned-string-constant",
      "template_name": "String constant missing alignment",
      "severity": "critical",
      "file": "file.ll",
      "line": 7,
      "function": null,
      "matched_text": "@.str.0 = private constant [6 x i8] c\"hello\\00\"",
      "description": "...",
      "impact": "...",
      "suggestion": "Add ', align 2'",
      "cwe": "CWE-188",
      "tags": ["abi", "string"],
      "extractions": {
        "constant_name": "@.str.0",
        "byte_size": "6"
      }
    }
  ]
}
```

### SARIF

[SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html) for GitHub Code Scanning integration.

```bash
culebra scan file.ll --format sarif > results.sarif
```

Upload to GitHub:

```bash
# In a GitHub Action:
- name: Run Culebra scan
  run: culebra scan main.ll --format sarif > results.sarif

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

### Markdown

Markdown report with severity summary table and per-finding details. Useful for CI reports, PR comments.

```bash
culebra scan file.ll --format markdown > report.md
```

---

## Configuration: culebra.toml

The `culebra.toml` file configures project-level settings for `test`, `pipeline`, `status`, and other commands.

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

[[tests]]
name = "hello"
source = 'fn main() { print("hello") }'
expect = "hello"

[[tests]]
name = "math"
source = "fn main() { print(2 + 3) }"
expect = "5"
```

**Substitution variables in stage commands:**
| Variable | Resolved to |
|---|---|
| `{input}` | The stage's `input` field |
| `{output}` | The stage's `output` field |
| `{prev_output}` | The previous stage's `output` field |

Generate a starter config with `culebra init`.

---

## Template Directory

Templates are loaded from the first directory found:

1. `./culebra-templates/` (project-local)
2. `<binary_dir>/culebra-templates/` (next to the culebra binary)
3. `~/.culebra/templates/` (user-global)

Override with `--template <PATH>`.

**Recommended structure:**

```
culebra-templates/
  abi/
    unaligned-string-constant.yaml
    direct-push-no-writeback.yaml
    sret-input-output-alias.yaml
    missing-byval-large-struct.yaml
    tagged-pointer-odd-address.yaml
    struct-layout-mismatch.yaml
  ir/
    byte-count-mismatch.yaml
    empty-switch-body.yaml
    ret-type-mismatch.yaml
    raw-control-byte.yaml
    phi-predecessor-mismatch.yaml
    unreachable-after-branch.yaml
  binary/
    odd-address-rodata.yaml
    missing-symbol.yaml
  bootstrap/
    stage-output-divergence.yaml
    function-count-drop.yaml
    fixed-point-delta.yaml
  workflows/
    bootstrap-health-check.yaml
    pre-commit.yaml
    ci-full.yaml
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Success / no critical findings |
| `1` | Error or critical/high findings detected |

For `scan`: exit code 1 if any finding has severity `critical` or `high`. This makes it suitable for CI gates:

```bash
culebra scan main.ll --severity critical,high || exit 1
```

---

## CI/CD Integration

### GitHub Actions

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
# .git/hooks/pre-commit
culebra scan output.ll --severity critical,high
```

### Generic CI

```bash
# Run scan, fail on critical/high
culebra scan main.ll --severity critical,high

# Full report as markdown artifact
culebra scan main.ll --format markdown > culebra-report.md

# JSON for programmatic processing
culebra scan main.ll --format json | jq '.findings[] | select(.severity == "critical")'
```
