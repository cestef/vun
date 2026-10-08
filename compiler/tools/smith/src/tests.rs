use vun_diag::{DiagCtxt, DiagCx};
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

use crate::Entropy;
use crate::draw::{Config, Draw};
use crate::eval::Machine;
use crate::lower::Spelling;

/// What the parser reads of `text`: a tree that verifies, and nothing reported.
fn parses(text: &str) {
    let file = SourceFiles::new().add("smith.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    assert_eq!(diag.error_count(), 0, "{text}");
    cst.verify(&tokens, text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
}

#[test]
fn made_programs_parse_as_made() {
    let config = Config::default();
    for seed in 0..200 {
        let program = Draw::program(&mut Entropy::seeded(seed), &config);
        for spelling in [Spelling::Prelude, Spelling::Qualified] {
            parses(&program.source(spelling));
        }
    }
}

#[test]
fn the_same_seed_makes_the_same_program() {
    let config = Config::default();
    let [a, b] = [0, 0].map(|_| Draw::program(&mut Entropy::seeded(7), &config).source(Spelling::Prelude));
    assert_eq!(a, b);
}

#[test]
fn bytes_make_programs_and_empty_ones_the_simplest() {
    let config = Config::default();
    for bytes in [&[][..], &[1, 2, 3, 4, 5, 6, 7, 8][..], &[255; 4096][..]] {
        parses(&Draw::program(&mut Entropy::bytes(bytes), &config).source(Spelling::Prelude));
    }
}

#[test]
fn made_programs_run_in_the_model_without_trapping() {
    let config = Config::default();
    for seed in 0..300 {
        let program = Draw::program(&mut Entropy::seeded(seed), &config);
        let hashes = Machine::new(&program, config.fuel()).hashes();
        assert!(hashes.is_ok(), "seed {seed}: {hashes:?}\n{}", program.source(Spelling::Prelude));
    }
}

#[test]
fn print_one() {
    let program = Draw::program(&mut Entropy::seeded(3), &Config::default());
    println!("{}", program.source(Spelling::Prelude));
}

/// The forms of every expression of `expr`, itself and what is in it.
fn forms(
    expr: &mut crate::ir::Expr,
    found: &mut std::collections::HashSet<std::mem::Discriminant<crate::ir::Kind>>,
) {
    found.insert(std::mem::discriminant(&expr.kind));
    if let crate::ir::Kind::Block(stmts, _) = &mut expr.kind {
        for stmt in stmts {
            if let crate::ir::Stmt::Let { init, .. } = stmt {
                forms(init, found);
            }
        }
    }
    for child in expr.children_mut() {
        forms(child, found);
    }
}

#[test]
fn the_programs_made_hold_every_form_of_expression() {
    use crate::ir::{Kind, TypeId};
    use vun_index::Idx;
    let config = Config::default();
    let mut found = std::collections::HashSet::new();
    let mut generic = false;
    for seed in 0..300 {
        let mut program = Draw::program(&mut Entropy::seeded(seed), &config);
        generic |= program.functions.iter().any(|(_, f)| !f.generics.is_empty());
        for (_, function) in program.functions.iter_mut() {
            forms(&mut function.tail, &mut found);
            for stmt in &mut function.body {
                if let crate::ir::Stmt::Let { init, .. } = stmt {
                    forms(init, &mut found);
                }
            }
        }
    }
    let wanted = [
        Kind::Lit(crate::value::Value::Unit),
        Kind::Struct(TypeId::new(0), Vec::new()),
        Kind::Variant(crate::ty::Nominal::Option, 0, Vec::new()),
        Kind::Match(Box::new(crate::ir::Expr::unit()), Vec::new()),
        Kind::TryTo(Box::new(crate::ir::Expr::unit())),
        Kind::UnwrapOr(Box::new(crate::ir::Expr::unit()), Box::new(crate::ir::Expr::unit())),
        Kind::IsSome(Box::new(crate::ir::Expr::unit())),
        Kind::Deref(Box::new(crate::ir::Expr::unit())),
        Kind::Borrow(
            true,
            crate::ir::Place { local: crate::ir::LocalId::new(0), deref: false, steps: Vec::new() },
        ),
        Kind::Closure(Box::new(crate::ir::Closure {
            id: 0,
            captures: Vec::new(),
            params: Vec::new(),
            result: None,
            body: crate::ir::Expr::unit(),
        })),
        Kind::CallClosure(Box::new(crate::ir::Expr::unit()), Vec::new()),
        Kind::Forever(crate::ir::Label(0), Vec::new()),
        Kind::Try(Box::new(crate::ir::Expr::unit())),
        Kind::TailCall(crate::ir::FnId::new(0), Vec::new()),
    ];
    assert!(generic, "no generic function among 300 programs");
    for kind in wanted {
        assert!(found.contains(&std::mem::discriminant(&kind)), "no {kind:?} among 300 programs");
    }
}

/// The forms of the statements in `stmts`, and of those inside them.
fn stmt_forms(
    stmts: &[crate::ir::Stmt],
    found: &mut std::collections::HashSet<std::mem::Discriminant<crate::ir::Stmt>>,
) {
    for stmt in stmts {
        found.insert(std::mem::discriminant(stmt));
        if let crate::ir::Stmt::Labeled(_, inner) | crate::ir::Stmt::Defer(inner) = stmt {
            found.insert(std::mem::discriminant(&**inner));
        }
        for body in stmt.bodies() {
            stmt_forms(body, found);
        }
    }
}

#[test]
fn the_programs_made_hold_every_form_of_statement() {
    use crate::ir::{Expr, LocalId, Stmt};
    use vun_index::Idx;
    let config = Config::default();
    let mut found = std::collections::HashSet::new();
    for seed in 0..300 {
        let program = Draw::program(&mut Entropy::seeded(seed), &config);
        for (_, function) in program.functions.iter() {
            stmt_forms(&function.body, &mut found);
        }
    }
    let local = LocalId::new(0);
    let wanted = [
        Stmt::WhileSome { local, option: Expr::unit(), body: Vec::new() },
        Stmt::Drain { local, function: crate::ir::FnId::new(0), args: Vec::new(), body: Vec::new() },
        Stmt::Yield(Expr::unit()),
        Stmt::Defer(Box::new(Stmt::Continue(None))),
        Stmt::While { condition: Expr::unit(), body: Vec::new() },
        Stmt::Labeled(crate::ir::Label(0), Box::new(Stmt::Continue(None))),
    ];
    for stmt in wanted {
        assert!(found.contains(&std::mem::discriminant(&stmt)), "no {stmt:?} among 300 programs");
    }
}
