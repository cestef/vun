//! The command line of a GCC-style compiler driver, which C compilers and the drivers of GNU ld
//! share.

/// Compiles to an object file, without linking.
pub const COMPILE: &str = "-c";
/// Names the file written.
pub const OUTPUT: &str = "-o";
/// Names the entry symbol of the link.
pub const ENTRY: &str = "-e";
/// Links with the linker script that follows.
pub const SCRIPT: &str = "-T";
/// Links with mold rather than the driver's default linker.
pub const USE_MOLD: &str = "-fuse-ld=mold";
/// Looks for the programs it runs, the linker `ld` among them, in the directory that follows
/// first.
pub const PROGRAMS: &str = "-B";
/// Writes each function's frame size beside the object (`.su`).
pub const STACK_USAGE: &str = "-fstack-usage";
/// Links a shared library rather than an executable.
pub const SHARED: &str = "-shared";
/// Compiles code that runs wherever it is loaded, as a shared library's must.
pub const PIC: &str = "-fPIC";
