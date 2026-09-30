//! `vun target show`: what a build for the session's target uses, each part with where it is
//! found on the machine and why it is the one.

use std::io;

use vun_command::{Args, Outcome};
use vun_data::Object;
use vun_intern::Symbol;
use vun_span::SourceMap;
use vun_term::{Column, Record, Row, Shell, Style, Text};

use vun_driver::setup::{Chose, Found, From, Runs, Setup, Shown};
use vun_driver::{Link, Pick, Session, Unchosen};
use vun_target::{Profile, Run};

use super::{Shows, root};

pub(super) fn show(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(file) = root(session, args, shell) else { return Ok(session.outcome()) };
    let Ok(shown) = session.shown(file) else { return Ok(session.outcome()) };
    shell.table(&Part::of(&shown, session))?;
    Ok(if shown.setup.ready() { session.outcome() } else { Outcome::Failure })
}

/// What a part of a build is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Aspect {
    Target,
    Backend,
    Linker,
    Program(Run),
    Sysroot,
    Runs,
}

impl Aspect {
    fn name(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Backend => "backend",
            Self::Linker => "linker",
            Self::Program(Run::Compiler) => "compiler",
            Self::Program(Run::Archiver) => "archiver",
            Self::Program(Run::Program(_)) => "program",
            Self::Sysroot => "sysroot",
            Self::Runs => "runs",
        }
    }
}

/// Why a part of a build is the one it is.
enum Reason {
    /// Named by this option.
    Option(&'static str),
    Host,
    Plan,
    /// The first of the target's preferences for the profile that builds, after these.
    Preferred(Profile, Vec<Symbol>),
    /// None of the target's preferences for the profile builds.
    Unpreferred(Profile),
    Target,
    /// The linker making the output, the target's own, this one, not making it.
    Instead(Symbol),
    /// The machine's tools file, at this place.
    Tools(String),
    /// Run by this component.
    Component(Symbol),
    /// The target is the running machine's.
    Machine,
    /// No runner is named for a target of another machine.
    Unrun,
}

impl Reason {
    fn from(from: From, session: &Session) -> Self {
        match from {
            From::Target => Self::Target,
            From::Tools(span) => Self::Tools(session.sources.locate(span).to_string()),
            From::Component(name) => Self::Component(name),
        }
    }

    fn text(&self) -> Text {
        let dim = |text: &'static str| Text::of(Style::Dim, text);
        let literal = |name: Symbol| Text::of(Style::Literal, name.as_str());
        match self {
            Self::Option(option) => Text::of(Style::Literal, *option),
            Self::Host => dim("the host's"),
            Self::Plan => dim("the plan's"),
            Self::Preferred(profile, passed) => {
                passed.iter().fold(dim(profile.name()).append(dim(" preference")), |why, &name| {
                    why.append(dim(", not ")).append(literal(name))
                })
            }
            Self::Unpreferred(profile) => {
                dim("no ").append(dim(profile.name())).append(dim(" preference builds"))
            }
            Self::Target => dim("the target's"),
            Self::Instead(own) => dim("for this output, not the target's ").append(literal(*own)),
            Self::Tools(place) => Text::of(Style::Scope, place.clone()),
            Self::Component(name) => dim("run by ").append(literal(*name)),
            Self::Machine => dim("the target is this machine's"),
            Self::Unrun => dim("no runner in the tools file"),
        }
    }
}

/// A part of a build as `vun target show` shows it: what it is, where a program is found on the
/// machine, and why it is the one.
struct Part {
    aspect: Aspect,
    value: String,
    /// For a program: whether the machine has it.
    found: Option<Found>,
    why: Reason,
}

impl Part {
    fn of(shown: &Shown, session: &Session) -> Vec<Self> {
        let Shown { path, chose, setup } = shown;
        let target = path.as_ref().map_or_else(|| setup.spec.arch.as_str().to_owned(), ToString::to_string);
        let chose = match chose {
            Chose::Named => Reason::Option("--target"),
            Chose::Host => Reason::Host,
            Chose::Plan => Reason::Plan,
        };
        let mut parts =
            vec![Self::named(Aspect::Target, target, chose), Self::backend(setup), Self::linker(setup)];
        parts.extend(setup.programs.iter().map(|program| Self {
            aspect: Aspect::Program(program.run),
            value: program.command.as_str().to_owned(),
            found: Some(program.found.clone()),
            why: Reason::from(program.from, session),
        }));
        if let Some((root, from)) = setup.sysroot {
            parts.push(Self::named(Aspect::Sysroot, root.as_str().to_owned(), Reason::from(from, session)));
        }
        parts.push(match &setup.runs {
            Runs::Here => Self::named(Aspect::Runs, "here".to_owned(), Reason::Machine),
            Runs::Through { command, found, at } => {
                let command: Vec<&str> = command.iter().map(|s| s.as_str()).collect();
                let why = Reason::from(From::Tools(*at), session);
                Self { aspect: Aspect::Runs, value: command.join(" "), found: Some(found.clone()), why }
            }
            Runs::Elsewhere => Self::named(Aspect::Runs, "elsewhere".to_owned(), Reason::Unrun),
        });
        parts
    }

    fn named(aspect: Aspect, value: String, why: Reason) -> Self {
        Self { aspect, value, found: None, why }
    }

    fn backend(setup: &Setup) -> Self {
        let (name, why) = match &setup.backend {
            Ok((name, Pick::Named)) => (Some(*name), Reason::Option("--backend")),
            Ok((name, Pick::Preferred(profile, passed))) => {
                (Some(*name), Reason::Preferred(*profile, passed.iter().map(|&(n, _)| n).collect()))
            }
            Err(Unchosen::Named(_, _)) => (None, Reason::Option("--backend")),
            Err(Unchosen::Preferred(profile, _)) => (None, Reason::Unpreferred(*profile)),
        };
        Self::named(Aspect::Backend, name.map_or_else(String::new, |n| n.as_str().to_owned()), why)
    }

    fn linker(setup: &Setup) -> Self {
        let why = match setup.linkage.by {
            Link::Named => Reason::Option("--linker"),
            Link::Target => Reason::Target,
            Link::Instead(own) => Reason::Instead(own),
        };
        Self::named(Aspect::Linker, setup.linkage.linker.as_str().to_owned(), why)
    }
}

impl Record for Part {
    fn kind(&self) -> &'static str {
        "part"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("part", self.aspect.name())
            .with("value", self.value.clone())
            .with("found", self.found.as_ref().and_then(Shows::at))
            .with("usable", self.found.as_ref().is_none_or(|f| matches!(f, Found::At(_))))
            .with("why", self.why.text().to_string())
    }
}

impl Row for Part {
    const COLUMNS: &'static [Column] =
        &[Column::left("part"), Column::left("value"), Column::left("found"), Column::left("why")];

    fn cells(&self) -> Vec<Text> {
        let found = self.found.as_ref().map_or_else(Text::new, Shows::text);
        let value = if self.value.is_empty() {
            Text::of(Style::Error, "none")
        } else {
            Text::of(Style::Literal, self.value.clone())
        };
        vec![Text::of(Style::Key, self.aspect.name()), value, found, self.why.text()]
    }
}
