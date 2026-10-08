//! The kinds of the nodes a made form is read back as, in preorder: what printing must preserve
//! (spec 13, section 5), so that a test or the smith compares it with the tree the parser reads.

use vun_grammar::NodeKind;

use super::{Arm, Bind, Block, Expr, Field, Function, Init, Item, Member, Param, Variant};

/// A form that pushes the kinds of the nodes it is read back as.
trait Visit {
    fn visit(&self, out: &mut Vec<NodeKind>);
}

impl<T: Visit> Visit for Box<T> {
    fn visit(&self, out: &mut Vec<NodeKind>) {
        (**self).visit(out);
    }
}

/// The kinds of each of `items`, in order.
fn visit_all<'a, T: Visit + 'a>(items: impl IntoIterator<Item = &'a T>, out: &mut Vec<NodeKind>) {
    for item in items {
        item.visit(out);
    }
}

impl Expr {
    /// The kinds of this form and everything in it, in source order, parentheses left out.
    #[must_use]
    pub fn kinds(&self) -> Vec<NodeKind> {
        let mut kinds = Vec::new();
        self.visit(&mut kinds);
        kinds
    }

    /// The control forms and bindings.
    fn visit_control(&self, out: &mut Vec<NodeKind>) {
        match self {
            Self::Bind(bind) => bind.visit_after(out),
            Self::If(condition, then, otherwise) => {
                condition.visit(out);
                out.push(NodeKind::Block);
                then.visit(out);
                visit_all(otherwise, out);
            }
            Self::For(head, body) => {
                visit_all(head, out);
                out.push(NodeKind::Block);
                body.visit(out);
            }
            Self::Match(scrutinee, arms) => {
                scrutinee.visit(out);
                visit_all(arms, out);
            }
            Self::Labeled(_, body) => body.visit(out),
            Self::Return(value) => visit_all(value, out),
            Self::Break(label, value) => {
                out.extend(label.as_ref().map(|_| NodeKind::Label));
                visit_all(value, out);
            }
            Self::Continue(Some(_)) => out.push(NodeKind::Label),
            _ => {}
        }
    }

    fn visit_result(result: Option<&Self>, out: &mut Vec<NodeKind>) {
        if let Some(result) = result {
            out.push(NodeKind::Returns);
            result.visit(out);
        }
    }
}

impl Visit for Expr {
    fn visit(&self, out: &mut Vec<NodeKind>) {
        out.push(self.kind());
        match self {
            Self::Int(n) if n.negative => out.push(NodeKind::Int),
            Self::Float(bits) if f64::from_bits(*bits).is_sign_negative() => out.push(NodeKind::Float),
            Self::Path(path) if path.len() > 1 => {
                // `a.b.c` is `(a.b).c`: a member for each name after the first, then the names.
                out.extend(std::iter::repeat_n(NodeKind::Member, path.len() - 2));
                out.extend(std::iter::repeat_n(NodeKind::Name, path.len()));
            }
            Self::Int(_)
            | Self::Float(_)
            | Self::Char(_)
            | Self::String(_)
            | Self::Hole
            | Self::Unit
            | Self::Path(_)
            | Self::Continue(None) => {}
            Self::DotName(_) => out.push(NodeKind::Name),
            Self::Tuple(items) | Self::List(items) => visit_all(items, out),
            Self::Repeat(a, b)
            | Self::ArrayType(a, b)
            | Self::Index(a, b)
            | Self::Test(a, b)
            | Self::In(a, b) => {
                a.visit(out);
                b.visit(out);
            }
            Self::Record(inits) => visit_all(inits, out),
            Self::Typed(ty, inits) => {
                ty.visit(out);
                visit_all(inits, out);
            }
            Self::Block(block) => block.visit(out),
            Self::FnType(params, result) => {
                visit_all(params, out);
                Self::visit_result(result.as_deref(), out);
            }
            Self::Struct(members) => visit_all(members, out),
            Self::Enum(variants, members) => {
                visit_all(variants, out);
                if !members.is_empty() {
                    out.push(NodeKind::Namespace);
                    visit_all(members, out);
                }
            }
            Self::Function(function) => function.visit(out),
            Self::Closure(captures, function) => {
                visit_all(captures, out);
                out.push(NodeKind::Fn);
                function.visit(out);
            }
            Self::Member(object, property) => {
                object.visit(out);
                out.push(match property {
                    super::Property::Name(_) => NodeKind::Name,
                    super::Property::Index(_) => NodeKind::Int,
                });
            }
            Self::Call(callee, args) => {
                callee.visit(out);
                visit_all(args, out);
            }
            Self::Prefix(_, inner)
            | Self::Postfix(_, inner)
            | Self::Yield(inner)
            | Self::Defer(inner)
            | Self::Staged(inner) => inner.visit(out),
            Self::Borrow(borrow, inner) => {
                out.extend(borrow.qualifier.map(|_| NodeKind::Qualifier));
                out.extend(borrow.mode.map(|_| NodeKind::Mode));
                inner.visit(out);
            }
            Self::Infix(_, left, right) => {
                left.visit(out);
                visit_all(right, out);
            }
            Self::Assign(_, place, value) => {
                place.visit(out);
                value.visit(out);
            }
            Self::Attributed(attrs, inner) => {
                visit_all(attrs, out);
                inner.visit(out);
            }
            _ => self.visit_control(out),
        }
    }
}

impl Visit for Block {
    /// What a block holds, its own kind pushed by the caller.
    fn visit(&self, out: &mut Vec<NodeKind>) {
        visit_all(&self.stmts, out);
        visit_all(&self.tail, out);
    }
}

impl Visit for Function {
    /// What a function holds, its own kind pushed by the caller: parameters, result, body.
    fn visit(&self, out: &mut Vec<NodeKind>) {
        visit_all(&self.params, out);
        Expr::visit_result(self.result.as_deref(), out);
        out.push(NodeKind::Block);
        self.body.visit(out);
    }
}

impl Visit for Param {
    fn visit(&self, out: &mut Vec<NodeKind>) {
        match &self.pattern {
            Some(pattern) => {
                out.push(NodeKind::Param);
                pattern.visit(out);
                self.ty.visit(out);
            }
            None => self.ty.visit(out),
        }
    }
}

impl Visit for Init {
    fn visit(&self, out: &mut Vec<NodeKind>) {
        match self {
            Self::Item(value) => value.visit(out),
            Self::Named(_, value) => {
                out.extend([NodeKind::FieldInit, NodeKind::DotName, NodeKind::Name]);
                value.visit(out);
            }
        }
    }
}

impl Visit for Member {
    fn visit(&self, out: &mut Vec<NodeKind>) {
        match self {
            Self::Field(field) => field.visit(out),
            Self::Item(item) => item.visit(out),
        }
    }
}

impl Visit for Field {
    fn visit(&self, out: &mut Vec<NodeKind>) {
        out.extend([NodeKind::Field, NodeKind::Name]);
        self.ty.visit(out);
        visit_all(&self.default, out);
    }
}

impl Visit for Variant {
    fn visit(&self, out: &mut Vec<NodeKind>) {
        if self.payload.is_empty() {
            out.push(NodeKind::Name);
        } else {
            out.extend([NodeKind::Call, NodeKind::Name]);
            visit_all(&self.payload, out);
        }
    }
}

impl Visit for Arm {
    fn visit(&self, out: &mut Vec<NodeKind>) {
        out.push(NodeKind::Arm);
        self.pattern.visit(out);
        if let Some(guard) = &self.guard {
            out.push(NodeKind::Guard);
            guard.visit(out);
        }
        visit_all(&self.value, out);
    }
}

impl Bind {
    /// What a binding holds, its own kind pushed by the caller.
    fn visit_after(&self, out: &mut Vec<NodeKind>) {
        self.pattern.visit(out);
        visit_all(&self.ty, out);
        visit_all(&self.value, out);
    }
}

impl Visit for Item {
    /// The kinds of this declaration: docs and attributes around its binding.
    fn visit(&self, out: &mut Vec<NodeKind>) {
        out.extend(self.doc.iter().map(|_| NodeKind::Doc));
        if !self.attrs.is_empty() {
            out.push(NodeKind::Attr);
            visit_all(&self.attrs, out);
        }
        out.push(if self.bind.ty.is_some() { NodeKind::TypedBinding } else { NodeKind::Binding });
        self.bind.visit_after(out);
    }
}
