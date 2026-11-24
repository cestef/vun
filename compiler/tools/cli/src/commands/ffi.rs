//! `vun util ffi`: C bindings, C headers and register packages (spec 12, section 7).

use std::io;
use std::path::{Path, PathBuf};

use vun_binder::{
    BindError, Bytes, Failure, Glob, Library, Reason, Request, Skipped, Unavailable, Unsupported,
};
use vun_command::{Args, Arity, Command, CommandVerb, Example, Outcome, Param, PathKind, Values};
use vun_diag::{ChildKind, Diagnostic, Msg, Severity, errors};
use vun_fs::Edit;
use vun_term::{Document, Shell};

use super::PROGRAM;
use vun_driver::Session;

const HEADER: Param =
    Param::positional("header").about("the C header to bind").values(Values::Path(PathKind::File));
const OUT: Param = Param::option("out")
    .short('o')
    .about("the Vun file to write; standard output without it")
    .values(Values::Path(PathKind::File));
const CFLAGS: Param = Param::option("cflag")
    .arity(Arity::Many)
    .about("a flag the build compiles the header with, such as -Iinclude; repeat it for each");
const SCOPE: Param = Param::option("scope")
    .arity(Arity::Many)
    .about("a directory whose declarations are bound with the header's own, such as the one it includes from")
    .values(Values::Path(PathKind::Dir));
const ONLY: Param = Param::option("only")
    .arity(Arity::Many)
    .about("bind only the declarations with this name (`*` for any run of characters), and what they need");
const CLOSED: Param = Param::option("closed").arity(Arity::Many).about(
    "bind the enums named so (`*` for any run of characters) as Vun enums, for those C never holds another value in",
);
const LIBRARY: Param = Param::option("library")
    .arity(Arity::Many)
    .about("a library the functions and variables are linked from, such as gtk-4, or gobject-2.0=<dir> for those of the headers under <dir>");

const SPLIT: Param = Param::flag("split").about(
    "write the bindings of each directory of headers to a file of its own, beside the one written to `--out`, which spreads them",
);

pub(super) const GROUP: &str = "ffi";

/// The binder of C headers.
const C: &str = "c";

pub(super) const BIND: Command<Session> =
    Command::run(CommandVerb::Bind, "generate declarations from a C header", bind)
        .params(&[HEADER, OUT, CFLAGS, SCOPE, ONLY, CLOSED, LIBRARY, SPLIT])
        .examples(&[
            Example {
                line: "board.h --cflag -Iinclude --cflag -DREV=2 -o board.vun",
                about: "bind a header",
            },
            Example {
                line: "all.h --scope /usr/include/lib --only 'lib_window_*' --library lib -o lib.vun",
                about: "bind what a program calls from a library whose headers are in several directories",
            },
        ]);

/// `vun util ffi bind <header>`: the header's declarations as Vun, through libclang loaded now.
fn bind(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let header =
        PathBuf::from(args.value(&HEADER).expect("positional parameters are present when a command runs"));
    let target = args.value(&OUT).map(PathBuf::from);
    let flags: Vec<String> = args.values(&CFLAGS).map(str::to_owned).collect();
    // The bindings avoid the names the file they are written to sees already.
    let place = target.as_deref().unwrap_or(&header);
    let dir = place.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let reserved = session.names_seen_in(dir);
    let module = if args.flag(&SPLIT) {
        let module = target.as_deref().and_then(|out| session.module_of(out));
        if module.is_none() {
            let msg = Msg::new()
                .code(SPLIT.long())
                .text(" needs ")
                .code(OUT.long())
                .text(" to name a file inside a package");
            session.diag.error(Diagnostic::error(errors::TOOL_SPLIT_PLACE, msg));
            return Ok(Outcome::Failure);
        }
        module
    } else {
        None
    };
    let Some(binder) = session.registry.binders.get(C) else {
        let msg = Msg::new().text("this program runs without ").code("util ffi bind");
        session.diag.error(Diagnostic::error(errors::CLI_UNAVAILABLE, msg));
        return Ok(Outcome::Failure);
    };
    let request = Request {
        module,
        scope: args.values(&SCOPE).map(PathBuf::from).collect(),
        only: args.values(&ONLY).map(Glob::new).collect(),
        closed: args.values(&CLOSED).map(Glob::new).collect(),
        libraries: args.values(&LIBRARY).map(Library::named).collect(),
        ..Request::new(header.clone(), flags, reserved)
    };
    let bindings = match binder.item.bind(&request) {
        Ok(bindings) => bindings,
        Err(BindError::Failed(failure)) => {
            session.diag.error(failed(&header, failure));
            return Ok(Outcome::Failure);
        }
        Err(BindError::Component(fault)) => {
            let msg = Msg::new().text("the binder ").code(C).text(" failed");
            session.diag.error(Diagnostic::error(errors::COMPONENT_FAILED, msg).caused_by(&fault));
            return Ok(Outcome::Failure);
        }
    };
    for skipped in &bindings.skipped {
        session.diag.report(skip(skipped));
    }
    let Some(path) = target else {
        let name = header.display().to_string();
        shell.result(&Document { kind: "bindings", name, text: bindings.source })?;
        return Ok(Outcome::Success);
    };
    // The parts are beside the file that spreads them, in the directory of its name.
    let beside = path.with_extension("");
    let parts = bindings
        .parts
        .iter()
        .map(|part| (beside.join(&part.name).with_extension(vun_span::EXTENSION), part.source.as_bytes()));
    for (place, text) in std::iter::once((path.clone(), bindings.source.as_bytes())).chain(parts) {
        if let Err(e) = place.put(text) {
            session.diag.error(Diagnostic::unwritable(&place, &e));
            return Ok(Outcome::Failure);
        }
    }
    Ok(Outcome::Success)
}

fn failed(header: &Path, failure: Failure) -> Diagnostic {
    let name = header.display().to_string();
    match failure {
        Failure::Missing(missing) => {
            let msg = Msg::new()
                .code(PROGRAM.invocation(&[super::util::GROUP, GROUP, BIND.name]).expect("bind is a command"))
                .text(" needs libclang ")
                .num(i128::from(missing.oldest))
                .text(" or later, and none was found");
            let help = Msg::new()
                .text("install libclang, or set ")
                .code("LIBCLANG_PATH")
                .text(" to the directory holding it");
            Diagnostic::error(errors::TOOL_MISSING_LIBRARY, msg)
                .child(
                    ChildKind::Note,
                    match missing.why {
                        Unavailable::Unloadable(why) => Msg::new().text(why),
                        Unavailable::Old => Msg::new().text("the libclang found is older"),
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
            let help = Msg::new().text("pass the build's compile flags with ").code(CFLAGS.long());
            reports
                .into_iter()
                .fold(d, |d, r| d.child(ChildKind::Note, Msg::new().text(r)))
                .child(ChildKind::Help, help)
        }
    }
}

fn skip(skipped: &Skipped) -> Diagnostic {
    let why = match &skipped.reason {
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
