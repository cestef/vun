//! The wasm linker on objects clang writes, where the machine has clang: a COMDAT group two
//! objects hold is kept once, and zeros take no bytes of the program.

use std::process::Command;

use vun_fs::Scratch;
use vun_tool::Executable;

use vun_link_wasm::WasmLinker;

use super::objects::read;

/// Each source compiled by clang for WASI, as `language`; none without clang.
fn compiled(name: &str, language: &str, sources: &[&str]) -> Option<Vec<Vec<u8>>> {
    let clang = Executable::search("clang")?;
    let scratch = Scratch::new(name);
    let objects = sources.iter().enumerate().map(|(i, source)| {
        let path = scratch.write(format!("{i}.src"), source);
        let object = scratch.join(format!("{i}.o"));
        let status = Command::new(&clang)
            .args(["--target=wasm32-wasip1", "-O1", "-x", language, "-c"])
            .arg(&path)
            .arg("-o")
            .arg(&object)
            .status();
        assert!(status.unwrap().success(), "clang compiles {}", path.display());
        std::fs::read(object).unwrap()
    });
    Some(objects.collect())
}

/// Two C++ objects using one inline function, each exporting a function calling it.
const INLINE: [&str; 2] = [
    "inline int twice(int x) { return 2 * x; }\n__attribute__((export_name(\"a\"))) int a(int x) { return twice(x); }",
    "inline int twice(int x) { return 2 * x; }\n__attribute__((export_name(\"b\"))) int b(int x) { return twice(x) + 1; }",
];

#[test]
fn a_comdat_group_two_objects_hold_is_kept_once() {
    let Some(objects) = compiled("wasm-link-comdat", "c++", &INLINE) else { return };
    let read = read(&WasmLinker::program(&objects, None).unwrap());
    assert_eq!(read.exports, ["memory", "a", "b"]);
    // `a`, `b`, and `twice` once, unless clang inlined it into both.
    assert!(read.functions <= 3, "{} functions", read.functions);
}

/// Bytes the program writes, and a table of zeros.
const ZEROS: &str = "static int counts[1024];\nstatic const char text[] = \"vun\";\n\
__attribute__((export_name(\"count\"))) int count(int i) { counts[i & 1023] += text[i & 3]; return counts[i & 1023]; }";

#[test]
fn zeros_take_no_bytes_of_the_program() {
    let Some(objects) = compiled("wasm-link-zeros", "c", &[ZEROS]) else { return };
    let read = read(&WasmLinker::program(&objects, None).unwrap());
    let written: usize = read.data.iter().map(|(_, bytes)| bytes.len()).sum();
    assert!(written < 64, "{written} bytes written of the 4 KiB of zeros");
}
