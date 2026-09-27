//! Functions compiled through the framework and each architecture's instruction compilers,
//! checked by the instructions their listings show.

mod arm64;
mod x86_64;

use std::fmt::Display;

use vun_intern::Symbol;
use vun_lir::func::{Conv, Linkage, Signature};
use vun_lir::{AbiType, Block, Builder, Ext, Scalar, Target, Unit, Value};
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceFiles, Span};

use crate::{Code, Native};

/// `f(n: i64) -> i64` for `A`, its body built by `build` from `n`, compiled and listed.
fn compiled<A: Native>(build: impl FnOnce(&mut Builder, Value, OriginId)) -> Code<A> {
    let origins = OriginTable::new();
    let origin = origins.source(Span::new(SourceFiles::new().add("a.vun", "n"), 0, 1));
    let int = AbiType::Int(Scalar::I64, Ext::Sign);
    let signature = Signature { conv: Conv::C, params: vec![int.clone()], result: Some(int) };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, origin);
    let n = b.params(b.current())[0];
    build(&mut b, n, origin);
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    let f = unit.functions.push(b.finish());
    // The first convention the architecture follows.
    vun_codegen::compile::<A>(&unit, &unit.functions[f], A::CONVENTIONS[0].1, true).unwrap()
}

/// The instructions of `code`, as the listing prints them.
fn text<T: Display>(code: &vun_codegen::Compiled<T>) -> Vec<String> {
    code.code
        .listing
        .iter()
        .filter_map(|line| match line {
            vun_asm::Line::Inst { inst, .. } => Some(inst.to_string()),
            vun_asm::Line::Label(_) => None,
        })
        .collect()
}

fn to(block: Block) -> Target {
    Target { block, args: Vec::new() }
}
