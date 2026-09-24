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
/// Writes each function's frame size beside the object (`.su`).
pub const STACK_USAGE: &str = "-fstack-usage";
