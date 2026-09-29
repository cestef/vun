//! The editors' query dialects: which query files each reads, what it names each capture, and
//! whether its first matching pattern wins.

pub(crate) use super::queries::Dialect;
use super::queries::{Capture, File};

/// The names nvim-treesitter settled on, which the tree-sitter CLI and GitHub read too.
pub(crate) const STANDARD: Dialect =
    Dialect { files: &[File::Highlights], capture: standard, first_wins: false };

pub(crate) const NEOVIM: Dialect = Dialect {
    files: &[File::Highlights, File::Folds, File::Indents, File::TextObjects],
    capture: standard,
    first_wins: false,
};

pub(crate) const HELIX: Dialect = Dialect {
    files: &[File::Highlights, File::Indents, File::TextObjects],
    capture: helix,
    first_wins: true,
};

pub(crate) const ZED: Dialect = Dialect {
    files: &[File::Highlights, File::Brackets, File::Indents, File::Outline, File::TextObjects],
    capture: zed,
    first_wins: false,
};

fn standard(c: Capture) -> Option<&'static str> {
    Some(match c {
        Capture::Keyword => "keyword",
        Capture::Operator => "operator",
        Capture::Bracket => "punctuation.bracket",
        Capture::Delimiter => "punctuation.delimiter",
        Capture::Number => "number",
        Capture::String => "string",
        Capture::Character => "character",
        Capture::Comment => "comment",
        Capture::Documentation => "comment.documentation",
        Capture::Label => "label",
        Capture::Function => "function",
        Capture::Call => "function.call",
        Capture::Method => "function.method.call",
        Capture::Property => "property",
        Capture::Variant => "constant",
        Capture::Parameter => "variable.parameter",
        Capture::Type => "type",
        Capture::Variable => "variable",
        Capture::Builtin => "variable.builtin",
        Capture::Fold => "fold",
        Capture::Indent => "indent.begin",
        Capture::Outdent => "indent.branch",
        Capture::FunctionAround => "function.outer",
        Capture::FunctionInside => "function.inner",
        Capture::TypeAround => "class.outer",
        Capture::ParameterInside => "parameter.inner",
        Capture::CommentAround => "comment.outer",
        Capture::Item | Capture::Name | Capture::Open | Capture::Close => return None,
    })
}

fn helix(c: Capture) -> Option<&'static str> {
    Some(match c {
        Capture::Number => "constant.numeric",
        Capture::Character => "constant.character",
        Capture::Documentation => "comment.line.documentation",
        Capture::Call => "function",
        Capture::Method => "function.method",
        Capture::Property => "variable.other.member",
        Capture::Variant => "type.enum.variant",
        Capture::Indent => "indent",
        Capture::Outdent => "outdent",
        Capture::FunctionAround => "function.around",
        Capture::FunctionInside => "function.inside",
        Capture::TypeAround => "class.around",
        Capture::ParameterInside => "parameter.inside",
        Capture::CommentAround => "comment.around",
        Capture::Fold => return None,
        other => return standard(other),
    })
}

fn zed(c: Capture) -> Option<&'static str> {
    Some(match c {
        Capture::Character => "string",
        Capture::Documentation => "comment.doc",
        Capture::Call => "function",
        Capture::Method => "function.method",
        Capture::Variant => "variant",
        Capture::Builtin => "variable.special",
        Capture::Indent => "indent",
        Capture::Outdent => "end",
        Capture::Item => "item",
        Capture::Name => "name",
        Capture::Open => "open",
        Capture::Close => "close",
        Capture::FunctionAround => "function.around",
        Capture::FunctionInside => "function.inside",
        Capture::TypeAround => "class.around",
        Capture::ParameterInside | Capture::Fold => return None,
        Capture::CommentAround => "comment.around",
        other => return standard(other),
    })
}
