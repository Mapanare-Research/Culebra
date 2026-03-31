; Stage 2 IR — tokenize as emitted by the self-hosted compiler (BROKEN)
; Reproduces the two critical bugs from Mapanare v2.2.0:
;   1. __mn_range returns {i64, i64} instead of ptr (ABI mismatch)
;   2. Only if_alpha branch survives — digit/string/char/op branches dropped

; --- Runtime declarations (BUG: __mn_range return type wrong) ---
declare {i64, i64} @__mn_range(i64, i64)
declare void @__mn_list_push(ptr, ptr, i64)
declare i1 @__mn_str_eq({ptr, i64}, {ptr, i64})
declare i64 @__mn_str_len({ptr, i64})
declare {ptr, i64} @__mn_str_slice({ptr, i64}, i64, i64)
declare i1 @__mn_char_is_alpha(i64)
declare i1 @__mn_char_is_digit(i64)

; --- String constants (correctly aligned — this was already fixed) ---
@str.0 = private unnamed_addr constant [3 x i8] c"fn\00", align 2
@str.1 = private unnamed_addr constant [4 x i8] c"let\00", align 2
@str.2 = private unnamed_addr constant [6 x i8] c"match\00", align 2
@str.3 = private unnamed_addr constant [4 x i8] c"for\00", align 2

; --- tokenize: TRUNCATED function — only alpha branch survived ---
define { ptr, i64, i64, i64 } @tokenize({ ptr, i64 } %source, { ptr, i64 } %filename) {
entry:
  %tokens = alloca { ptr, i64, i64, i64 }, align 8
  %pos = alloca i64, align 8
  %slen = alloca i64, align 8
  store i64 0, ptr %pos
  %len = call i64 @__mn_str_len({ ptr, i64 } %source)
  store i64 %len, ptr %slen
  ; BUG: __mn_range returns {i64, i64} — stage1 expects ptr (iterator)
  ; This causes for_start >= for_end → loop body never executes → TOKS=0
  %range = call {i64, i64} @__mn_range(i64 0, i64 1000000)
  %for_start = extractvalue {i64, i64} %range, 0
  %for_end = extractvalue {i64, i64} %range, 1
  br label %for_header

for_header:
  %pos.val = load i64, ptr %pos
  %slen.val = load i64, ptr %slen
  %done = icmp sge i64 %pos.val, %slen.val
  br i1 %done, label %for_exit, label %for_body

for_body:
  %ch = call i64 @char_at({ ptr, i64 } %source, i64 %pos.val)
  %is_alpha = call i1 @__mn_char_is_alpha(i64 %ch)
  br i1 %is_alpha, label %if_alpha, label %if_merge

if_alpha:
  %ident_tok = call { ptr, i64 } @scan_identifier({ ptr, i64 } %source, i64 %pos.val)
  call void @__mn_list_push(ptr %tokens, ptr %ident_tok, i64 16)
  br label %if_merge

; BUG: All else branches are MISSING — digit, string, char, operator
; Stage1 has 5 __mn_list_push calls, stage2 has only 1
; This is the "nested else dropped" Python lowerer bug

if_merge:
  %new_pos = add i64 %pos.val, 1
  store i64 %new_pos, ptr %pos
  br label %for_header

for_exit:
  %result = load { ptr, i64, i64, i64 }, ptr %tokens
  ret { ptr, i64, i64, i64 } %result
}

; --- Helper declarations ---
declare i64 @char_at({ ptr, i64 }, i64)
declare { ptr, i64 } @scan_identifier({ ptr, i64 }, i64)
declare { ptr, i64 } @scan_number({ ptr, i64 }, i64)
declare { ptr, i64 } @scan_string({ ptr, i64 }, i64)
declare { ptr, i64 } @scan_char({ ptr, i64 }, i64)
declare { ptr, i64 } @scan_operator({ ptr, i64 }, i64)
