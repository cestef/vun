//! `vun debug disasm <file> [symbol]`: the code of an ELF file read back as instructions, each
//! with its address and bytes, labelled by the file's symbols; with a symbol, the code it labels.

use std::io;
use std::path::Path;

use vun_command::{Args, Arity, Example, Outcome, Param, PathKind, Values};
use vun_diag::{Diagnostic, Msg, errors};
use vun_driver::Session;
use vun_term::Shell;

use super::dump::DUMP;

pub(super) const OBJECT: Param = Param::positional("file")
    .about("an ELF file: an object, a program or a library")
    .values(Values::Path(PathKind::File));
pub(super) const SYMBOL: Param =
    Param::positional("symbol").arity(Arity::Optional).about("list only the code this symbol labels");

pub(super) const EXAMPLES: &[Example] = &[
    Example { line: "app", about: "list the code of a program" },
    Example { line: "app main", about: "list the code a symbol labels" },
];

pub(super) fn disasm(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let path = Path::new(args.value(&OBJECT).expect("positional parameters are present when a command runs"));
    let only = args.value(&SYMBOL).map(str::as_bytes);
    let mut refuse = |why: &dyn std::fmt::Display| {
        let msg = Msg::new()
            .text("cannot list the code of ")
            .code(path.display().to_string())
            .text(": ")
            .text(why.to_string());
        session.diag.error(Diagnostic::error(errors::TOOL_DISASM_FAILED, msg));
        Ok(Outcome::Failure)
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => return refuse(&e),
    };
    let file = match vun_elf::File::read(&bytes) {
        Ok(file) => file,
        Err(e) => return refuse(&format!("not an ELF file ({e:?})")),
    };
    let lines = match vun_disasm::listing(&file, only) {
        Ok(lines) => lines,
        Err(e) => return refuse(&e),
    };
    shell.document(DUMP, "disasm".to_owned(), |out| {
        for line in &lines {
            writeln!(out, "{line}")?;
        }
        Ok(Outcome::Success)
    })
}
