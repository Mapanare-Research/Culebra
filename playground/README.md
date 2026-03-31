# Culebra Playground

Minimal IR fixtures reproducing real Mapanare v2.2.0 bootstrap bugs.
Run Culebra's templates against these to see what would have been caught automatically.

## The bugs

### Tokenizer bugs (`stage2-tokenize.ll`)

| Bug | Root cause | Template that catches it |
|-----|-----------|------------------------|
| `TOKS=0` — tokenizer produces zero tokens | `__mn_range` declared as `-> {i64, i64}` in stage2, but C runtime returns `void*` (ptr) | `abi/return-type-divergence` |
| Only identifiers tokenized | Nested else branches dropped by Python lowerer — only `if_alpha` survived | `ir/dropped-else-branch` |
| 1 push vs 5 | `__mn_list_push` called once (alpha only) instead of 5 times (all token types) | `bootstrap/call-count-divergence` |
| 352 lines vs 846 | Stage2 tokenize body is less than half the size of stage1 | `bootstrap/body-size-shrinkage` |

### Lowerer bugs (`stage2-lowerer-bugs.ll`)

| Bug | Root cause | Template that catches it |
|-----|-----------|------------------------|
| All else clauses silently None | Option zeroinit + inner type store clobbers i1 discriminant | `ir/option-type-pun-zeroinit` |
| 93/367 functions vanish from binary | LLVM -O1 strips `define internal` functions with sret | `ir/internal-linkage-dce` |
| SIGSEGV in libc SSE instructions | Allocas in non-entry blocks misalign RSP by 8 | `ir/dynamic-alloca-non-entry` |
| Return inside match/if falls through | Block terminator missing after ret in nested control flow | `ir/return-inside-nested-block` |
| Crash copying 760-byte struct | Large structs passed by-value via load/store instead of memcpy | `abi/large-struct-by-value` |

### Control flow bugs (`stage2-control-flow-bugs.ll`)

| Bug | Root cause | Template that catches it |
|-----|-----------|------------------------|
| llvm-as rejects PHI types | if_result PHI declares `%enum.Expr` but operand is `i64` | `ir/phi-operand-type-mismatch` |
| Infinite loop, stack overflow | `break` inside `if` inside `for` silently dropped — loop runs 1M iterations | `ir/break-inside-nested-control` |
| Semantic checker reads garbage | `__mn_list_new(16)` but Definition struct is ~48 bytes | `abi/list-element-size-undercount` |

## Quick start

```bash
# Scan stage2 for ABI bugs (catches the __mn_range mismatch)
culebra scan playground/fixtures/stage2-tokenize.ll --tags abi

# Scan for dropped branches
culebra scan playground/fixtures/stage2-tokenize.ll --tags ir

# Scan for bootstrap divergence patterns
culebra scan playground/fixtures/stage2-tokenize.ll --tags bootstrap

# Scan the lowerer bugs fixture
culebra scan playground/fixtures/stage2-lowerer-bugs.ll

# Scan control flow bugs — break dropped, PHI mismatch, list undercount
culebra scan playground/fixtures/stage2-control-flow-bugs.ll

# Diff both stages to see exactly what's missing
culebra diff playground/fixtures/stage1-tokenize.ll playground/fixtures/stage2-tokenize.ll

# Run the full playground workflow
culebra workflow culebra-templates/workflows/playground-mapanare.yaml playground/fixtures/stage2-tokenize.ll

# Test the autofix (dry-run)
culebra scan playground/fixtures/stage2-tokenize.ll --id return-type-divergence --autofix --dry-run
```

## Fixtures

- `stage1-tokenize.ll` — Correct IR from the Python compiler. Full 5-branch tokenizer.
- `stage2-tokenize.ll` — Broken IR from the self-hosted compiler. ABI + branch-dropping bugs.
- `stage2-lowerer-bugs.ll` — Option type-pun, internal DCE, dynamic alloca, return fall-through, large struct.
- `stage2-control-flow-bugs.ll` — PHI type mismatch, break dropped, list element size undercount.

## Adding your own

Drop any `.ll` file into `playground/fixtures/` and scan it:

```bash
culebra scan playground/fixtures/your-file.ll
```
