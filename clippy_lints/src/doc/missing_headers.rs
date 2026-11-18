use super::{DocHeaders, MISSING_ERRORS_DOC, MISSING_PANICS_DOC, MISSING_SAFETY_DOC, UNNECESSARY_SAFETY_DOC};
use clippy_utils::diagnostics::{span_lint, span_lint_and_note};
use clippy_utils::macros::{is_panic, root_macro_call_first_node};
use clippy_utils::res::MaybeDef as _;
use clippy_utils::ty::implements_trait_with_env;
use clippy_utils::visitors::for_each_expr;
use clippy_utils::{fulfill_or_allowed, is_doc_hidden, is_inside_always_const_context, return_ty};
use rustc_hir::{BodyId, ExprKind, FnSig, OwnerId, Safety};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::{Span, sym};
use std::ops::ControlFlow;

pub fn check(
    cx: &LateContext<'_>,
    owner_id: OwnerId,
    sig: FnSig<'_>,
    headers: DocHeaders,
    body_id: Option<BodyId>,
    check_private_items: bool,
) {
    if !check_private_items && !cx.effective_visibilities.is_exported(owner_id.def_id) {
        return; // Private functions do not require doc comments
    }

    // do not lint if any parent has `#[doc(hidden)]` attribute (#7347)
    if !check_private_items
        && cx
            .tcx
            .hir_parent_iter(owner_id.into())
            .any(|(id, _node)| is_doc_hidden(cx.tcx.hir_attrs(id)))
    {
        return;
    }

    let span = cx.tcx.def_span(owner_id);
    match (headers.safety, sig.header.safety()) {
        (false, Safety::Unsafe) => span_lint(
            cx,
            MISSING_SAFETY_DOC,
            span,
            "unsafe function's docs are missing a `# Safety` section",
        ),
        (true, Safety::Safe) => span_lint(
            cx,
            UNNECESSARY_SAFETY_DOC,
            span,
            "safe function's docs have unnecessary `# Safety` section",
        ),
        _ => (),
    }
    if !headers.panics
        && let Some(body_id) = body_id
        && let Some(panic_span) = find_panic(cx, body_id)
    {
        span_lint_and_note(
            cx,
            MISSING_PANICS_DOC,
            span,
            "docs for function which may panic missing `# Panics` section",
            Some(panic_span),
            "first possible panic found here",
        );
    }
    if !headers.errors {
        if return_ty(cx, owner_id).is_diag_item(cx, sym::Result) {
            span_lint(
                cx,
                MISSING_ERRORS_DOC,
                span,
                "docs for function returning `Result` missing `# Errors` section",
            );
        } else if let Some(body_id) = body_id
            && let Some(future) = cx.tcx.lang_items().future_trait()
            && let typeck = cx.tcx.typeck_body(body_id)
            && let body = cx.tcx.hir_body(body_id)
            && let ret_ty = typeck.expr_ty(body.value)
            && implements_trait_with_env(
                cx.tcx,
                ty::TypingEnv::non_body_analysis(cx.tcx, owner_id.def_id),
                ret_ty,
                future,
                Some(owner_id.def_id.to_def_id()),
                &[],
            )
            && let ty::Coroutine(_, subs) = ret_ty.kind()
            && subs.as_coroutine().return_ty().is_diag_item(cx, sym::Result)
        {
            span_lint(
                cx,
                MISSING_ERRORS_DOC,
                span,
                "docs for function returning `Result` missing `# Errors` section",
            );
        }
    }
}

fn find_panic(cx: &LateContext<'_>, body_id: BodyId) -> Option<Span> {
    let mut panic_span = None;
    let typeck = cx.tcx.typeck_body(body_id);
    for_each_expr(cx.tcx, cx.tcx.hir_body(body_id), |expr| {
        let macro_call = root_macro_call_first_node(cx, expr).filter(|macro_call| {
            is_panic(cx, macro_call.def_id)
                || matches!(
                    cx.tcx.get_diagnostic_name(macro_call.def_id),
                    Some(sym::assert_macro | sym::assert_eq_macro | sym::assert_ne_macro)
                )
        });

        // Check for `unwrap` and `expect` for both `Option` and `Result`.
        let panicking_method = match expr.kind {
            ExprKind::MethodCall(path, receiver, args, _)
                if matches!(path.ident.name, sym::unwrap | sym::expect)
                    && !receiver.span.from_expansion()
                    && args.iter().all(|arg| !arg.span.from_expansion()) =>
            {
                matches!(
                    typeck.expr_ty(receiver).peel_refs().opt_diag_name(cx),
                    Some(sym::Option | sym::Result)
                )
            },
            _ => false,
        };

        if macro_call.is_none() && !panicking_method || is_inside_always_const_context(cx.tcx, expr.hir_id) {
            return ControlFlow::<!>::Continue(());
        }

        if let Some(macro_call) = macro_call
            && !fulfill_or_allowed(cx, MISSING_PANICS_DOC, [expr.hir_id])
            && panic_span.is_none()
        {
            panic_span = Some(macro_call.span);
        }

        if panicking_method && !fulfill_or_allowed(cx, MISSING_PANICS_DOC, [expr.hir_id]) && panic_span.is_none() {
            panic_span = Some(expr.span);
        }

        // Visit all nodes to fulfill any `#[expect]`s after the first linted panic
        ControlFlow::<!>::Continue(())
    });
    panic_span
}
