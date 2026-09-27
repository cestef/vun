use vun_lir::analysis::{Cfg, Liveness, Loops, Order};
use vun_lir::{AbiType, Access, Alias, BinOp, Builder, Conv, Ext, Linkage, Scalar, Signature, Terminator};
use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};

use super::inline::Inlined;

#[test]
fn a_value_carrying_a_load_stays_before_a_later_store() {
    let origins = OriginTable::new();
    let at = origins.source(Span::new(SourceFiles::new().add("a.vun", "f := () {};"), 0, 1));
    let signature = Signature {
        conv: Conv::Vun,
        params: vec![AbiType::Address(Alias::Any)],
        result: Some(AbiType::Int(Scalar::I32, Ext::Zero)),
    };
    let mut b = Builder::new(Linkage::Internal, signature, at);
    let p = b.params(b.current())[0];
    let x = b.load(Scalar::I32, p, Access::Plain, at);
    let seven = b.constant(Scalar::I32, 7, at);
    let y = b.binary(BinOp::Xor, x, seven, at);
    let five = b.constant(Scalar::I32, 5, at);
    b.store(five, p, Access::Plain, at);
    b.terminate(Terminator::Return(Some(y)));
    let func = b.finish();
    let loops = Loops::of(&func, &Cfg::of(&func));
    let order = Order::of(&func, &loops);
    let inlined = Inlined::of(&func, &order, &Liveness::of(&func, &loops, &order));
    assert!(inlined.contains(x), "the load is read where `y` is computed");
    assert!(!inlined.contains(y), "`y` reads memory the store writes, so it is computed before it");
}
