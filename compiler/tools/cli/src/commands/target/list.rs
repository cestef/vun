//! `vun target list`: every target, what builds for it, where its programs run, and what the
//! machine lacks for it.

use std::io;

use vun_command::{Args, Outcome};
use vun_data::Object;
use vun_term::{Column, Record, Row, Shell, Style, Text};

use vun_driver::Session;
use vun_driver::setup::{Found, Offered, Runs};

use super::root;

pub(super) fn list(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(file) = root(session, args, shell) else { return Ok(session.outcome()) };
    let offered = session.setups(file);
    shell.table(&offered.iter().map(Offer).collect::<Vec<_>>())?;
    Ok(session.outcome())
}

/// A target as `vun target list` shows it.
struct Offer<'a>(&'a Offered);

impl Offer<'_> {
    fn status(&self) -> Text {
        let setup = &self.0.setup;
        if setup.backend.is_err() {
            return Text::of(Style::Error, "no backend");
        }
        let lacking: Vec<Text> = setup
            .lacking()
            .map(|p| {
                let command = Text::of(Style::Literal, p.command.as_str());
                match p.found {
                    Found::Host(_) => Text::of(Style::Dim, "a cross ").append(command),
                    Found::At(_) | Found::Missing => command,
                }
            })
            .collect();
        if lacking.is_empty() {
            Text::of(Style::Good, "ready")
        } else {
            Text::of(Style::Warning, "lacks ").append(Text::join(lacking, ", "))
        }
    }

    fn runs(&self) -> Text {
        match &self.0.setup.runs {
            Runs::Here => Text::of(Style::Good, "here"),
            Runs::Through { command, .. } => Text::of(Style::Literal, command[0].as_str()),
            Runs::Elsewhere => Text::of(Style::Dim, "elsewhere"),
        }
    }
}

impl Record for Offer<'_> {
    fn kind(&self) -> &'static str {
        "target"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        let Offered { listed, setup, host } = self.0;
        let lacks: Vec<String> = setup.lacking().map(|p| p.command.as_str().to_owned()).collect();
        Object::new()
            .with("target", listed.path.to_string())
            .with("host", *host)
            .with("backend", setup.backend.as_ref().ok().map(|(name, _)| name.as_str().to_owned()))
            .with("linker", setup.linkage.linker.as_str().to_owned())
            .with("runs", self.runs().to_string())
            .with("ready", setup.ready())
            .with("lacks", lacks)
    }
}

impl Row for Offer<'_> {
    const COLUMNS: &'static [Column] = &[
        Column::left("target"),
        Column::left("backend"),
        Column::left("linker"),
        Column::left("runs"),
        Column::left("status"),
    ];

    fn cells(&self) -> Vec<Text> {
        let Offered { listed, setup, host } = self.0;
        let mut target = Text::of(Style::Literal, listed.path.to_string());
        if *host {
            target = target.append(Text::of(Style::Dim, " (host)"));
        }
        let backend = setup.backend.as_ref().map_or_else(
            |_| Text::of(Style::Dim, "none"),
            |(name, _)| Text::of(Style::Literal, name.as_str()),
        );
        let linker = Text::of(Style::Literal, setup.linkage.linker.as_str());
        vec![target, backend, linker, self.runs(), self.status()]
    }
}
