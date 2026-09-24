//! Patterns: what bindings, parameters, tests and arms match against.

use vun_origin::OriginId;

use super::{BinderId, ExprId, List, Name, PatId};

pub struct Pat {
    pub kind: PatKind,
    pub origin: OriginId,
}

pub enum PatKind {
    Bind(BinderId),
    /// `_`
    Hole,
    Tuple(List<PatId>),
    /// `.name(args)` or `.name`.
    Variant {
        name: Name,
        args: Option<List<PatId>>,
    },
    /// `.{ .x = p }` or `T{ .x = p }`; unnamed entries are positional.
    Struct {
        ty: Option<ExprId>,
        fields: List<(Option<Name>, PatId)>,
    },
    /// `p | q`: either alternative.
    Or(Vec<PatId>),
    /// `lo..hi` or `lo..=hi`; `hi` is absent for `lo..`.
    Range {
        lo: ExprId,
        hi: Option<ExprId>,
        inclusive: bool,
    },
    /// `..` or `..rest`: the remaining items or fields.
    Rest(Option<BinderId>),
    /// `n := p`: the whole value bound to `n`, and matched against `p`.
    At {
        binder: BinderId,
        pat: PatId,
    },
    /// A value compared for equality: literals, constants.
    Value(ExprId),
    /// `$(e)`: a binder whose name `e` computes at compile time (spec 03, section 2).
    Computed(ExprId),
    Error,
}

impl PatKind {
    /// Whether the pattern matches every value of its type, as far as syntax tells: binders,
    /// holes, and tuples and anonymous structs of those.
    #[must_use]
    pub fn irrefutable(&self, pats: &vun_index::IndexVec<PatId, super::Pat>) -> bool {
        let all = |entries: &List<PatId>| {
            entries.iter().all(|e| match e {
                super::Entry::One(p) => pats[*p].kind.irrefutable(pats),
                super::Entry::Staged(_) => false,
            })
        };
        match self {
            Self::Bind(_) | Self::Computed(_) | Self::Hole | Self::Rest(_) => true,
            Self::At { pat, .. } => pats[*pat].kind.irrefutable(pats),
            Self::Or(alternatives) => alternatives.iter().any(|p| pats[*p].kind.irrefutable(pats)),
            Self::Tuple(items) => all(items),
            Self::Struct { ty: None, fields } => fields.iter().all(|e| match e {
                super::Entry::One((_, p)) => pats[*p].kind.irrefutable(pats),
                super::Entry::Staged(_) => false,
            }),
            Self::Struct { .. }
            | Self::Variant { .. }
            | Self::Range { .. }
            | Self::Value(_)
            | Self::Error => false,
        }
    }
}
