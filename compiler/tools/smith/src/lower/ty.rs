//! Types as source.

use vun_grammar::TokenKind;
use vun_items::{Item, lang, std};
use vun_syntax::make::{Borrow, Expr};

use super::{Lowering, Spelling, names};
use crate::float::Format;
use crate::int::{BigInt, Range, Width};
use crate::ir::{Bound, Function, Role};
use crate::ty::{Nominal, Ty};

/// The integer types `lang.int` names, by width.
const NAMED: [(Width, Item); 10] = [
    (Width { signed: false, bits: 8 }, lang::int::u8),
    (Width { signed: false, bits: 16 }, lang::int::u16),
    (Width { signed: false, bits: 32 }, lang::int::u32),
    (Width { signed: false, bits: 64 }, lang::int::u64),
    (Width { signed: false, bits: 128 }, lang::int::u128),
    (Width { signed: true, bits: 8 }, lang::int::i8),
    (Width { signed: true, bits: 16 }, lang::int::i16),
    (Width { signed: true, bits: 32 }, lang::int::i32),
    (Width { signed: true, bits: 64 }, lang::int::i64),
    (Width { signed: true, bits: 128 }, lang::int::i128),
];

impl Lowering<'_> {
    /// A declaration the prelude has in scope, written as the spelling says.
    pub(super) fn prelude(&self, item: Item) -> Expr {
        match self.spelling {
            Spelling::Prelude => Expr::local(item.name()),
            Spelling::Qualified => Expr::decl(item),
        }
    }

    /// What a `$` parameter is declared to be: `type`, or the callable its argument must be.
    pub(super) fn bound(&self, bound: &Bound) -> Expr {
        match bound {
            Bound::Any => Expr::local("type"),
            Bound::Call(taken, given) => Expr::call(
                Expr::decl(lang::ops::Call),
                vec![Expr::Tuple(taken.iter().map(|t| self.ty(t)).collect()), self.ty(given)],
            ),
        }
    }

    /// What a function is written to give: its result, or the iterator of what it yields.
    pub(super) fn result(&self, function: &Function) -> Expr {
        match function.role {
            Role::Plain => self.ty(&function.result),
            Role::Iterator => Expr::call(self.prelude(std::iter::Iter), vec![self.ty(&function.result)]),
        }
    }

    /// The type as a type expression.
    pub(super) fn ty(&self, ty: &Ty) -> Expr {
        match ty {
            Ty::Int(range) => self.range(range),
            Ty::Bool => self.prelude(lang::bool),
            Ty::Float(Format::F16) => self.prelude(lang::f16),
            Ty::Float(Format::F32) => self.prelude(lang::f32),
            Ty::Float(Format::F64) => self.prelude(lang::f64),
            Ty::Unit => Expr::Unit,
            Ty::Tuple(parts) => Expr::Tuple(parts.iter().map(|p| self.ty(p)).collect()),
            Ty::Array(len, item) => Expr::ArrayType(Box::new(Expr::int(*len)), Box::new(self.ty(item))),
            Ty::Closure(..) => Expr::Hole,
            Ty::Param(at) => Expr::local(&names::generic(*at)),
            Ty::Ref(mutable, inner) => {
                let mode = if *mutable { Borrow::MUT } else { Borrow::SHARED };
                Expr::borrow(mode, self.ty(inner))
            }
            Ty::Struct(id, _) => Expr::local(&names::ty(*id, true)),
            Ty::Enum(nominal, _) => self.nominal(*nominal, ty),
        }
    }

    /// The integer type of `range`: the name `lang.int` gives it, or `Int(lo..=hi)`.
    pub(super) fn range(&self, range: &Range) -> Expr {
        if let Some(width) = range.width()
            && let Some((_, item)) = NAMED.iter().find(|(w, _)| *w == width)
        {
            return self.prelude(*item);
        }
        let bounds = Expr::infix(TokenKind::DotDotEq, Self::integer(&range.lo), Self::integer(&range.hi));
        Expr::call(self.prelude(lang::Int), vec![bounds])
    }

    /// The enum `ty`, of kind `nominal`, as a type expression: its name, or the constructor of
    /// `lang` applied to what it holds.
    pub(super) fn nominal(&self, nominal: Nominal, ty: &Ty) -> Expr {
        let Ty::Enum(_, variants) = ty else { return Expr::Hole };
        match nominal {
            Nominal::Declared(id) => Expr::local(&names::ty(id, false)),
            Nominal::Option => Expr::call(self.prelude(lang::Option), vec![self.ty(&variants[1][0])]),
            Nominal::Result => Expr::call(
                self.prelude(lang::Result),
                vec![self.ty(&variants[0][0]), self.ty(&variants[1][0])],
            ),
        }
    }

    /// An integer as a literal.
    pub(super) fn integer(n: &BigInt) -> Expr {
        let magnitude = u128::try_from(n.magnitude()).expect("an integer of the model fits 128 bits");
        Expr::Int(vun_syntax::make::Integer { negative: n.sign() == num_bigint::Sign::Minus, magnitude })
    }
}

impl Lowering<'_> {
    /// The declaration of the struct or enum `ty`, which the program names.
    pub(super) fn declaration(&self, ty: &Ty) -> Option<vun_syntax::make::Item> {
        use vun_syntax::make::{Bind, Field, Member, Variant};
        match ty {
            Ty::Struct(id, fields) => {
                let members = fields
                    .iter()
                    .enumerate()
                    .map(|(at, f)| Member::Field(Field::new(&names::field(at), self.ty(f))))
                    .collect();
                Some(Bind::new(&names::ty(*id, true), Expr::Struct(members)).item())
            }
            Ty::Enum(Nominal::Declared(id), variants) => {
                let variants = variants
                    .iter()
                    .enumerate()
                    .map(|(at, payload)| {
                        Variant::new(
                            &names::variant(Nominal::Declared(*id), at),
                            payload.iter().map(|p| self.ty(p)).collect(),
                        )
                    })
                    .collect();
                Some(Bind::new(&names::ty(*id, false), Expr::Enum(variants, Vec::new())).item())
            }
            _ => None,
        }
    }
}
