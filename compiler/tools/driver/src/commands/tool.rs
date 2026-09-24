//! `vun tool`: C bindings, C headers and register packages (spec 12, section 7).

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use vun_bind::{Bytes, Failure, Reason, Skipped, Unsupported};
use vun_clang::Missing;
use vun_command::{Action, Args, Command, Outcome, Param, ParamKind};
use vun_diag::{ChildKind, Diagnostic, Msg, Severity, errors};
use vun_fs::Edit;

use crate::Session;

const HEADER: Param =
    Param { name: "header", short: None, about: "the C header to bind", kind: ParamKind::Positional };
const OUT: Param = Param {
    name: "out",
    short: Some('o'),
    about: "the Vun file to write; standard output without it",
    kind: ParamKind::Option,
};
const CFLAGS: Param = Param {
    name: "cflags",
    short: None,
    about: "the flags the build compiles the header with, such as \"-Iinclude -DREV=2 --target=...\"",
    kind: ParamKind::Option,
};

pub(super) const BIND: Command<Session> = Command {
    name: "bind",
    about: "generate declarations from a C header",
    params: &[HEADER, OUT, CFLAGS],
    action: Action::Run(bind),
};

/// `vun tool bind <header>`: the header's declarations as Vun, through libclang loaded now.
fn bind(session: &mut Session, args: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let header =
        PathBuf::from(args.value(&HEADER).expect("positional parameters are present when a command runs"));
    let target = args.value(&OUT).map(PathBuf::from);
    let flags: Vec<String> =
        args.value(&CFLAGS).map(|f| f.split_whitespace().map(str::to_owned).collect()).unwrap_or_default();
    // The bindings avoid the names the file they are written to sees already.
    let place = target.as_deref().unwrap_or(&header);
    let dir = place.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let reserved = session.names_seen_in(dir);
    let bindings = match vun_bind::bind(&header, &flags, &reserved) {
        Ok(bindings) => bindings,
        Err(failure) => {
            session.diag.error(failed(&header, failure));
            return Ok(Outcome::Failure);
        }
    };
    for skipped in &bindings.skipped {
        session.diag.report(skip(skipped));
    }
    match target {
        Some(path) => {
            if let Err(e) = path.put(bindings.source.as_bytes()) {
                let msg = Msg::new().text("cannot write ").code(path.display().to_string());
                session.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(&e));
                return Ok(Outcome::Failure);
            }
        }
        None => out.write_all(bindings.source.as_bytes())?,
    }
    Ok(Outcome::Success)
}

fn failed(header: &Path, failure: Failure) -> Diagnostic {
    let name = header.display().to_string();
    match failure {
        Failure::Missing(missing) => {
            let msg = Msg::new()
                .code("vun tool bind")
                .text(" needs libclang ")
                .num(i128::from(vun_clang::OLDEST))
                .text(" or later, and none was found");
            let help = Msg::new()
                .text("install libclang, or set ")
                .code("LIBCLANG_PATH")
                .text(" to the directory holding it");
            Diagnostic::error(errors::TOOL_MISSING_LIBRARY, msg)
                .child(
                    ChildKind::Note,
                    match missing {
                        Missing::Unloadable(why) => Msg::new().text(why),
                        Missing::Old => Msg::new().text("the libclang found is older"),
                    },
                )
                .child(ChildKind::Help, help)
        }
        Failure::Unreadable => {
            Diagnostic::error(errors::TOOL_BIND_FAILED, Msg::new().text("clang could not open ").code(name))
        }
        Failure::Errors(reports) => {
            let d = Diagnostic::error(
                errors::TOOL_BIND_FAILED,
                Msg::new().text("clang could not read ").code(name),
            );
            let help = Msg::new().text("pass the build's compile flags with ").code("--cflags");
            reports
                .into_iter()
                .fold(d, |d, r| d.child(ChildKind::Note, Msg::new().text(r.text)))
                .child(ChildKind::Help, help)
        }
    }
}

fn skip(skipped: &Skipped) -> Diagnostic {
    let why = match &skipped.reason {
        Reason::Variadic => {
            Msg::new().text(" is left out: it takes arguments past its parameters (").code("...").text(")")
        }
        Reason::Bytes(why) => {
            let only = Msg::new().text(" is bound as its size and alignment only: ");
            match why {
                Bytes::Union => only.text("it is a union"),
                Bytes::BitField(field) => {
                    only.text("its bit field ").code(field.clone()).text(" is laid out by C")
                }
                Bytes::Field(field) => {
                    only.text("Vun has no counterpart for the type of its field ").code(field.clone())
                }
            }
        }
        Reason::Unsupported(Unsupported::Type(c)) => {
            Msg::new().text(" is left out: Vun has no counterpart for ").code(c.clone())
        }
        Reason::Unsupported(Unsupported::Opaque(name)) => Msg::new()
            .text(" is left out: it passes ")
            .code(name.clone())
            .text(" by value, whose fields are not bound"),
    };
    let msg = Msg::new().code(skipped.name.clone()).append(why);
    let at = Msg::new()
        .text("declared at ")
        .code(skipped.file.display().to_string())
        .text(", line ")
        .num(i128::from(skipped.line));
    Diagnostic::new(errors::TOOL_BIND_SKIPPED, Severity::Warning, msg).child(ChildKind::Note, at)
}
