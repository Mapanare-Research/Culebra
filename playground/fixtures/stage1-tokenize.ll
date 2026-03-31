; Stage 1 IR — tokenize function as emitted by the Python compiler (correct)
; This is the "golden" reference. Stage2 output should converge to this.

; --- Runtime declarations (correct ABI) ---
declare ptr @__mn_range(i64, i64)
declare void @__mn_list_push(ptr, ptr, i64)
declare i1 @__mn_str_eq({ptr, i64}, {ptr, i64})
declare i64 @__mn_str_len({ptr, i64})
declare {ptr, i64} @__mn_str_slice({ptr, i64}, i64, i64)
declare i1 @__mn_char_is_alpha(i64)
declare i1 @__mn_char_is_digit(i64)

; --- String constants (correctly aligned) ---
@str.0 = private unnamed_addr constant [3 x i8] c"fn\00", align 2
@str.1 = private unnamed_addr constant [4 x i8] c"let\00", align 2
@str.2 = private unnamed_addr constant [6 x i8] c"match\00", align 2
@str.3 = private unnamed_addr constant [4 x i8] c"for\00", align 2

; --- tokenize: full function with all branches ---
define {ptr, i64, i64, i64} @lexer__tokenize({ptr, i64} %source, {ptr, i64} %filename) {
pre_entry:
  %tokens = alloca {ptr, i64, i64, i64}, align 8
  %pos = alloca i64, align 8
  %slen = alloca i64, align 8
  store i64 0, ptr %pos
  %len = call i64 @__mn_str_len({ptr, i64} %source)
  store i64 %len, ptr %slen
  br label %for_header

for_header:
  %pos.val = load i64, ptr %pos
  %slen.val = load i64, ptr %slen
  %done = icmp sge i64 %pos.val, %slen.val
  br i1 %done, label %for_exit, label %for_body

for_body:
  %ch = call i64 @char_at({ptr, i64} %source, i64 %pos.val)
  %is_alpha = call i1 @__mn_char_is_alpha(i64 %ch)
  br i1 %is_alpha, label %if_alpha, label %else_check_digit

if_alpha:
  ; Accumulate identifier token
  %ident_tok = call {ptr, i64} @scan_identifier({ptr, i64} %source, i64 %pos.val)
  call void @__mn_list_push(ptr %tokens, ptr %ident_tok, i64 16)
  br label %for_continue

else_check_digit:
  %is_digit = call i1 @__mn_char_is_digit(i64 %ch)
  br i1 %is_digit, label %if_digit, label %else_check_string

if_digit:
  ; Accumulate number token
  %num_tok = call {ptr, i64} @scan_number({ptr, i64} %source, i64 %pos.val)
  call void @__mn_list_push(ptr %tokens, ptr %num_tok, i64 16)
  br label %for_continue

else_check_string:
  %is_quote = icmp eq i64 %ch, 34
  br i1 %is_quote, label %if_string, label %else_check_char

if_string:
  ; Accumulate string token
  %str_tok = call {ptr, i64} @scan_string({ptr, i64} %source, i64 %pos.val)
  call void @__mn_list_push(ptr %tokens, ptr %str_tok, i64 16)
  br label %for_continue

else_check_char:
  %is_squote = icmp eq i64 %ch, 39
  br i1 %is_squote, label %if_char, label %else_operator

if_char:
  ; Accumulate char token
  %char_tok = call {ptr, i64} @scan_char({ptr, i64} %source, i64 %pos.val)
  call void @__mn_list_push(ptr %tokens, ptr %char_tok, i64 16)
  br label %for_continue

else_operator:
  ; Accumulate operator/punctuation token
  %op_tok = call {ptr, i64} @scan_operator({ptr, i64} %source, i64 %pos.val)
  call void @__mn_list_push(ptr %tokens, ptr %op_tok, i64 16)
  br label %for_continue

for_continue:
  %new_pos = add i64 %pos.val, 1
  store i64 %new_pos, ptr %pos
  br label %for_header

for_exit:
  %result = load {ptr, i64, i64, i64}, ptr %tokens
  ret {ptr, i64, i64, i64} %result
}

; --- Helper declarations ---
declare i64 @char_at({ptr, i64}, i64)
declare {ptr, i64} @scan_identifier({ptr, i64}, i64)
declare {ptr, i64} @scan_number({ptr, i64}, i64)
declare {ptr, i64} @scan_string({ptr, i64}, i64)
declare {ptr, i64} @scan_char({ptr, i64}, i64)
declare {ptr, i64} @scan_operator({ptr, i64}, i64)
