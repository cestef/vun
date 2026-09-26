use crate::imm::{fits_signed, fits_unsigned, sign_extend, truncate};

#[test]
fn immediates_fit_as_the_operation_reads_them() {
    assert!(fits_signed(-1, 8, 32));
    assert!(fits_signed(0xffff_ffff, 8, 32));
    assert!(!fits_signed(0xffff_ffff, 8, 64));
    assert!(fits_signed(127, 8, 64));
    assert!(!fits_signed(128, 8, 64));
    assert!(fits_signed(128, 8, 8));
    assert!(fits_unsigned(0xffff_ffff, 32, 64));
    assert!(!fits_unsigned(-1, 32, 64));
    assert!(fits_unsigned(-1, 32, 32));
    assert_eq!(truncate(-1, 16), 0xffff);
    assert_eq!(sign_extend(0x80, 8), -128);
}
