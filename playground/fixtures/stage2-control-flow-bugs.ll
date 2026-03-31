; Stage 2 IR — control flow and list bugs from Mapanare v2.2.0 chat
; Reproduces: PHI type mismatch, break dropped, list element size undercount

; --- Bug 1: PHI operand type mismatch ---
; if_result PHI declares %enum.Expr but one operand is i64
; This happens when if-else is used as statement, not expression

%enum.Expr = type { i64, [24 x i64] }

define void @lower__lower_call(ptr %state, ptr %expr) {
entry:
  %tag = load i64, ptr %expr
  %is_struct_init = icmp eq i64 %tag, 42
  br i1 %is_struct_init, label %if_then, label %if_else

if_then:
  ; Struct constructor returns %enum.Expr
  %constructed = load %enum.Expr, ptr %expr
  br label %if_merge

if_else:
  ; Fallback returns i64 (position), not %enum.Expr
  %fallback_pos = add i64 0, 1
  br label %if_merge

if_merge:
  ; BUG: PHI type is %enum.Expr but %fallback_pos is i64
  ; llvm-as rejects: "instruction forward referenced with type 'i64'"
  %if_result = phi %enum.Expr [ %constructed, %if_then ], [ zeroinitializer, %if_else ]
  ret void
}

; --- Bug 2: Break inside if-inside-for dropped ---
; The if_then branches to if_merge instead of for_exit
; Loop runs 1M iterations instead of breaking when done

declare i1 @__mn_char_is_whitespace(i64)
declare i64 @char_at({ptr, i64}, i64)
declare {ptr, i64, i64, i64} @__mn_list_new(i64)

define {ptr, i64, i64, i64} @lexer__skip_whitespace({ptr, i64} %source, i64 %pos, i64 %slen) {
entry:
  %pos.addr = alloca i64, align 8
  store i64 %pos, ptr %pos.addr
  br label %for_header

for_header:
  %p = load i64, ptr %pos.addr
  %in_range = icmp slt i64 %p, %slen
  br i1 %in_range, label %for_body, label %for_exit

for_body:
  %ch = call i64 @char_at({ptr, i64} %source, i64 %p)
  %is_ws = call i1 @__mn_char_is_whitespace(i64 %ch)
  br i1 %is_ws, label %if_then_ws, label %if_else_ws

if_then_ws:
  %next = add i64 %p, 1
  store i64 %next, ptr %pos.addr
  ; BUG: branches to if_merge instead of for_header — break was here
  br label %if_merge_ws

if_else_ws:
  ; Should have been: br label %for_exit (break)
  ; BUG: branches to if_merge — break dropped!
  br label %if_merge_ws

if_merge_ws:
  ; Loop continues unconditionally — break was silently dropped
  ; Creates millions of COW lists, stack overflow
  %cow = call {ptr, i64, i64, i64} @__mn_list_new(i64 16)
  br label %for_header

for_exit:
  %result = load {ptr, i64, i64, i64}, ptr %pos.addr
  ret {ptr, i64, i64, i64} %result
}

; --- Bug 3: List element size undercount ---
; Definition struct is ~48 bytes but list created with elem_sz=16

%struct.Definition = type { {ptr, i64}, {ptr, i64}, i64, {ptr, i64, i64, i64}, i1 }

define void @sem__check_definitions({ptr, i64, i64, i64} %defs) {
entry:
  ; BUG: elem_sz=16 but %struct.Definition is ~48 bytes
  ; Elements pushed will be truncated at 16 bytes
  %new_list = call {ptr, i64, i64, i64} @__mn_list_new(i64 16)

  ; Also common: elem_sz=8 for struct elements
  %another = call {ptr, i64, i64, i64} @__mn_list_new(i64 8)
  ret void
}

; --- Helper declarations ---
declare void @__mn_list_push(ptr, ptr, i64)
