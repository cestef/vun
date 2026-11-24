use vun_variants::Variants;

/// What running a command leaves behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
pub enum Kind {
    /// Reports, and changes no file, package or setting the user keeps; caches do not count.
    Read,
    Write,
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

    const fn entry(self) -> (&'static str, Kind, &'static str) {
        use Kind::{Read, Write};
        match self {
            Self::List => ("list", Read, "name every item of a collection"),
            Self::Show => ("show", Read, "describe one item in full"),
            Self::Find => ("find", Read, "search for items matching a query"),
            Self::Watch => ("watch", Read, "report again whenever the subject changes"),
            Self::Check => ("check", Read, "analyze and report problems, generating nothing"),
            Self::Explain => ("explain", Read, "give the reference page of a name or a diagnostic"),
            Self::Dump => ("dump", Read, "print an internal representation"),
            Self::Compare => (
                "compare",
                Read,
                "run one input through several implementations and report whether they agree",
            ),
            Self::Reduce => ("reduce", Read, "shrink an input while it keeps failing the same way"),
            Self::Run => ("run", Read, "build a program and execute it"),
            Self::Test => ("test", Read, "run tests and report their results"),
            Self::Bench => ("bench", Read, "run benchmarks and report measurements"),
            Self::Fuzz => ("fuzz", Read, "search for inputs that break properties"),
            Self::Cover => ("cover", Read, "run tests and report the lines they reach"),
            Self::Coverage => ("coverage", Read, "report how far the stages reach"),
            Self::Disasm => ("disasm", Read, "read machine code back as instructions"),
            Self::Queries => ("queries", Read, "report what saved queries hold"),
            Self::Profile => ("profile", Read, "measure a compile and rank its costs"),
            Self::Smith => ("smith", Read, "try programs made by chance"),
            Self::Completion => ("completion", Read, "print a shell's completion script"),
            Self::Lsp => ("lsp", Read, "serve the language server"),
            Self::Add => ("add", Write, "put an item into a collection"),
            Self::Remove => ("remove", Write, "take an item out of a collection"),
            Self::Set => ("set", Write, "give a setting a value"),
            Self::Default => ("default", Write, "choose the item used when none is named"),
            Self::Update => ("update", Write, "move items to their newest release"),
            Self::Install => ("install", Write, "fetch an item into its place"),
            Self::New => ("new", Write, "start a project from a template"),
            Self::Build => ("build", Write, "generate the output of a program"),
            Self::Flash => ("flash", Write, "build programs and write them to devices"),
            Self::Fix => ("fix", Write, "apply the fixes diagnostics propose"),
            Self::Fmt => ("fmt", Write, "rewrite source files in the canonical layout"),
            Self::Doc => ("doc", Write, "write documentation"),
            Self::Pack => ("pack", Write, "write a package's archive"),
            Self::Prime => ("prime", Write, "ship precomputed results with a toolchain"),
            Self::Bind => ("bind", Write, "generate declarations from another language's header"),
            Self::Gc => ("gc", Write, "remove what nothing refers to any more"),
            Self::Clean => ("clean", Write, "remove generated output"),
        }
    }
}
