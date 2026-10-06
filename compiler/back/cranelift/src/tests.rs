//! The backend's code for units of LIR: functions compile for each architecture it lists, what
//! it cannot express is refused where it is, and the object reads back.

use vun_asm::Reloc;
use vun_backend::Compiled;
use vun_backend::{Lack, Missing};
use vun_intern::Symbol;
use vun_lir::testing::{origin, sum};
use vun_lir::{AbiType, Builder, Conv, FuncId, InstKind, Linkage, Scalar, Signature, Terminator, Unit};
use vun_origin::OriginTable;
use vun_target::{Convention, Profile};

use crate::compile::Compilation;
use crate::failed::Failed;
use crate::isa::{Isa, NAMES};

/// What the assisting backend's code is, as far as these tests tell.
const ASSISTED: u8 = 0xC3;

fn convention(arch: &str) -> Convention {
    if arch == "arm64" { Convention::Aapcs64 } else { Convention::Sysv }
}

fn isa(arch: &str) -> Isa {
    let name = Symbol::intern;
    Isa::build(name(arch), name("linux"), name("elf"), convention(arch), Profile::Debug)
        .expect("elf is written")
}

/// A unit of one exported function of `signature`, holding `kind` and returning.
fn unit_of(signature: Signature, kind: Option<InstKind>) -> Unit {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    if let Some(kind) = kind {
        b.push(kind, None, at);
    }
    b.terminate(Terminator::Return(None));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

fn signature(conv: Conv, params: Vec<AbiType>) -> Signature {
    Signature { conv, params, result: None, variadic: None, convention: None }
}

/// What the backend lacks of `unit`, with nothing to assist it.
fn lacks(unit: &Unit) -> Option<Missing> {
    match Compilation::of(unit, &isa(NAMES[0].0), None, false) {
        Err(Failed::Lacks(Lack { missing, .. })) => Some(missing),
        _ => None,
    }
}

/// A block of inline assembly of no instructions, which no processor lacks and Cranelift cannot
/// express.
fn asm() -> InstKind {
    let block = vun_inline::Block {
        arch: vun_items::lang::arch::x86_64,
        insts: Vec::new(),
        returns: true,
        needs: Vec::new(),
    };
    InstKind::Asm(Box::new(block), None)
}

#[test]
fn a_loop_compiles_for_each_architecture() {
    let origins = OriginTable::new();
    let unit = sum(origin(&origins));
    for name in NAMES {
        let compiled =
            Compilation::of(&unit, &isa(name.0), None, true).unwrap_or_else(|_| panic!("{name:?}"));
        let code = &compiled.code.iter().next().expect("a function").1.code;
        assert!(!code.bytes.is_empty());
        assert!(
            code.relocs.is_empty(),
            "{:?}",
            code.relocs.iter().map(|r: &Reloc<_>| r.kind).collect::<Vec<_>>()
        );
        assert_eq!(compiled.listing.len(), 1);
    }
}

#[test]
fn what_cranelift_does_not_express_is_refused_where_it_is() {
    assert_eq!(lacks(&unit_of(signature(Conv::Vun, Vec::new()), Some(asm()))), Some(Missing::Asm));
    let float = AbiType::Scalar(Scalar::F64);
    let variadic = Signature { variadic: Some(0), ..signature(Conv::C, vec![float]) };
    assert_eq!(lacks(&unit_of(variadic, None)), Some(Missing::Variadic));
    let record = AbiType::Record(vun_lir::Record { size: 8, align: 8, fields: vec![(0, Scalar::I64)] });
    assert_eq!(lacks(&unit_of(signature(Conv::C, vec![record]), None)), Some(Missing::Record));
    let wide = AbiType::Wide { limb: Scalar::I64, limbs: 2, ext: vun_lir::Ext::Zero };
    assert_eq!(lacks(&unit_of(signature(Conv::C, vec![wide.clone()]), None)), Some(Missing::Int128));
    assert_eq!(lacks(&unit_of(signature(Conv::Vun, vec![wide]), None)), None);
}

#[test]
fn a_function_it_cannot_express_is_the_assisting_backends() {
    let unit = unit_of(signature(Conv::Vun, Vec::new()), Some(asm()));
    let assisted = |ids: &[FuncId]| {
        let code = |id| {
            let mut code = Compiled::unreached();
            code.code.bytes = vec![ASSISTED];
            (id, code)
        };
        Ok(ids.iter().copied().map(code).collect())
    };
    let compiled = Compilation::of(&unit, &isa(NAMES[0].0), Some(&assisted), false).expect("assisted");
    let (_, code) = compiled.code.iter().next().expect("a function");
    assert_eq!(code.code.bytes, [ASSISTED]);
}

#[test]
fn a_format_it_does_not_write_is_refused() {
    let name = Symbol::intern;
    let isa = Isa::build(name("x86_64"), name("linux"), name("macho"), Convention::Sysv, Profile::Debug);
    assert!(matches!(isa, Err(Missing::Format(_))));
}
