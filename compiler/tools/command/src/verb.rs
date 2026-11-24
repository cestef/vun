use vun_variants::Variants;

/// What running a command leaves behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
pub enum Kind {
    /// Reports, and changes no file, package or setting the user keeps; caches do not count.
    Read,
    Write,
}

/// Whether a table entry is a verb or a noun kept so that no command was renamed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
pub enum Standing {
    Verb,
    /// A noun the tree already used as a leaf name; a later rename may remove it.
    Legacy,
}

/// The closed set of names a leaf command of `vun` may carry, each with one meaning everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Variants)]
pub enum CommandVerb {
    List,
    Show,
    Find,
    Watch,
    Check,
    Explain,
    Dump,
    Compare,
    Reduce,
    Run,
    Test,
    Bench,
    Fuzz,
    Cover,
    Coverage,
    Disasm,
    Queries,
    Profile,
    Smith,
    Completion,
    Lsp,
    Add,
    Remove,
    Set,
    Default,
    Update,
    Install,
    New,
    Build,
    Flash,
    Fix,
    Fmt,
    Doc,
    Pack,
    Prime,
    Bind,
    Gc,
    Clean,
}

impl CommandVerb {
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.entry().0
    }

    #[must_use]
    pub const fn kind(self) -> Kind {
        self.entry().1
    }

    #[must_use]
    pub const fn about(self) -> &'static str {
        self.entry().2
    }

    #[must_use]
    pub const fn standing(self) -> Standing {
        self.entry().3
    }

    pub fn legacy() -> impl Iterator<Item = Self> {
        Self::ALL.iter().copied().filter(|verb| matches!(verb.standing(), Standing::Legacy))
    }

    const fn entry(self) -> (&'static str, Kind, &'static str, Standing) {
        use Kind::{Read, Write};
        match self {
            Self::List => ("list", Read, "name every item of a collection", Standing::Verb),
            Self::Show => ("show", Read, "describe one item in full", Standing::Verb),
            Self::Find => ("find", Read, "search for items matching a query", Standing::Verb),
            Self::Watch => ("watch", Read, "report again whenever the subject changes", Standing::Verb),
            Self::Check => ("check", Read, "analyze and report problems, generating nothing", Standing::Verb),
            Self::Explain => {
                ("explain", Read, "give the reference page of a name or a diagnostic", Standing::Verb)
            }
            Self::Dump => ("dump", Read, "print an internal representation", Standing::Verb),
            Self::Compare => (
                "compare",
                Read,
                "run one input through several implementations and report whether they agree",
                Standing::Verb,
            ),
            Self::Reduce => {
                ("reduce", Read, "shrink an input while it keeps failing the same way", Standing::Verb)
            }
            Self::Run => ("run", Read, "build a program and execute it", Standing::Verb),
            Self::Test => ("test", Read, "run tests and report their results", Standing::Verb),
            Self::Bench => ("bench", Read, "run benchmarks and report measurements", Standing::Verb),
            Self::Fuzz => ("fuzz", Read, "search for inputs that break properties", Standing::Verb),
            Self::Cover => ("cover", Read, "run tests and report the lines they reach", Standing::Verb),
            Self::Coverage => ("coverage", Read, "report how far the stages reach", Standing::Legacy),
            Self::Disasm => ("disasm", Read, "read machine code back as instructions", Standing::Verb),
            Self::Queries => ("queries", Read, "report what saved queries hold", Standing::Legacy),
            Self::Profile => ("profile", Read, "measure a compile and rank its costs", Standing::Legacy),
            Self::Smith => ("smith", Read, "try programs made by chance", Standing::Legacy),
            Self::Completion => ("completion", Read, "print a shell's completion script", Standing::Legacy),
            Self::Lsp => ("lsp", Read, "serve the language server", Standing::Legacy),
            Self::Add => ("add", Write, "put an item into a collection", Standing::Verb),
            Self::Remove => ("remove", Write, "take an item out of a collection", Standing::Verb),
            Self::Set => ("set", Write, "give a setting a value", Standing::Verb),
            Self::Default => ("default", Write, "choose the item used when none is named", Standing::Verb),
            Self::Update => ("update", Write, "move items to their newest release", Standing::Verb),
            Self::Install => ("install", Write, "fetch an item into its place", Standing::Verb),
            Self::New => ("new", Write, "start a project from a template", Standing::Verb),
            Self::Build => ("build", Write, "generate the output of a program", Standing::Verb),
            Self::Flash => ("flash", Write, "build programs and write them to devices", Standing::Verb),
            Self::Fix => ("fix", Write, "apply the fixes diagnostics propose", Standing::Verb),
            Self::Fmt => ("fmt", Write, "rewrite source files in the canonical layout", Standing::Verb),
            Self::Doc => ("doc", Write, "write documentation", Standing::Verb),
            Self::Pack => ("pack", Write, "write a package's archive", Standing::Verb),
            Self::Prime => ("prime", Write, "ship precomputed results with a toolchain", Standing::Verb),
            Self::Bind => {
                ("bind", Write, "generate declarations from another language's header", Standing::Verb)
            }
            Self::Gc => ("gc", Write, "remove what nothing refers to any more", Standing::Verb),
            Self::Clean => ("clean", Write, "remove generated output", Standing::Verb),
        }
    }
}
