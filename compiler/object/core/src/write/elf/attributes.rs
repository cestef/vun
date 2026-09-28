//! The build attributes of a 32-bit Arm object (`.ARM.attributes`, the Arm ABI's addenda,
//! section 3): the architecture its code needs, Armv7, whose Thumb-2 it is written in, and where
//! its functions pass floats. Linkers read them to choose how calls reach a function, and refuse
//! to join objects of other float ABIs.

use vun_bytes::Out;

use crate::{Float, Machine};

/// The version of the format, and the vendor of the attributes read here.
const FORMAT: u8 = b'A';
const VENDOR: &[u8] = b"aeabi";
/// The tag of the attributes of the whole file.
const FILE: u64 = 1;
/// The attributes' tags, in the order they are written, and their values.
const CPU_ARCH: u64 = 6;
const V7: u64 = 10;
const THUMB_ISA_USE: u64 = 9;
const THUMB_2: u64 = 2;
const ABI_VFP_ARGS: u64 = 28;
const VFP_REGISTERS: u64 = 1;

impl Machine {
    /// The bytes of `.ARM.attributes`, for 32-bit Arm.
    pub(super) fn elf_attributes(self) -> Option<Vec<u8>> {
        let Self::Arm(float) = self else { return None };
        let mut tags = Out::default();
        for (tag, value) in [(CPU_ARCH, V7), (THUMB_ISA_USE, THUMB_2)] {
            tags.uleb(tag);
            tags.uleb(value);
        }
        if float == Float::Hard {
            tags.uleb(ABI_VFP_ARGS);
            tags.uleb(VFP_REGISTERS);
        }
        // Each length counts itself: the file's with its tag, the vendor's with its name.
        let length = |bytes: usize| u32::try_from(bytes).expect("a few attributes");
        let mut out = Out::default();
        out.u8(FORMAT);
        out.u32(length(4 + VENDOR.len() + 1 + 1 + 4 + tags.0.len()));
        out.str(VENDOR);
        out.uleb(FILE);
        out.u32(length(1 + 4 + tags.0.len()));
        out.bytes(&tags.0);
        Some(out.0)
    }
}
