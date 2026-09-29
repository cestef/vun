//! Generated files of formats other than Rust, and the header each format writes.

/// A generated file of a format other than Rust.
pub(crate) struct Data {
    /// The file's path in the output directory, with its extension.
    pub(crate) name: String,
    pub(crate) format: Format,
    pub(crate) text: String,
}

/// A format of generated data, which says how its header is written.
#[derive(Clone, Copy)]
pub(crate) enum Format {
    /// YAML 1.2, which JSON documents are: the directive, then a comment.
    Yaml,
    JavaScript,
    /// Tree-sitter queries.
    Query,
    C,
    Rust,
    Toml,
    Lua,
    /// JSON, which has no comments, so no header.
    Json,
}

impl Format {
    /// The file's start: `header` as a comment of the format, after what must come first.
    pub(super) fn header(self, header: &str) -> String {
        let (preamble, comment) = match self {
            Self::Yaml => ("%YAML 1.2\n---\n", "#"),
            Self::JavaScript | Self::C | Self::Rust => ("", "//"),
            Self::Toml => ("", "#"),
            Self::Lua => ("", "--"),
            Self::Query => ("", ";"),
            Self::Json => return String::new(),
        };
        [preamble, comment, header, "\n"].concat()
    }
}
