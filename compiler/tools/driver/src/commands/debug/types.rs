//! `vun debug types`: what typing found in each function of a file, plain, and in every
//! instantiation the program reaches, from any file: each expression's type and stage, the
//! untyped ones marked, and each operator without the function it calls.

use std::io::{self, Write};
use std::path::Path;

use vun_command::{Args, Outcome};
use vun_hir::db::Parsed;
use vun_hir::tree::ExprKind;
use vun_mir::{Programs, Roots};
use vun_sema::eval::Instantiation;
use vun_sema::typeck::{Operation, Stage, Typed};
use vun_span::SourceMap;
use vun_term::Shell;

use super::FILE;
use crate::Session;

/// How much of an expression's source a line shows.
const SHOWN: usize = 48;

pub(super) fn types(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let out = &mut shell.out;
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    let Some(file) = session.load(Path::new(path)) else { return Ok(session.outcome()) };
    let key = session.file_ref(file);
    let program = session.db.get::<Programs>(&(key, Roots::Functions));
    let plain = session.functions(file).into_iter().map(Instantiation::plain);
    let instantiations: Vec<Instantiation> = plain.chain(program.functions.iter().cloned()).collect();
    for i in instantiations {
        let Some(typed) = session.db.get::<Typed>(&i) else { continue };
        let Some(parsed) = session.db.get::<Parsed>(&i.f.literal.file) else { continue };
        let hir = &parsed.hir;
        let f = &i.f.literal;
        writeln!(
            out,
            "fn {} {:?}",
            session.sources.locate(session.origins.source_span(f.expr.origin)),
            i.args
        )?;
        let body = hir.exprs.iter().filter(|(e, _)| hir.enclosing_function(*e) == Some(f.expr.id));
        for (e, expr) in body {
            let span = session.origins.source_span(expr.origin);
            let at = session.sources.locate(span).at;
            let source: String =
                span.range.of(&session.sources.file(span.file).text).chars().take(SHOWN).collect();
            let source = source.replace('\n', " ");
            let known = match typed.types.get(&e) {
                Some(k) => {
                    format!("{} ({})", k.ty, if k.stage == Stage::Compile { "compile" } else { "run" })
                }
                None => "untyped".to_owned(),
            };
            let operator = matches!(expr.kind, ExprKind::Infix { .. } | ExprKind::Prefix { .. })
                && !typed.operators.contains_key(&Operation::Expr(e));
            let operator = if operator { ", no operator" } else { "" };
            writeln!(out, "  @{}:{} {source}: {known}{operator}", at.line, at.col)?;
        }
    }
    Ok(session.outcome())
}
