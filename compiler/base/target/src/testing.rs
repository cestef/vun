//! Targets for tests of what takes one.

use vun_intern::Symbol;

use crate::{
    Atomics, Attributes, Backends, BuildsFor, CCompiler, CInt, CTypes, Convention, Endian, Floats,
    FrameSizes, TailCalls, TargetSpec, Threads,
};

/// A 64-bit target whose C compiler has no atomics.
#[must_use]
pub fn target() -> TargetSpec {
    let int = |bits, align| CInt { bits, align };
    let name = Symbol::intern;
    TargetSpec {
        arch: name("x86_64"),
        isa: name("x86_64"),
        features: Vec::new(),
        cpu: name("generic"),
        os: name("linux"),
        abi: name("gnu"),
        convention: Convention::Sysv,
        accepts: Vec::new(),
        pointer_bits: 64,
        register_bits: 64,
        floats: Floats::ALL,
        threads: Threads::One,
        endian: Endian::Little,
        object_format: name("elf"),
        entry_symbol: None,
        os_version: None,
        backends: Backends { debug: vec![name("c")], release: vec![name("c")] },
        linker: name("gnu"),
        c: CTypes {
            short: int(16, 2),
            int: int(32, 4),
            long: int(64, 8),
            long_long: int(64, 8),
            pointer_align: 8,
            compiler: CCompiler {
                builds_for: BuildsFor::Target,
                command: name("cc"),
                archiver: name("ar"),
                flags: Vec::new(),
                optimize: Vec::new(),
                shared: Vec::new(),
                link_flags: Vec::new(),
                libraries: Vec::new(),
                atomics: Atomics::None,
                attributes: Attributes::None,
                tail_calls: TailCalls::None,
                frame_sizes: FrameSizes::StackUsage,
                version_flag: None,
                sysroot: None,
            },
        },
    }
}
