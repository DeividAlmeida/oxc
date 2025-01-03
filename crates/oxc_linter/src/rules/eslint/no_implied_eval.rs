use oxc_ast::{ast::{Argument, BinaryExpression, BindingPatternKind, Expression, IdentifierName, Statement}, AstKind};
use oxc_diagnostics::OxcDiagnostic;
use oxc_macros::declare_oxc_lint;
use oxc_span::{CompactStr, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    context::LintContext,
    fixer::{RuleFix, RuleFixer},
    rule::Rule,
    AstNode,
};
enum ImpliedEvalFunction {
    SetTimeout,
    SetInterval,
    ExecScript,
}

fn no_implied_eval_diagnostic(span: Span) -> OxcDiagnostic {
    // See <https://oxc.rs/docs/contribute/linter/adding-rules.html#diagnostics> for details
    OxcDiagnostic::warn("Should be an imperative statement about what is wrong")
        .with_help("Should be a command-like statement that tells the user how to fix the issue")
        .with_label(span)
}

#[derive(Debug, Default, Clone)]
pub struct NoImpliedEval;

declare_oxc_lint!(
    /// ### What it does
    ///
    ///
    /// ### Why is this bad?
    ///
    ///
    /// ### Examples
    ///
    /// Examples of **incorrect** code for this rule:
    /// ```js
    /// FIXME: Tests will fail if examples are missing or syntactically incorrect.
    /// ```
    ///
    /// Examples of **correct** code for this rule:
    /// ```js
    /// FIXME: Tests will fail if examples are missing or syntactically incorrect.
    /// ```
    NoImpliedEval,
    nursery, // TODO: change category to `correctness`, `suspicious`, `pedantic`, `perf`, `restriction`, or `style`
             // See <https://oxc.rs/docs/contribute/linter.html#rule-category> for details

    pending  // TODO: describe fix capabilities. Remove if no fix can be done,
             // keep at 'pending' if you think one could be added but don't know how.
             // Options are 'fix', 'fix_dangerous', 'suggestion', and 'conditional_fix_suggestion'
);

impl Rule for NoImpliedEval {
    // fn run<'a>(&self, node: &AstNode<'a>, ctx: &LintContext<'a>) {
    //     let kind = node.kind();
    //     dbg!(kind);
    // }

    fn run_once(&self, ctx: &LintContext) {
        let Some(root) = ctx.nodes().root_node() else {
            return;
        };
        dbg!(root.kind());
        let AstKind::Program(program) = root.kind() else { unreachable!() };

        let mut variable_names = FxHashSet::default();
        
        for statement in &program.body {
            match statement {
                Statement::ExpressionStatement(decl) => {
                    let Expression::CallExpression(call) = &decl.expression else {
                        continue;
                    };

                    let mut expr  = None;
                    match &call.callee {
                        Expression::StaticMemberExpression(ident) => {
                            if !ImpliedEvalFunction::is_implied_eval_function(&ident.property.name) {
                                continue;
                            }
                            expr = Some(&ident.property);
                        },
                        Expression::Identifier(ident) => {
                            if !ImpliedEvalFunction::is_implied_eval_function(&ident.name) {
                                continue;
                            }
                            expr = Some(&IdentifierName { name: &ident.name, span:ident.span });
                        },

                        _ => continue,
                    }

                    if !ImpliedEvalFunction::is_implied_eval_function(expr.unwrap().name.into()) {
                        continue;
                    }

                    let Some(arg) = call.arguments.first() else {
                        continue;
                    };

                    match arg {
                        Argument::StringLiteral(_)
                         | Argument::CallExpression(_) => ctx.diagnostic(no_implied_eval_diagnostic(expr.unwrap().span)),
                        Argument::Identifier(ident_ref) => {
                            if variable_names.contains(ident_ref.name.as_str()) {
                                ctx.diagnostic(no_implied_eval_diagnostic(ident_ref.span));
                            }
                        },
                        Argument::BinaryExpression(bin_expr) => {
                            if ImpliedEvalFunction::is_string_in_binary_expression(bin_expr) {
                                ctx.diagnostic(no_implied_eval_diagnostic(bin_expr.span));
                            }
                        }
                        _=> continue,
                    };
                },
                Statement::VariableDeclaration(decl) => {
                    decl.declarations.iter().for_each(|variable| {

                        if !matches!(&variable.init, Some(Expression::StringLiteral(_))) {
                            return;
                        }

                        let BindingPatternKind::BindingIdentifier(pointer) = &variable.id.kind else {
                            return;
                        };

                        variable_names.insert(CompactStr::new(pointer.name.as_str()));
                    });
                },
                _ => continue,
                
            }
        }
    }
}


impl ImpliedEvalFunction {
    fn from_str(name: &str) -> Option<Self> {
        match name {
            "setTimeout" => Some(ImpliedEvalFunction::SetTimeout),
            "setInterval" => Some(ImpliedEvalFunction::SetInterval),
            "execScript" => Some(ImpliedEvalFunction::ExecScript),
            _ => None,
        }
    }

    fn is_implied_eval_function(name: &str) -> bool {
        ImpliedEvalFunction::from_str(name).is_some()
    }

    fn is_string_in_binary_expression(expr: &BinaryExpression) -> bool {
        match (&expr.left, &expr.right) {
            (Expression::StringLiteral(_), _)
            | (_, Expression::StringLiteral(_))
            | (Expression::TemplateLiteral(_), _)
            | (_, Expression::TemplateLiteral(_)) => true,
            (Expression::BinaryExpression(expr), _)
            | (_,Expression::BinaryExpression(expr)) => ImpliedEvalFunction::is_string_in_binary_expression(expr),
            _ => false,
        }
    }
}

#[test]
fn test() {
    use crate::tester::Tester;

    let pass = vec![
        // "setTimeout();",
        // "setTimeout;",                     // { "globals": globals.browser },
        // "setTimeout = foo;",               // { "globals": globals.browser },
        // "window.setTimeout;",              // { "globals": globals.browser },
        // "window.setTimeout = foo;",        // { "globals": globals.browser },
        // "window['setTimeout'];",           // { "globals": globals.browser },
        // "window['setTimeout'] = foo;",     // { "globals": globals.browser },
        // "global.setTimeout;",              // { "sourceType": "commonjs" },
        // "global.setTimeout = foo;",        // { "sourceType": "commonjs" },
        // "global['setTimeout'];",           // { "sourceType": "commonjs" },
        // "global['setTimeout'] = foo;",     // { "sourceType": "commonjs" },
        // "globalThis['setTimeout'] = foo;", // { "ecmaVersion": 2020 },
        // "window.setTimeout('foo')",
        // "window.setInterval('foo')",
        // "window['setTimeout']('foo')",
        // "window['setInterval']('foo')",
        // "window.setTimeout('foo')",     // { "sourceType": "commonjs" },
        // "window.setInterval('foo')",    // { "sourceType": "commonjs" },
        // "window['setTimeout']('foo')",  // { "sourceType": "commonjs" },
        // "window['setInterval']('foo')", // { "sourceType": "commonjs" },
        // "global.setTimeout('foo')",     // { "globals": globals.browser },
        // "global.setInterval('foo')",    // { "globals": globals.browser },
        // "global['setTimeout']('foo')",  // { "globals": globals.browser },
        // "global['setInterval']('foo')", // { "globals": globals.browser },
        // "globalThis.setTimeout('foo')", // { "ecmaVersion": 6 },
        // "globalThis['setInterval']('foo')", // { "ecmaVersion": 2017 },
        // "window[`SetTimeOut`]('foo', 100);", // { "ecmaVersion": 6, "globals": globals.browser },
        // "global[`SetTimeOut`]('foo', 100);", // { "ecmaVersion": 6, "sourceType": "commonjs" },
        // "global[`setTimeout${foo}`]('foo', 100);", // { "ecmaVersion": 6, "globals": globals.browser },
        // "global[`setTimeout${foo}`]('foo', 100);", // { "ecmaVersion": 6, "sourceType": "commonjs" },
        // "globalThis[`setTimeout${foo}`]('foo', 100);", // { "ecmaVersion": 2020 },
        // "setTimeout(function() { x = 1; }, 100);",
        // "setInterval(function() { x = 1; }, 100)",
        // "execScript(function() { x = 1; }, 100)",
        // "window.setTimeout(function() { x = 1; }, 100);", // { "globals": globals.browser },
        // "window.setInterval(function() { x = 1; }, 100);", // { "globals": globals.browser },
        // "window.execScript(function() { x = 1; }, 100);", // { "globals": globals.browser },
        // "window.setTimeout(foo, 100);",                   // { "globals": globals.browser },
        // "window.setInterval(foo, 100);",                  // { "globals": globals.browser },
        // "window.execScript(foo, 100);",                   // { "globals": globals.browser },
        // "global.setTimeout(function() { x = 1; }, 100);", // { "sourceType": "commonjs" },
        // "global.setInterval(function() { x = 1; }, 100);", // { "sourceType": "commonjs" },
        // "global.execScript(function() { x = 1; }, 100);", // { "sourceType": "commonjs" },
        // "global.setTimeout(foo, 100);",                   // { "sourceType": "commonjs" },
        // "global.setInterval(foo, 100);",                  // { "sourceType": "commonjs" },
        // "global.execScript(foo, 100);",                   // { "sourceType": "commonjs" },
        // "globalThis.setTimeout(foo, 100);",               // { "ecmaVersion": 2020 },
        // "foo.setTimeout('hi')",
        // "setTimeout(foo, 10)",
        // "setInterval(1, 10)",
        // "execScript(2)",
        // "setTimeout(function() {}, 10)",
        // "foo.setInterval('hi')",
        // "setInterval(foo, 10)",
        // "setInterval(function() {}, 10)",
        // "foo.execScript('hi')",
        // "execScript(foo)",
        // "execScript(function() {})",
        // "setTimeout(foo + bar, 10)",
        // "setTimeout(foobar, 'buzz')",
        // "setTimeout(foobar, foo + 'bar')",
        // "setTimeout(function() { return 'foobar'; }, 10)",
        // "setTimeoutFooBar('Foo Bar')",
        // "foo.window.setTimeout('foo', 100);", // { "globals": globals.browser },
        // "foo.global.setTimeout('foo', 100);", // { "sourceType": "commonjs" },
        // "var window; window.setTimeout('foo', 100);", // { "globals": globals.browser },
        // "var global; global.setTimeout('foo', 100);", // { "sourceType": "commonjs" },
        // "function foo(window) { window.setTimeout('foo', 100); }", // { "globals": globals.browser },
        // "function foo(global) { global.setTimeout('foo', 100); }", // { "sourceType": "commonjs" },
        // "foo('', window.setTimeout);", // { "globals": globals.browser },
        // "foo('', global.setTimeout);", // { "sourceType": "commonjs" }
    ];

    let fail = vec![
        // r#"setTimeout("x = 1;");"#,
        // r#"setTimeout("x = 1;", 100);"#,
        // r#"setInterval("x = 1;");"#,
        // r#"execScript("x = 1;");"#,
        // "const s = 'x=1'; setTimeout(s, 100);", // { "ecmaVersion": 6 },
        // "setTimeout(String('x=1'), 100);",         // { "ecmaVersion": 6 },
        // "setTimeout('foo' + bar)",
        // "setTimeout(foo + 'bar')",
        // "setTimeout(`foo` + bar)", // { "ecmaVersion": 6 },
        // "setTimeout(1 + ';' + 1)",
        "window.setTimeout('foo')",             // { "globals": globals.browser },
        // "window.setInterval('foo')",            // { "globals": globals.browser },
        // "window['setTimeout']('foo')",          // { "globals": globals.browser },
        // "window['setInterval']('foo')",         // { "globals": globals.browser },
        // "window[`setInterval`]('foo')",         // { "ecmaVersion": 6, "globals": globals.browser },
        // "window.window['setInterval']('foo')",  // { "globals": globals.browser },
        // "global.setTimeout('foo')",             // { "sourceType": "commonjs" },
        // "global.setInterval('foo')",            // { "sourceType": "commonjs" },
        // "global['setTimeout']('foo')",          // { "sourceType": "commonjs" },
        // "global['setInterval']('foo')",         // { "sourceType": "commonjs" },
        // "global[`setInterval`]('foo')",         // { "ecmaVersion": 6, "sourceType": "commonjs" },
        // "global.global['setInterval']('foo')",  // { "sourceType": "commonjs" },
        // "globalThis.setTimeout('foo')",         // { "ecmaVersion": 2020 },
        // "globalThis.setInterval('foo')",        // { "ecmaVersion": 2020 },
        // "setTimeout(`foo${bar}`)",              // { "ecmaVersion": 6 },
        // "window.setTimeout(`foo${bar}`)",       // { "ecmaVersion": 6, "globals": globals.browser },
        // "window.window.setTimeout(`foo${bar}`)", // { "ecmaVersion": 6, "globals": globals.browser },
        // "global.global.setTimeout(`foo${bar}`)", // { "ecmaVersion": 6, "globals": globals.node },
        // "window.setTimeout('foo' + bar)", // { "globals": globals.browser },
        // "window.setTimeout(foo + 'bar')", // { "globals": globals.browser },
        // "window.setTimeout(`foo` + bar)", // { "ecmaVersion": 6, "globals": globals.browser },
        // "window.setTimeout(1 + ';' + 1)", // { "globals": globals.browser },
        // "window.window.setTimeout(1 + ';' + 1)", // { "globals": globals.browser },
        // "global.setTimeout('foo' + bar)", // { "sourceType": "commonjs" },
        // "global.setTimeout(foo + 'bar')", // { "sourceType": "commonjs" },
        // "global.setTimeout(`foo` + bar)", // { "ecmaVersion": 6, "sourceType": "commonjs" },
        // "global.setTimeout(1 + ';' + 1)", // { "sourceType": "commonjs" },
        // "global.global.setTimeout(1 + ';' + 1)", // { "sourceType": "commonjs" },
        // "globalThis.setTimeout('foo' + bar)", // { "ecmaVersion": 2020 },
        // "window?.setTimeout('code', 0)", // {                "ecmaVersion": 2020,                "globals": { "window": "readonly" }            },
        // "(window?.setTimeout)('code', 0)", // {                "ecmaVersion": 2020,                "globals": { "window": "readonly" }            }
    ];

    Tester::new(NoImpliedEval::NAME, NoImpliedEval::CATEGORY, pass, fail).test_and_snapshot();
}
