//! The typist the driver installs: typing a body, a declaration or an expression with a `Typer`.

use std::sync::Arc;

use vun_hir::tree::{DeclId, ExprId, ExprKind};
use vun_hir::visit::Visitor;
use vun_sema::env::Env;
use vun_sema::typed::{Found, Stage, TypedBody, Typist};
use vun_sema_ty::Value;

use crate::Typer;
use crate::exprs::Exprs;

/// Types with a [`Typer`].
pub struct Typing;

impl Typist for Typing {
    fn function(&self, env: &Env<'_, '_>, literal: ExprId, args: &[Option<Value>]) -> TypedBody {
        let hir = &env.parsed.hir;
        let mut typer = Typer::new(env, Stage::Run);
        typer.function(literal, args, None, false);
        let mut exprs = Exprs(0);
        if let ExprKind::Fn { body, .. } = &hir.exprs[literal].kind {
            exprs.block(hir, *body);
        }
        typer.finish(exprs.0)
    }

    fn declaration(&self, env: &Env<'_, '_>, decl: DeclId) -> TypedBody {
        let mut typer = Typer::new(env, Stage::Compile);
        typer.decl(&env.parsed.hir, decl);
        typer.finish(0)
    }

    fn expression(&self, env: &Env<'_, '_>, expr: ExprId) -> Arc<Found> {
        let mut typer = Typer::new(env, Stage::Compile);
        typer.typed(expr);
        typer.finish(0).found
    }
}
