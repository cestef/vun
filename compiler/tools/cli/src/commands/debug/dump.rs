//! `vun debug dump <view>`: each view of a file as text, with origins.

use std::io;
use std::path::Path;

use vun_command::{Args, Outcome};
use vun_driver::Session;
use vun_driver::view::View;
use vun_term::Shell;

use super::super::FILE;

/// What debug commands report, and the group of the views.
pub(super) const DUMP: &str = "dump";

/// The command showing the view `$view` of a file, taking `$param`s besides the file.
macro_rules! view {
    ($view:ident $(, $param:expr)*) => {
        vun_command::Command::run(
            vun_driver::view::View::$view.name(),
            vun_driver::view::View::$view.about(),
            |s, args, shell| dump::show(vun_driver::view::View::$view, s, args, shell),
        )
        .params(&[FILE $(, $param)*])
    };
}
pub(super) use view;

/// Reports the view `view` of the file `args` names as a dump.
pub(super) fn show(
    view: View,
    session: &mut Session,
    args: &Args,
    shell: &mut Shell<'_>,
) -> io::Result<Outcome> {
    shell.document(DUMP, view.name().to_owned(), |out| {
        let path = args.value(&FILE).expect("positional parameters are present when a command runs");
        let Some(file) = session.load(Path::new(path)) else { return Ok(Outcome::Failure) };
        session.view(view, file, out)?;
        Ok(session.outcome())
    })
}
