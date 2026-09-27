//! A function generated from the input, allocated on a machine of three to eight registers,
//! gives a plan the checker accepts.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_origin::OriginTable;
use vun_regalloc::Allocation;
use vun_regalloc::sample::{Random, Toy};
use vun_span::{SourceFiles, Span};

fuzz_target!(|data: &[u8]| {
    let [regs, preserved, seed @ ..] = data else { return };
    let regs = 3 + regs % 6;
    let preserved = preserved % (regs - 2);
    let seed = seed.iter().fold(0x9E37_79B9_7F4A_7C15_u64, |h, &b| h.rotate_left(8) ^ u64::from(b));
    let origins = OriginTable::new();
    let origin = origins.source(Span::new(SourceFiles::new().add("fuzz.vun", "f := () {};"), 0, 1));
    // Three arguments in the first three registers leave a fourth for the rest.
    let f = Random(seed.max(1)).function(regs > 3, origin);
    let toy = Toy::new(&f, regs, preserved);
    if let Err(wrong) = Allocation::of(&f, &toy).check(&f, &toy) {
        panic!("{wrong}");
    }
});
