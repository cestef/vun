use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{AbiType, Builder, Conv, Ext, Linkage, Scalar, Signature, Terminator, Unit, Value};
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceFiles, Span};
use vun_target::{Atomics, CCompiler, CInt, CTypes, Endian, TargetSpec};

use crate::ast::{Ident, Names};
use crate::lack::Missing;
use crate::{CBackend, Lack};

fn origin(origins: &OriginTable) -> OriginId {
    let file = SourceFiles::new().add("a.vun", "f := () {};");
    origins.source(Span::new(file, 0, 1))
}

/// A 64-bit target whose C compiler has no atomics.
fn target() -> TargetSpec {
    let int = |bits, align| CInt { bits, align };
    let name = Symbol::intern;
    TargetSpec {
        arch: name("x86_64"),
        cpu: name("generic"),
        os: name("linux"),
        abi: name("gnu"),
        pointer_bits: 64,
        endian: Endian::Little,
        object_format: name("elf"),
        entry_symbol: None,
        default_backend: name("c"),
        default_linker: name("gnu"),
        c: CTypes {
            short: int(16, 2),
            int: int(32, 4),
            long: int(64, 8),
            long_long: int(64, 8),
            pointer_align: 8,
            compiler: CCompiler { command: name("cc"), flags: Vec::new(), atomics: Atomics::None },
        },
    }
}

struct NoLines;

impl vun_backend::Lines for NoLines {
    fn line(&self, _: OriginId) -> Option<vun_backend::Line> {
        None
    }
}

/// A unit exporting `symbol`, returning a constant of `ty`.
fn constant(symbol: &str, ty: Scalar, origin: OriginId) -> Unit {
    let result = if ty.is_int() { AbiType::Int(ty, Ext::Zero) } else { AbiType::Scalar(ty) };
    let signature = Signature { conv: Conv::Vun, params: Vec::new(), result: Some(result) };
    let mut b = Builder::new(Linkage::Export(Symbol::intern(symbol)), signature, origin);
    let v = b.constant(ty, 7, origin);
    b.terminate(Terminator::Return(Some(v)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

#[test]
fn what_c99_lacks_is_refused_where_it_is_needed() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let wide = CBackend::source(&constant("wide", Scalar::I128, at), &target(), &NoLines);
    assert_eq!(wide, Err(Lack { missing: Missing::Int128, at }));
    let keyword = CBackend::source(&constant("int", Scalar::I32, at), &target(), &NoLines);
    assert_eq!(keyword, Err(Lack { missing: Missing::Symbol(Symbol::intern("int")), at }));
    assert!(CBackend::source(&constant("seven", Scalar::I32, at), &target(), &NoLines).is_ok());
}

#[test]
fn own_names_step_aside_for_link_symbols() {
    let names = Names::new([Symbol::intern("v3"), Symbol::intern("v3_")]);
    assert_eq!(names.spell(Ident::value(Value::new(3))), "v3__");
    assert_eq!(names.spell(Ident::value(Value::new(4))), "v4");
    assert_eq!(names.spell(Ident::Symbol(Symbol::intern("v3"))), "v3");
}
