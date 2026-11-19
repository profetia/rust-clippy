#![expect(clippy::needless_doctest_main)]

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::is_entrypoint_fn;
use clippy_utils::source::snippet;
use rustc_attr_ir::find_attr;
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::{LateContext, LateLintPass, impl_lint_pass};
use rustc_middle::ty::TyCtxt;

declare_clippy_lint! {
    /// ### What it does
    /// Checks for recursion using the entrypoint.
    ///
    /// ### Why is this bad?
    /// Apart from special setups (which we could detect following attributes like `#![no_std]`),
    /// recursing into `main()` seems like an unintuitive anti-pattern we should be able to detect.
    ///
    /// ### Example
    /// ```no_run
    /// fn main() {
    ///     main();
    /// }
    /// ```
    #[clippy::version = "1.38.0"]
    pub MAIN_RECURSION,
    style,
    "recursion using the entrypoint"
}

impl_lint_pass!(MainRecursion => [MAIN_RECURSION]);

pub struct MainRecursion {
    has_no_std_attr: bool,
}

impl MainRecursion {
    pub fn new(tcx: TyCtxt<'_>) -> Self {
        Self {
            has_no_std_attr: find_attr!(tcx, crate, NoStd),
        }
    }
}

impl LateLintPass<'_> for MainRecursion {
    fn check_expr_post(&mut self, cx: &LateContext<'_>, expr: &Expr<'_>) {
        if self.has_no_std_attr {
            return;
        }

        if let ExprKind::Call(func, []) = &expr.kind
            && let ExprKind::Path(QPath::Resolved(_, path)) = &func.kind
            && let Some(def_id) = path.res.opt_def_id()
            && is_entrypoint_fn(cx, def_id)
        {
            span_lint_and_help(
                cx,
                MAIN_RECURSION,
                func.span,
                format!("recursing into entrypoint `{}`", snippet(cx, func.span, "main")),
                None,
                "consider using another function for this recursion",
            );
        }
    }
}
