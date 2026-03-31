; ModuleID = 'stage2-phi-enum-bugs'
; Fixture for dead-phi-chain and enum-payload-slot-undercount templates

; Enum with undersized payload — FnDefData needs ~29 slots but only has 3
%enum.Definition = type { i64, [3 x i64] }
%enum.Expr = type { i64, [4 x i64] }
%struct.FnDefData = type { { ptr, i64 }, i1, { ptr, i64, i64, i64 }, { ptr, i64, i64, i64 }, { i1, ptr }, { ptr, i64, i64, i64 }, { ptr, i64, i64, i64 }, { ptr, i64, i64, i64 } }

; Function with dead PHI chain — if_result PHIs from statement-context if-else
define void @lower__emit_stmt(ptr %st, i64 %kind) {
entry:
  %cmp = icmp eq i64 %kind, 1
  br i1 %cmp, label %if_then6, label %if_else7

if_then6:
  %t44 = add i64 %kind, 100
  br label %if_merge11

if_else7:
  %t73 = call %enum.Expr @lower__lower_expr(ptr %st)
  br label %if_merge14

if_merge14:
  %t76 = add i64 0, 0
  br label %if_merge11

if_merge11:
  ; Dead PHI chain: if_result113 (i64) used only by if_result114 (%enum.Expr)
  ; Neither is consumed — but llvm-as rejects the forward-reference type clash
  %if_result113 = phi i64 [ %t44, %if_then6 ], [ %t76, %if_merge14 ]
  %if_result114 = phi %enum.Expr [ zeroinitializer, %if_then6 ], [ %if_result113, %if_merge14 ]
  ret void
}

declare %enum.Expr @lower__lower_expr(ptr)
