use std::path::Path;

use vun_intern::Symbol;

use super::Output;

fn file(output: Output, format: &str) -> String {
    output.file(Path::new("out"), "app", Symbol::intern(format)).to_string_lossy().into_owned()
}

#[test]
fn a_program_and_a_library_are_named_as_their_object_format_names_them() {
    assert_eq!(file(Output::Executable, "elf"), "out/app");
    assert_eq!(file(Output::Shared, "elf"), "out/libapp.so");
    assert_eq!(file(Output::Static, "elf"), "out/libapp.a");
    assert_eq!(file(Output::Shared, "macho"), "out/libapp.dylib");
}

#[test]
fn a_windows_program_is_an_exe_and_its_libraries_a_dll_and_a_lib() {
    assert_eq!(file(Output::Executable, "coff"), "out/app.exe");
    assert_eq!(file(Output::Shared, "coff"), "out/app.dll");
    assert_eq!(file(Output::Static, "coff"), "out/app.lib");
}
