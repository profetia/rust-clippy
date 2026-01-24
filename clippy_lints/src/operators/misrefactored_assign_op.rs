use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::{eq_expr_value, sugg};
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::LateContext;

use super::MISREFACTORED_ASSIGN_OP;

pub(super) fn check<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx hir::Expr<'_>,
    op: hir::BinOpKind,
    lhs: &'tcx hir::Expr<'_>,
    rhs: &'tcx hir::Expr<'_>,
) {
    if let hir::ExprKind::Binary(binop, l, r) = &rhs.kind {
        if op != binop.node {
            return;
        }
        // lhs op= l op r
        if eq_expr_value(cx, lhs, l) {
            lint_misrefactored_assign_op(cx, expr, op, rhs, lhs, r);
        } else if is_commutative(op) && eq_expr_value(cx, lhs, r) {
            // lhs op= l commutative_op r
            lint_misrefactored_assign_op(cx, expr, op, rhs, lhs, l);
        }
    }
}

fn lint_misrefactored_assign_op(
    cx: &LateContext<'_>,
    expr: &hir::Expr<'_>,
    op: hir::BinOpKind,
    rhs: &hir::Expr<'_>,
    assignee: &hir::Expr<'_>,
    rhs_other: &hir::Expr<'_>,
) {
    span_lint_and_then(
        cx,
        MISREFACTORED_ASSIGN_OP,
        expr.span,
        "variable appears on both sides of an assignment operation",
        |diag| {
            let mut applicability = Applicability::MaybeIncorrect;
            let a = sugg::Sugg::hir_with_context(cx, assignee, expr.span.ctxt(), "..", &mut applicability);
            let r = sugg::Sugg::hir_with_context(cx, rhs, expr.span.ctxt(), "..", &mut applicability);
            let r_other = sugg::Sugg::hir_with_context(cx, rhs_other, expr.span.ctxt(), "..", &mut applicability);

            let long = format!("{a} = {}", sugg::make_binop(op, &a, &r));
            diag.span_suggestion(
                expr.span,
                format!(
                    "did you mean `{a} = {a} {} {r_other}` or `{long}`? Consider replacing it with",
                    op.as_str()
                ),
                format!("{a} {}= {r_other}", op.as_str()),
                applicability,
            );
            diag.span_suggestion(expr.span, "or", long, applicability);
        },
    );
}

#[must_use]
fn is_commutative(op: hir::BinOpKind) -> bool {
    use rustc_hir::BinOpKind::{
        Add, And, BitAnd, BitOr, BitXor, Div, Eq, Ge, Gt, Le, Lt, Mul, Ne, Or, Rem, Shl, Shr, Sub,
    };
    match op {
        Add | Mul | And | Or | BitXor | BitAnd | BitOr | Eq | Ne => true,
        Sub | Div | Rem | Shl | Shr | Lt | Le | Ge | Gt => false,
    }
}
