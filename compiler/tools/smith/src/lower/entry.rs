//! The entry of a program: `start`, and `main`, which folds what each root gives into a hash it
//! prints, so that two builds that print alike agree on every value the roots made.

use vun_grammar::TokenKind;
use vun_items::lang::float;
use vun_items::std::{fmt, int_Narrow, os, os_Exit, os_System};
use vun_syntax::make::{Arm, Bind, Block, Borrow, Expr as Source, Function, Item, Param};

use super::{Lowering, names};
use crate::int::Range;
use crate::ir::{Fold, Piece, Step, WORD};
use crate::ty::Ty;

use super::{HASH_OFFSET as OFFSET, HASH_PRIME as PRIME};

impl Lowering<'_> {
    /// `start := std.os.start(main);`
    pub(super) fn start() -> Item {
        Bind::new("start", Source::call(Source::decl(os::start), vec![Source::local("main")])).item()
    }

    /// `main := (sys: &mut System) -> Exit { ... }`
    pub(super) fn main(&self) -> Item {
        let hash = Source::local("h");
        let word = Range::unsigned(WORD);
        let mut stmts = vec![
            Source::bind(Bind {
                mutable: true,
                ..Bind::new("out", Source::method(Source::local("sys"), os_System::stdout.name(), Vec::new()))
            }),
            Source::bind(Bind::mutable("h", self.range(&word), Source::int(OFFSET))),
        ];
        for (n, &root) in self.program.roots.iter().enumerate() {
            let value = names::root(n);
            stmts.push(Source::bind(Bind::new(
                &value,
                Source::call(Source::local(&names::function(root)), Vec::new()),
            )));
            self.fold(
                &hash,
                &Source::local(&value),
                &self.program.functions[root].result.fold(),
                0,
                &mut stmts,
            );
            let out = Source::borrow(Borrow::MUT, Source::local("out"));
            let format = Source::String("{}".to_owned());
            stmts.push(Source::call(Source::decl(fmt::println), vec![out, format, hash.clone()]));
        }
        let system = Source::borrow(Borrow::MUT, Source::decl(os::System));
        let main = Function {
            params: vec![Param::named("sys", system)],
            result: Some(Box::new(Source::decl(os::Exit))),
            body: Block::new(stmts, Some(Source::DotName(os_Exit::ok.name().to_owned()))),
        };
        Bind::new("main", Source::Function(main)).item()
    }

    /// Statements folding the value at `place` into the hash, as `fold` says, `depth` enums down.
    fn fold(&self, hash: &Source, place: &Source, fold: &Fold, depth: usize, out: &mut Vec<Source>) {
        let word = Range::unsigned(WORD);
        match fold {
            Fold::Int(range) => {
                for &piece in Fold::pieces(range) {
                    let part = match piece {
                        Piece::Low if range.within(&word) => place.clone(),
                        Piece::Low => self.wrapped(place.clone(), &word),
                        Piece::High => {
                            let high = Source::infix(
                                TokenKind::Slash,
                                place.clone(),
                                Self::integer(&Piece::word_size()),
                            );
                            self.wrapped(high, &word)
                        }
                    };
                    Self::mix(hash, part, out);
                }
            }
            Fold::Float(format) => {
                let ty = self.ty(&Ty::Float(*format));
                let bits = Source::call(Source::decl(float::bits), vec![ty, place.clone()]);
                Self::mix(hash, self.wrapped(bits, &word), out);
            }
            Fold::Bool => {
                let zero = Source::Block(Block::of(Source::int(0u32)));
                Self::mix(hash, Source::when(place.clone(), Block::of(Source::int(1u32)), Some(zero)), out);
            }
            Fold::Parts(parts) => {
                for (step, part) in parts {
                    let at = match *step {
                        Step::Field(i) => {
                            Source::field_at(place.clone(), u32::try_from(i).expect("a tuple has few parts"))
                        }
                        Step::Member(i) => Source::member(place.clone(), &names::field(i)),
                        Step::Item(i) => Source::index(place.clone(), Source::int(i)),
                    };
                    self.fold(hash, &at, part, depth, out);
                }
            }
            Fold::Variants(nominal, variants) => {
                let arms = variants
                    .iter()
                    .enumerate()
                    .map(|(number, payload)| {
                        let mut stmts = Vec::new();
                        Self::mix(hash, Source::int(number), &mut stmts);
                        let binds = (0..payload.len()).map(|i| names::binder(depth, i));
                        let pattern = Source::variant(
                            &names::variant(*nominal, number),
                            binds.clone().map(|b| Source::local(&b)).collect(),
                        );
                        for (bind, part) in binds.zip(payload) {
                            self.fold(hash, &Source::local(&bind), part, depth + 1, &mut stmts);
                        }
                        Arm { pattern, guard: None, value: Some(Source::block(stmts, None)) }
                    })
                    .collect();
                out.push(Source::matching(place.clone(), arms));
            }
        }
    }

    /// `hash = hash * prime + part`, wrapping.
    fn mix(hash: &Source, part: Source, out: &mut Vec<Source>) {
        out.push(Source::assign(TokenKind::StarPercentEq, hash.clone(), Source::int(PRIME)));
        out.push(Source::assign(TokenKind::PlusPercentEq, hash.clone(), part));
    }

    /// `value.wrap(to)`
    fn wrapped(&self, value: Source, to: &Range) -> Source {
        Source::method(value, int_Narrow::wrap.name(), vec![self.range(to)])
    }
}
