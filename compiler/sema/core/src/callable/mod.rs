//! What a call runs (spec 01, section 4): a function value, a primitive declared in `lang` with
//! its function type (decision 66), or a link symbol declared with one (spec 08, section 1). All
//! write their parameters the same way, `$` ones included, so a call reads from where they are
//! written which arguments are known at compile time, which a call may leave out, and in which
//! mode each is borrowed.

mod func;
mod written;

use vun_codec::{Decode, Encode};
use vun_desugar::Borrow;
use vun_hir::tree::ExprId;
use vun_primitives::Primitive;
use vun_sema_ty::call::{Func, Instantiation};
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Signature, Ty, TyKind, Value};

pub use func::{Generic, Parameters};

use crate::env::Env;
use crate::eval::Signed;
use crate::link::Linkable;

/// A function a call runs.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Callable {
    Fn(Func),
    Primitive(Primitive),
    /// A function the linker provides.
    Linked(DeclRef),
}

/// Where a callable's parameters are written: the environment of their file, and the function
/// literal or function type listing them.
pub struct Written<'a, 'db> {
    pub env: Env<'a, 'db>,
    pub expr: ExprId,
}

impl Env<'_, '_> {
    /// The type of `value`, a function's included.
    ///
    /// # Errors
    /// Returns why its type is not known here.
    pub fn value_type(&self, value: &Value) -> Result<Ty, Unevaluated> {
        match value.ty() {
            Some(ty) => Ok(ty),
            None => Callable::of(self, value.clone()).ok_or(Unevaluated::Unsupported)?.ty(self),
        }
    }
}

impl Callable {
    /// The callable a value is, if it is one. A declaration provided from outside is a function
    /// the linker provides when it names a symbol; otherwise it is a requirement an impl provides.
    #[must_use]
    pub fn of(env: &Env<'_, '_>, value: Value) -> Option<Self> {
        match value {
            Value::Fn(f) => Some(Self::Fn(f)),
            Value::Primitive(p) => Some(Self::Primitive(p)),
            Value::Linked(decl) => decl.linked(env.cx).map(|_| Self::Linked(decl)),
            _ => None,
        }
    }

    /// The value calling it.
    #[must_use]
    pub fn value(&self) -> Value {
        match self {
            Self::Fn(f) => Value::Fn(f.clone()),
            Self::Primitive(p) => Value::Primitive(*p),
            Self::Linked(decl) => Value::Linked(decl.clone()),
        }
    }

    /// The declaration whose annotation gives its signature, when it has no literal.
    fn declaration(&self, env: &Env<'_, '_>) -> Option<DeclRef> {
        match self {
            Self::Fn(_) => None,
            Self::Primitive(p) => env.primitive_decl(*p).ok(),
            Self::Linked(decl) => Some(decl.clone()),
        }
    }

    /// The signature its annotation gives for a call whose `$` arguments have the values `args`:
    /// a function written as a literal has none, its signature is typed per instantiation.
    ///
    /// # Errors
    /// Returns why it is not known here, such as a type naming a `$` parameter not bound.
    pub fn declared_signature(
        &self,
        env: &Env<'_, '_>,
        args: &[Option<Value>],
    ) -> Result<Signature, Unevaluated> {
        env.declared_signature(&self.declaration(env).ok_or(Unevaluated::Unsupported)?, args)
    }

    /// Where its parameters are written, as `env` finds it.
    #[must_use]
    pub fn written<'a, 'db>(&self, env: &Env<'a, 'db>) -> Option<Written<'a, 'db>> {
        match self {
            Self::Fn(f) => f.written(env.cx),
            Self::Primitive(_) | Self::Linked(_) => env.declared_written(&self.declaration(env)?),
        }
    }

    /// The parameter gathering the arguments after the others: see [`Written::pack`].
    #[must_use]
    pub fn pack(&self, env: &Env<'_, '_>) -> Option<usize> {
        self.written(env)?.pack()
    }

    /// Whether each parameter is a `$` parameter, known at compile time.
    #[must_use]
    pub fn comptime(&self, env: &Env<'_, '_>) -> Option<Vec<bool>> {
        self.written(env)?.comptime()
    }

    /// Its type: its signature's, or its own when it has `$` parameters.
    ///
    /// # Errors
    /// Returns why its signature is not known here, when it has none of its own.
    pub fn ty(&self, env: &Env<'_, '_>) -> Result<Ty, Unevaluated> {
        let signature = match self {
            Self::Fn(f) => env.cx.get::<Signed>(&Instantiation::new(f.clone(), Vec::new())),
            Self::Primitive(_) | Self::Linked(_) => self.declared_signature(env, &[]),
        };
        signature.map(|s| s.ty()).or_else(|why| self.generic(env).ok_or(why))
    }

    /// Its type when it has `$` parameters: itself, having no signature apart from the `$`
    /// arguments of a call ([`TyKind::Generic`]).
    #[must_use]
    pub fn generic(&self, env: &Env<'_, '_>) -> Option<Ty> {
        self.comptime(env)?.contains(&true).then(|| TyKind::Generic(Box::new(self.value())).intern())
    }

    /// Whether a call of it is known when compiling, whatever runs: one whose parameters are all
    /// `$` ones, so that nothing at run time changes its value. A primitive without parameters
    /// is such a call; a function needs a `$` parameter to be, or a call of it is a run-time one;
    /// a function the linker provides runs at run time.
    #[must_use]
    pub fn folds(&self, env: &Env<'_, '_>) -> bool {
        self.comptime(env)
            .is_some_and(|c| (matches!(self, Self::Primitive(_)) || !c.is_empty()) && c.iter().all(|&c| c))
            && !matches!(self, Self::Linked(_))
    }

    /// The borrow mode each parameter's annotation writes: see [`Written::modes`].
    #[must_use]
    pub fn modes(&self, env: &Env<'_, '_>) -> Option<Vec<Option<Borrow>>> {
        self.written(env)?.modes()
    }

    /// Whether each parameter receives its argument as code: see [`Written::quotes`].
    #[must_use]
    pub fn quotes(&self, env: &Env<'_, '_>) -> Option<Vec<bool>> {
        self.written(env)?.quotes()
    }

    /// Each parameter's type, where known before a call: see [`Written::types`].
    #[must_use]
    pub fn types(&self, env: &Env<'_, '_>) -> Option<Vec<Option<Ty>>> {
        self.written(env)?.types()
    }
}
