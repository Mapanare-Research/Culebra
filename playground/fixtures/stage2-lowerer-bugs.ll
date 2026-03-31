; Stage 2 IR — lowerer bugs from Mapanare v2.2.0 chat reviews
; Reproduces: Option type-pun, internal linkage DCE, dynamic alloca,
;             return-inside-nested-block, large struct by-value

; --- Bug 1: Option type-pun zeroinitializer ---
; ElseClause ({i64, ptr}) stored over Option<ElseClause> ({i1, {i64, ptr}})
; ElseBlock tag = i64 0 → reads as i1 0 → None. All else branches dropped.

define {i1, {i64, ptr}} @parser__wrap_else_clause({i64, ptr} %clause) {
entry:
  %opt = alloca {i1, {i64, ptr}}, align 8
  ; BUG: zeroinitializer sets i1 discriminant to 0 (None)
  store {i1, {i64, ptr}} zeroinitializer, ptr %opt
  ; BUG: inner type store overwrites from offset 0, clobbering discriminant
  store {i64, ptr} %clause, ptr %opt
  ; Reads back as None because i1 at offset 0 is 0 (ElseBlock tag)
  %result = load {i1, {i64, ptr}}, ptr %opt
  ret {i1, {i64, ptr}} %result
}

; --- Bug 2: Internal linkage DCE ---
; These functions are at risk of being stripped by LLVM -O1

define internal void @lower__emit_field_access(ptr sret({ptr, i64}) %result, ptr %state, i64 %idx) {
entry:
  %tmp = alloca {ptr, i64}, align 8
  store {ptr, i64} zeroinitializer, ptr %tmp
  %val = load {ptr, i64}, ptr %tmp
  store {ptr, i64} %val, ptr %result
  ret void
}

define internal void @lower__emit_match_arm(ptr sret({ptr, i64, i64, i64}) %result, ptr %state, i64 %tag) {
entry:
  %tmp = alloca {ptr, i64, i64, i64}, align 8
  store {ptr, i64, i64, i64} zeroinitializer, ptr %tmp
  %val = load {ptr, i64, i64, i64}, ptr %tmp
  store {ptr, i64, i64, i64} %val, ptr %result
  ret void
}

define internal i64 @lower__resolve_type(ptr %state, {ptr, i64} %name) {
entry:
  ret i64 0
}

; --- Bug 3: Dynamic alloca in non-entry block ---
; Alloca after entry creates dynamic stack allocation, misaligns RSP

define void @emit__emit_call(ptr sret({ptr, i64}) %result, {ptr, i64} %fn_name, {ptr, i64, i64, i64} %args) {
entry:
  %n = extractvalue {ptr, i64, i64, i64} %args, 1
  %has_args = icmp sgt i64 %n, 0
  br i1 %has_args, label %build_args, label %emit_call

build_args:
  ; BUG: alloca in non-entry block — misaligns RSP
  %arg_buf = alloca [8 x i8], align 8
  %coerced = alloca {ptr, i64}, align 8
  store {ptr, i64} %fn_name, ptr %coerced
  br label %emit_call

emit_call:
  store {ptr, i64} %fn_name, ptr %result
  ret void
}

; --- Bug 4: Return inside nested block — fall-through ---
; The ret does not terminate; code after it executes

define {ptr, i64} @util__strip_percent({ptr, i64} %s) {
entry:
  %first_char = call i64 @char_at({ptr, i64} %s, i64 0)
  %is_percent = icmp eq i64 %first_char, 37
  br i1 %is_percent, label %if_percent, label %no_strip

if_percent:
  %stripped = call {ptr, i64} @__mn_str_slice({ptr, i64} %s, i64 1, i64 -1)
  ; BUG: this ret should terminate, but fall-through happens
  ret {ptr, i64} %stripped
  ; BUG: code after ret still executes in compiled binary
  %fallback = call {ptr, i64} @__mn_str_slice({ptr, i64} %s, i64 0, i64 -1)
  ret {ptr, i64} %fallback

no_strip:
  ret {ptr, i64} %s
}

; --- Bug 5: Large struct by-value load/store ---
; 760-byte LowerResult copied by value — crashes at -O1

%LowerResult = type {{ptr, i64}, {{ptr, i64}, {ptr, i64, i64, i64}, {ptr, i64, i64, i64}, {ptr, i64, i64, i64}, {ptr, i64, i64, i64}, {ptr, i64, i64, i64}, {ptr, i64, i64, i64}, {ptr, i64, i64, i64}, {ptr, i64, i64, i64}}, {i1, {{ptr, i64}, {ptr, i64, i64, i64}, {{ptr, i64}, {ptr, i64}, {ptr, i64, i64, i64}}, {ptr, i64, i64, i64}, {ptr, i64, i64, i64}, i1}}, i64, i64, i64, {ptr, i64, i64, i64}}

define void @lower__lower_expr(ptr sret(%LowerResult) %result, ptr %state, ptr %expr) {
entry:
  %tmp = alloca %LowerResult, align 8
  ; BUG: by-value load of 760-byte struct
  %val = load %LowerResult, ptr %tmp
  store %LowerResult %val, ptr %result
  ret void
}

; --- Helper declarations ---
declare i64 @char_at({ptr, i64}, i64)
declare {ptr, i64} @__mn_str_slice({ptr, i64}, i64, i64)
