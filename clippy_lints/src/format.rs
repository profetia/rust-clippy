use clippy_utils::diagnostics::span_lint_and_sugg;
use clippy_utils::get_parent_expr;
use clippy_utils::macros::{FormatArgsStorage, find_format_arg_expr, first_node_in_macro};
use clippy_utils::source::{SpanExt as _, snippet_with_context};
use clippy_utils::sugg::Sugg;
use rustc_ast::{FormatArgsPiece, FormatOptions, FormatTrait};
use rustc_data_structures::fx::FxHashMap;
use rustc_errors::Applicability;
use rustc_hir::def_id::DefId;
use rustc_hir::{BorrowKind, Expr, ExprKind, Mutability};
use rustc_lint::{LateContext, LateLintPass, impl_lint_pass};
use rustc_middle::ty;
use rustc_span::hygiene::{ExpnId, ExpnKind, SyntaxContext};
use rustc_span::{Span, sym};

declare_clippy_lint! {
    /// ### What it does
    /// Checks for the use of `format!("string literal with no
    /// argument")` and `format!("{}", foo)` where `foo` is a string.
    ///
    /// ### Why is this bad?
    /// There is no point of doing that. `format!("foo")` can
    /// be replaced by `"foo".to_owned()` if you really need a `String`. The even
    /// worse `&format!("foo")` is often encountered in the wild. `format!("{}",
    /// foo)` can be replaced by `foo.clone()` if `foo: String` or `foo.to_owned()`
    /// if `foo: &str`.
    ///
    /// ### Examples
    /// ```no_run
    /// let foo = "foo";
    /// format!("{}", foo);
    /// ```
    ///
    /// Use instead:
    /// ```no_run
    /// let foo = "foo";
    /// foo.to_owned();
    /// ```
    #[clippy::version = "pre 1.29.0"]
    pub USELESS_FORMAT,
    complexity,
    "useless use of `format!`"
}

impl_lint_pass!(UselessFormat => [USELESS_FORMAT]);

type RootMacroCall = (DefId, ExpnId, Span);

pub struct UselessFormat {
    format_args: FormatArgsStorage,
    root_macro_calls: FxHashMap<ExpnId, Option<RootMacroCall>>,
}

impl UselessFormat {
    pub fn new(format_args: FormatArgsStorage) -> Self {
        Self {
            format_args,
            root_macro_calls: FxHashMap::default(),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for UselessFormat {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        // Loosened from `root_macro_call_first_node` so the lint also fires when `format!` is
        // the tail of a block emitted by another macro. The `!= macro_call.expn` check filters
        // HIR nodes inside `format!`'s own expansion (its outer block's tail, its nested
        // `format_args!`), which would otherwise also pass `first_node_in_macro` and cause the
        // lint to fire multiple times per call.
        //
        // `first_node_in_macro` is checked first because it is cheaper.
        if let Some(p_expn) = first_node_in_macro(cx, expr)
            && let Some((def_id, macro_expn, call_site)) = root_macro_call(expr.span, &mut self.root_macro_calls)
            && cx.tcx.is_diagnostic_item(sym::format_macro, def_id)
            && p_expn != macro_expn
            && let Some(format_args) = self.format_args.get(cx, expr, macro_expn)
        {
            let mut applicability = Applicability::MachineApplicable;

            match (format_args.arguments.all_args(), &format_args.template[..]) {
                ([], []) => span_lint(
                    cx,
                    call_site,
                    "`String::new()`",
                    "String::new()".to_owned(),
                    applicability,
                ),
                ([], [_]) => {
                    // Simulate macro expansion, converting {{ and }} to { and }.
                    let Some(snippet) = format_args.span.get_text(cx) else {
                        return;
                    };
                    let s_expand = snippet.replace("{{", "{").replace("}}", "}");
                    // If the format! is immediately borrowed as `&format!("literal")`,
                    // suggest using the string literal directly (no `.to_string()` needed).
                    if let Some(parent) = get_parent_expr(cx, expr)
                        && let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Not, _) = parent.kind
                    {
                        span_lint(cx, parent.span, "the string literal directly", s_expand, applicability);
                    } else {
                        let sugg = format!("{s_expand}.to_string()");
                        span_lint(cx, call_site, "`.to_string()`", sugg, applicability);
                    }
                },
                ([arg], [piece]) => {
                    if let Some(value) = find_format_arg_expr(expr, arg)
                        && let FormatArgsPiece::Placeholder(placeholder) = piece
                        && placeholder.format_trait == FormatTrait::Display
                        && placeholder.format_options == FormatOptions::default()
                        && match cx.typeck_results().expr_ty(value).peel_refs().kind() {
                            ty::Adt(adt, _) => Some(adt.did()) == cx.tcx.lang_items().string(),
                            ty::Str => true,
                            _ => false,
                        }
                    {
                        let is_new_string = match value.kind {
                            ExprKind::Binary(..) => true,
                            ExprKind::MethodCall(path, ..) => path.ident.name == sym::to_string,
                            _ => false,
                        };
                        // If the arg is `&str` (not a newly-constructed string) and the format! is
                        // immediately borrowed as `&format!("{}", lit)`, suggest the arg directly.
                        // This avoids producing `&lit.to_string()` which `unnecessary_to_owned` would flag.
                        if !is_new_string
                            && matches!(cx.typeck_results().expr_ty(value).peel_refs().kind(), ty::Str)
                            && let Some(parent) = get_parent_expr(cx, expr)
                            && let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Not, _) = parent.kind
                        {
                            let sugg = Sugg::hir_with_context(cx, value, call_site.ctxt(), "<arg>", &mut applicability);
                            span_lint(
                                cx,
                                parent.span,
                                "the argument directly",
                                sugg.to_string(),
                                applicability,
                            );
                        } else {
                            let sugg = if is_new_string {
                                snippet_with_context(cx, value.span, call_site.ctxt(), "..", &mut applicability)
                                    .0
                                    .into_owned()
                            } else {
                                let sugg =
                                    Sugg::hir_with_context(cx, value, call_site.ctxt(), "<arg>", &mut applicability);
                                format!("{}.to_string()", sugg.maybe_paren())
                            };
                            span_lint(cx, call_site, "`.to_string()`", sugg, applicability);
                        }
                    }
                },
                _ => {},
            }
        }
    }
}

fn root_macro_call(mut span: Span, cache: &mut FxHashMap<ExpnId, Option<RootMacroCall>>) -> Option<RootMacroCall> {
    let mut uncached = Vec::new();
    let mut root_macro = None;

    let root_macro = loop {
        let ctxt = span.ctxt();
        if ctxt == SyntaxContext::root() {
            break root_macro;
        }

        let expn = ctxt.outer_expn();
        if let Some(&cached) = cache.get(&expn) {
            // A macro in the cached outer suffix is the root even if the uncached inner prefix
            // also contains macros. If the suffix contains none, keep the outermost macro found
            // in the prefix.
            break cached.or(root_macro);
        }

        uncached.push(expn);
        let data = expn.expn_data();
        if matches!(data.kind, ExpnKind::Macro(..))
            && let Some(def_id) = data.macro_def_id
        {
            root_macro = Some((def_id, expn, data.call_site));
        }
        span = data.call_site;
    };

    // Every expansion visited above has the same root macro. Caching the whole path lets spans
    // with different inner expansions share their usually much longer outer expansion suffix.
    for expn in uncached {
        cache.insert(expn, root_macro);
    }
    root_macro
}

fn span_lint(cx: &LateContext<'_>, span: Span, expl: &str, sugg: String, applicability: Applicability) {
    span_lint_and_sugg(
        cx,
        USELESS_FORMAT,
        span,
        "useless use of `format!`",
        format!("consider using {expl}"),
        sugg,
        applicability,
    );
}
