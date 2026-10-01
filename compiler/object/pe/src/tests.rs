use vun_bytes::Out;

use super::*;

/// Where the sections of the image `image` builds are loaded, and what its headers say.
const BASE: u64 = 0x1_4000_0000;
const TEXT_RVA: u32 = 0x1000;
const DATA_RVA: u32 = 0x2000;
/// Where the file holds each section's bytes.
const TEXT_RAW: u32 = 0x200;
const DATA_RAW: u32 = 0x400;

/// `bytes` written into `buffer` at `at`.
fn put(buffer: &mut [u8], at: usize, bytes: &[u8]) {
    buffer[at..at + bytes.len()].copy_from_slice(bytes);
}

/// A 64-bit program of two sections, code at 0x1000 and data at 0x2000: `main` is exported as
/// ordinal 1 and `helper` by ordinal 2 alone, and one function is imported by name and one by
/// ordinal from `kernel32.dll`.
fn image(library: bool) -> Vec<u8> {
    let mut data = vec![0u8; 0x200];
    // The export directory at 0x2000: ordinals from 1, two functions, one name.
    let mut directory = Out::default();
    directory.zeros(12);
    directory.u32(DATA_RVA + 0x100); // the library's name
    directory.u32(1); // the ordinal base
    directory.u32(2); // functions
    directory.u32(1); // names
    directory.u32(DATA_RVA + 0x40); // the address of functions
    directory.u32(DATA_RVA + 0x50); // of names
    directory.u32(DATA_RVA + 0x58); // of ordinals
    put(&mut data, 0, &directory.0);
    put(&mut data, 0x40, &(TEXT_RVA).to_le_bytes());
    put(&mut data, 0x44, &(TEXT_RVA + 4).to_le_bytes());
    put(&mut data, 0x50, &(DATA_RVA + 0x60).to_le_bytes());
    put(&mut data, 0x58, &0u16.to_le_bytes());
    put(&mut data, 0x60, b"main\0");
    put(&mut data, 0x100, b"app.dll\0");
    // The import descriptor at 0x2080, then a zero one: lookup table at 0x20c0, slots at 0x20e0.
    let mut descriptor = Out::default();
    descriptor.u32(DATA_RVA + 0xc0);
    descriptor.zeros(8);
    descriptor.u32(DATA_RVA + 0x110);
    descriptor.u32(DATA_RVA + 0xe0);
    put(&mut data, 0x80, &descriptor.0);
    put(&mut data, 0xc0, &(u64::from(DATA_RVA) + 0x120).to_le_bytes());
    put(&mut data, 0xc8, &((1u64 << 63) | 7).to_le_bytes());
    put(&mut data, 0x110, b"kernel32.dll\0");
    put(&mut data, 0x120, &[0, 0]);
    put(&mut data, 0x122, b"WriteFile\0");
    let code = [0x55, 0x48, 0x89, 0xe5, 0x5d, 0xc3, 0x90, 0x90];
    let mut out = Out::default();
    // The DOS header, the PE header's place at 0x40, and the signature.
    out.u16(0x5a4d);
    out.zeros(0x3a);
    out.u32(0x40);
    out.bytes(b"PE\0\0");
    // The file header: AMD64, two sections, and an optional header of 240 bytes.
    out.u16(0x8664);
    out.u16(2);
    out.zeros(12);
    out.u16(240);
    out.u16(if library { 0x2022 } else { 0x0022 });
    // The optional header.
    out.u16(0x020b);
    out.zeros(14);
    out.u32(TEXT_RVA); // the entry
    out.u32(TEXT_RVA); // the base of code
    out.u64(BASE);
    out.zeros(76);
    out.u32(16); // directories
    let directories = out.here();
    out.zeros(16 * 8);
    let at = usize::try_from(directories).expect("a small offset");
    out.0[at..at + 8].copy_from_slice(&[0, 0x20, 0, 0, 0x40, 0, 0, 0]);
    out.0[at + 8..at + 16].copy_from_slice(&[0x80, 0x20, 0, 0, 0x28, 0, 0, 0]);
    // The section headers.
    for (name, rva, raw, size, flags) in [
        (b".text\0\0\0", TEXT_RVA, TEXT_RAW, 8u32, 0x6000_0020u32),
        (b".rdata\0\0", DATA_RVA, DATA_RAW, 0x200, 0x4000_0040),
    ] {
        out.bytes(name);
        out.u32(size);
        out.u32(rva);
        out.u32(size);
        out.u32(raw);
        out.zeros(12);
        out.u32(flags);
    }
    out.zeros(usize::try_from(TEXT_RAW).expect("small") - out.0.len());
    out.bytes(&code);
    out.zeros(usize::try_from(DATA_RAW).expect("small") - out.0.len());
    out.bytes(&data);
    out.0
}

#[test]
fn the_headers_say_what_the_image_is_and_where_it_starts() {
    let image = Image::read(&image(false)).unwrap();
    assert_eq!((image.machine, image.wide, image.library), (Machine::X86_64, true, false));
    assert_eq!((image.base, image.entry), (BASE, BASE + u64::from(TEXT_RVA)));
    assert!(Image::read(&self::image(true)).unwrap().library);
}

#[test]
fn sections_are_loaded_at_their_addresses_with_what_the_file_holds() {
    let image = Image::read(&image(false)).unwrap();
    let names: Vec<_> =
        image.sections.iter().map(|s| (s.name.as_slice(), s.address, s.executable())).collect();
    assert_eq!(names, [(&b".text"[..], BASE + 0x1000, true), (&b".rdata"[..], BASE + 0x2000, false)]);
    assert_eq!(image.sections[0].data, [0x55, 0x48, 0x89, 0xe5, 0x5d, 0xc3, 0x90, 0x90]);
}

#[test]
fn exports_are_named_by_name_or_by_ordinal_alone() {
    let image = Image::read(&image(false)).unwrap();
    assert_eq!(
        image.exports,
        [
            Export { name: Some(b"main".to_vec()), ordinal: 1, address: BASE + 0x1000, forwarder: None },
            Export { name: None, ordinal: 2, address: BASE + 0x1004, forwarder: None },
        ]
    );
}

#[test]
fn imports_name_the_library_and_the_slot_the_loader_fills() {
    let image = Image::read(&image(false)).unwrap();
    assert_eq!(
        image.imports,
        [
            Import {
                library: b"kernel32.dll".to_vec(),
                name: Some(b"WriteFile".to_vec()),
                ordinal: None,
                slot: BASE + 0x20e0,
            },
            Import { library: b"kernel32.dll".to_vec(), name: None, ordinal: Some(7), slot: BASE + 0x20e8 },
        ]
    );
}

#[test]
fn bytes_that_are_no_pe_file_are_refused() {
    assert_eq!(Image::read(b"ELF"), Err(Malformed::Dos));
    assert_eq!(Image::read(b"M"), Err(Malformed::Truncated(Truncated { at: 2, end: 1 })));
    let mut bytes = image(false);
    bytes[0x40] = b'X';
    assert_eq!(Image::read(&bytes), Err(Malformed::Signature));
    let mut bytes = image(false);
    bytes[0x58] = 0x0b;
    bytes[0x59] = 0x05;
    assert_eq!(Image::read(&bytes), Err(Malformed::Magic(0x050b)));
    assert!(matches!(Image::read(&image(false)[..0x300]), Err(Malformed::Truncated(_))));
}
