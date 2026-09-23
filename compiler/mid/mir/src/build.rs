//! Lowering a function literal's HIR into MIR (spec 13, section 3). Parameters and local
//! declarations become locals; any other name is a constant sema evaluates. A form not lowered
//! yet is [`Unbuilt`], at its origin, so that evaluation stops rather than guesses.

use std::collections::HashMap;
use std::sync::Arc;

use overt_grammar::TokenKind;
use overt_hir::resolve::Res;
use overt_hir::tree::{
    Arg, BinderId, BlockId as HirBlock, Entry, ExprId, ExprKind, Lit, PatKind, Stmt as HirStmt,
};
use overt_index::IndexVec;
use overt_origin::OriginId;
use overt_query::{Cx, Query};
use overt_sema::env::Env;
use overt_sema::eval::FnRef;
use overt_sema::int::Integer;
use overt_sema::ty::Value;

use crate::body::{
    Block, BlockId, Body, Local, LocalDecl, Operand, Rvalue, Stmt, Terminator, TerminatorKind,
};
use crate::place::Place;

/// A form MIR does not lower yet, by where it is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Unbuilt {
    pub at: OriginId,
}

/// The MIR of a function literal.
pub struct Mir;

impl Query for Mir {
    const NAME: &'static str = "mir";
    type Key = FnRef;
    type Value = Result<Arc<Body>, Unbuilt>;

    fn compute(cx: &Cx<'_>, f: &FnRef) -> Self::Value {
        let env = Env::of(cx, &f.file).expect("a function value's file is read");
        Builder::lower(&env, f.expr).map(Arc::new)
    }
}

struct Builder<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    locals: IndexVec<Local, LocalDecl>,
    blocks: IndexVec<BlockId, Block>,
    /// The block statements are appended to.
    current: BlockId,
    /// The local each binder of the body is stored in.
    bound: HashMap<BinderId, Local>,
}

impl Builder<'_, '_, '_> {
    fn lower(env: &Env<'_, '_>, f: ExprId) -> Result<Body, Unbuilt> {
        let hir = &env.parsed.hir;
        let origin = hir.exprs[f].origin;
        let ExprKind::Fn { params, body, .. } = &hir.exprs[f].kind else {
            return Err(Unbuilt { at: origin });
        };
        let mut locals = IndexVec::new();
        let ret = locals.push(LocalDecl { name: None, origin });
        let open = Terminator { kind: TerminatorKind::Return, origin };
        let mut blocks = IndexVec::new();
        let current = blocks.push(Block { stmts: Vec::new(), term: open });
        let mut b = Builder { env, locals, blocks, current, bound: HashMap::new() };
        let mut args = Vec::new();
        for param in params {
            let Entry::One(param) = param else { return Err(Unbuilt { at: origin }) };
            let pat = param.pat.ok_or(Unbuilt { at: param.origin })?;
            let PatKind::Bind(binder) = hir.pats[pat].kind else { return Err(Unbuilt { at: param.origin }) };
            args.push(b.bind(binder));
        }
        let result = b.block(*body)?;
        b.assign(Place { local: ret }, Rvalue::Use(result), origin);
        b.blocks[b.current].term = Terminator { kind: TerminatorKind::Return, origin };
        Ok(Body { locals: b.locals, params: args, ret, blocks: b.blocks, origin })
    }

    /// A local for `binder`, which names in the body read from now on.
    fn bind(&mut self, binder: BinderId) -> Local {
        let b = &self.env.parsed.hir.binders[binder];
        let local = self.locals.push(LocalDecl { name: Some(b.name), origin: b.origin });
        self.bound.insert(binder, local);
        local
    }

    fn assign(&mut self, place: Place, value: Rvalue, origin: OriginId) {
        self.blocks[self.current].stmts.push(Stmt { place, value, origin });
    }

    /// `value` in a new temporary.
    fn temp(&mut self, value: Rvalue, origin: OriginId) -> Operand {
        let local = self.locals.push(LocalDecl { name: None, origin });
        self.assign(Place { local }, value, origin);
        Operand::Place(Place { local })
    }

    /// The statements of `block`, then the value of its tail, or `()` without one.
    fn block(&mut self, block: HirBlock) -> Result<Operand, Unbuilt> {
        let hir = &self.env.parsed.hir;
        let origin = hir.blocks[block].origin;
        for stmt in &hir.blocks[block].stmts {
            let Entry::One(stmt) = stmt else { return Err(Unbuilt { at: origin }) };
            match stmt {
                HirStmt::Decl(d) => {
                    let decl = &hir.decls[*d];
                    let (PatKind::Bind(binder), Some(value)) = (&hir.pats[decl.pat].kind, decl.value) else {
                        return Err(Unbuilt { at: decl.origin });
                    };
                    let value = self.operand(value)?;
                    let local = self.bind(*binder);
                    self.assign(Place { local }, Rvalue::Use(value), decl.origin);
                }
                HirStmt::Expr(e) => {
                    self.operand(*e)?;
                }
                HirStmt::Block(inner) => {
                    self.block(*inner)?;
                }
                HirStmt::Assign { origin, .. } => return Err(Unbuilt { at: *origin }),
            }
        }
        match hir.blocks[block].tail {
            Some(tail) => self.operand(tail),
            None => Ok(Operand::Const(Value::Tuple(Vec::new()))),
        }
    }

    fn operand(&mut self, expr: ExprId) -> Result<Operand, Unbuilt> {
        let hir = &self.env.parsed.hir;
        let origin = hir.exprs[expr].origin;
        let unbuilt = Unbuilt { at: origin };
        match &hir.exprs[expr].kind {
            ExprKind::Lit(Lit::Int(n)) => Ok(Operand::Const(Value::Int(Integer::from(*n)))),
            ExprKind::Name(_) => match self.env.parsed.resolution.names[expr] {
                Some(Res::Binder(b)) if self.bound.contains_key(&b) => {
                    Ok(Operand::Place(Place { local: self.bound[&b] }))
                }
                _ => self.env.eval(expr).map(Operand::Const).map_err(|_| unbuilt),
            },
            ExprKind::Block(block) => self.block(*block),
            ExprKind::Infix { op: op @ (TokenKind::DotDot | TokenKind::DotDotEq), left, right } => {
                let lo = self.operand(*left)?;
                let hi = right.map(|r| self.operand(r)).transpose()?;
                Ok(self.temp(Rvalue::Range { op: *op, lo, hi }, origin))
            }
            ExprKind::Infix { op: TokenKind::And | TokenKind::Or, .. } => Err(unbuilt),
            ExprKind::Infix { op, left, right: Some(right) } => {
                let (left, right) = (self.operand(*left)?, self.operand(*right)?);
                Ok(self.temp(Rvalue::Binary { op: *op, left, right }, origin))
            }
            ExprKind::Prefix { op: op @ (TokenKind::Minus | TokenKind::Tilde), operand, .. } => {
                let operand = self.operand(*operand)?;
                Ok(self.temp(Rvalue::Unary { op: *op, operand }, origin))
            }
            ExprKind::Call { callee, args } => {
                let callee = self.operand(*callee)?;
                let args = args
                    .iter()
                    .map(|arg| match arg {
                        Entry::One(Arg { name: None, value }) => self.operand(*value),
                        _ => Err(unbuilt),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.temp(Rvalue::Call { callee, args }, origin))
            }
            // Anything else only as a constant sema can compute, such as `lang.int.u8`.
            _ => self.env.eval(expr).map(Operand::Const).map_err(|_| unbuilt),
        }
    }
}
