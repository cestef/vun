//! The PE reader accepts any bytes: it gives an image or says why not, copies no more of the
//! bytes than there are, and the image it gives lists as code.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_pe::Image;

fuzz_target!(|bytes: &[u8]| {
    let Ok(image) = Image::read(bytes) else { return };
    let copied: usize = image.sections.iter().map(|s| s.data.len()).sum();
    assert!(copied <= bytes.len(), "{copied} bytes copied out of {}", bytes.len());
    if let Ok(code) = vun_disasm::Code::of_pe(&image) {
        let _ = vun_disasm::listing(&code, None);
    }
});
