# Culebra Playground

Minimal IR fixtures reproducing the Mapanare v2.2.0 bootstrap bugs.
Run Culebra's templates against these to see what would have been caught automatically.

## The bugs

| Bug | Root cause | Template that catches it |
|-----|-----------|------------------------|
| `TOKS=0` — tokenizer produces zero tokens | `__mn_range` declared as `→ {i64, i64}` in stage2, but C runtime returns `void*` (ptr) | `abi/return-type-divergence` |
| Only identifiers tokenized | Nested else branches dropped by Python lowerer — only `if_alpha` survived | `ir/dropped-else-branch` |
| 1 push vs 5 | `__mn_list_push` called once (alpha only) instead of 5 times (all token types) | `bootstrap/call-count-divergence` |
| 352 lines vs 846 | Stage2 tokenize body is less than half the size of stage1 | `bootstrap/body-size-shrinkage` |

## Quick start

```bash
# Scan stage2 for ABI bugs (catches the __mn_range mismatch)
culebra scan playground/fixtures/stage2-tokenize.ll --tags abi

# Scan for dropped branches
culebra scan playground/fixtures/stage2-tokenize.ll --tags ir

# Scan for bootstrap divergence patterns
culebra scan playground/fixtures/stage2-tokenize.ll --tags bootstrap

# Diff both stages to see exactly what's missing
culebra diff playground/fixtures/stage1-tokenize.ll playground/fixtures/stage2-tokenize.ll

# Run the full playground workflow
culebra workflow culebra-templates/workflows/playground-mapanare.yaml playground/fixtures/stage2-tokenize.ll
```

## Fixtures

- `stage1-tokenize.ll` — Correct IR from the Python compiler. Full 5-branch tokenizer.
- `stage2-tokenize.ll` — Broken IR from the self-hosted compiler. Both bugs present.

## Adding your own

Drop any `.ll` file into `playground/fixtures/` and scan it:

```bash
culebra scan playground/fixtures/your-file.ll
```
