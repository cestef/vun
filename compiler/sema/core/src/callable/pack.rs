//! A pack under the C convention (spec 07, section 5): its items are the variadic arguments of a
//! C function, `...`, which Vun passes and never reads, each as C's default promotions leave it.

use vun_diag::{Diagnostic, Msg, errors};
use vun_origin::OriginId;
use vun_sema_float::Format;
use vun_sema_ty::float::FloatType;
use vun_sema_ty::{Signature, Ty};

/// What a signature's pack is under the C convention.
pub trait Variadic {
    /// The first item of the pack that C promotes to a `double`, which a pack cannot pass as it
    /// is: its arguments are variadic ones only under the C convention. The backends extend an
    /// integer narrower than an `int`.
    fn promoted(&self) -> Option<Ty>;
}

impl Variadic for Signature {
    fn promoted(&self) -> Option<Ty> {
        let items = self.params.last().and_then(|p| p.packed())?;
        let items = items.as_tuple().filter(|_| self.is_c())?;
        items.iter().find(|p| Format::of(p).is_some_and(|f| f != Format::F64)).copied()
    }
}

/// An item of a pack that C promotes.
pub struct Promoted(pub Ty);

impl Promoted {
    #[must_use]
    pub fn report(&self, at: OriginId) -> Diagnostic {
        let msg = Msg::new().text("C promotes a variadic ").ty(self.0.to_string()).text(" to ");
        let msg = msg.code("f64").text(", which this item of the pack must be");
        Diagnostic::error(errors::ABI_VARIADIC, msg)
            .at(at, None)
            .help("declare the item as `f64`, which C passes")
    }
}
