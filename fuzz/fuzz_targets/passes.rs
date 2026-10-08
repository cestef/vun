//! A function generated from the input, simplified and then legalized for a backend computing
//! with 32 bits, gives LIR that verifies after each (the passes check themselves in debug
//! builds), never a panic.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_lir::{Scalar, Unit};
use vun_lir_passes::Simplify;
use vun_lir_passes::legalize::{Division, Half, Legalize};
use vun_origin::OriginTable;
use vun_regalloc::sample::Random;
use vun_span::{SourceFiles, Span};
use vun_target::{Convention, Endian, Floats};

fuzz_target!(|data: &[u8]| {
    let [calls, seed @ ..] = data else { return };
    let seed = seed.iter().fold(0x9E37_79B9_7F4A_7C15_u64, |h, &b| h.rotate_left(8) ^ u64::from(b));
    let origins = OriginTable::new();
    let origin = origins.source(Span::new(SourceFiles::new().add("fuzz.vun", "f := () {};"), 0, 1));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(Random(seed.max(1)).function(calls % 2 == 0, origin));
    if let Err(malformed) = unit.verify(&origins) {
        panic!("the generated function: {malformed:?}");
    }
    unit.simplify();
    if let Err(malformed) = unit.verify(&origins) {
        panic!("simplified: {malformed:?}");
    }
    let narrow = Legalize {
        widest: Scalar::I32,
        endian: Endian::Little,
        half: Half::Promoted,
        floats: Floats::ALL,
        convention: Convention::Aapcs,
        division: Division::Restoring,
    };
    if narrow.apply(&mut unit, &|_| false).is_ok()
        && let Err(malformed) = unit.verify(&origins)
    {
        panic!("legalized: {malformed:?}");
    }
});
