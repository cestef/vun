//! Which parameter each written argument of a call is for: one each, but for the `$` parameters
//! left out and a pack, which takes every argument after the others (spec 01, section 4).

use vun_sema_ty::{Ty, TyKind};

use super::Named;
use crate::callable::Callable;
use crate::env::Env;

/// Which parameter each written argument of a call is for.
pub struct Plan {
    /// The parameter of each written argument, the receiver's first; the arguments of a pack
    /// each have its parameter.
    pub slots: Vec<usize>,
    /// The `$` parameters left out, to infer.
    pub omitted: Vec<usize>,
    /// The parameter taking the arguments past the others as a tuple, when the function has a
    /// pack: none of them, one, or more.
    pub pack: Option<usize>,
}

impl Plan {
    /// How `written` arguments, the receiver included, go to the parameters of `f`: one each,
    /// or one each but for every `$` parameter a later parameter's type names; the arguments
    /// past the others to its pack, when it has one. With a pack, those `$` parameters are left
    /// out when a call can leave them out.
    #[must_use]
    pub fn of(env: &Env<'_, '_>, f: &Callable, written: usize) -> Option<Self> {
        let inferable = Self::inferable(env, f)?;
        let pack = f.written(env)?.pack();
        let all = Self::spread((0..inferable.len()).collect(), pack, written).map(|slots| Self {
            slots,
            omitted: Vec::new(),
            pack,
        });
        let omitted: Vec<usize> = (0..inferable.len()).filter(|&i| inferable[i]).collect();
        let kept: Vec<usize> = (0..inferable.len()).filter(|&i| !inferable[i]).collect();
        let fewer = Some(omitted.clone())
            .filter(|o| !o.is_empty())
            .and_then(|omitted| Some(Self { slots: Self::spread(kept, pack, written)?, omitted, pack }));
        if pack.is_some() { fewer.or(all) } else { all.or(fewer) }
    }

    /// The plan of a call passing `written` arguments to a function known only by its
    /// parameters' types `params`: one each, those after the others to its pack.
    #[must_use]
    pub fn typed(params: &[Ty], written: usize) -> Option<Self> {
        let pack = params.len().checked_sub(1).filter(|&last| params[last].packed().is_some());
        let slots = Self::spread((0..params.len()).collect(), pack, written)?;
        Some(Self { slots, omitted: Vec::new(), pack })
    }

    /// The parameters `params` taken by `written` arguments in order: one each, and every one
    /// past the others by the last when it is the pack `pack`.
    fn spread(params: Vec<usize>, pack: Option<usize>, written: usize) -> Option<Vec<usize>> {
        match params.split_last() {
            Some((&last, fixed)) if Some(last) == pack && written >= fixed.len() => {
                Some(fixed.iter().copied().chain(std::iter::repeat_n(last, written - fixed.len())).collect())
            }
            _ => (params.len() == written).then_some(params),
        }
    }

    /// The argument counts a call of `f` may pass, the receiver included: every parameter, or
    /// every one but the inferable `$` parameters when there are some; with a pack, the counts
    /// of its other parameters, the least it may pass.
    #[must_use]
    pub fn counts(env: &Env<'_, '_>, f: &Callable) -> Vec<usize> {
        let Some(inferable) = Self::inferable(env, f) else { return Vec::new() };
        let pack = usize::from(f.written(env).and_then(|w| w.pack()).is_some());
        let without = inferable.iter().filter(|&&i| !i).count();
        let counts =
            if without == inferable.len() { vec![inferable.len()] } else { vec![inferable.len(), without] };
        counts.into_iter().map(|n| n - pack).collect()
    }

    /// The type of the value each parameter a written argument goes to receives, from the
    /// written arguments' types `types`: an argument's own, and for the pack the tuple of the
    /// types of its arguments, `()` for none.
    #[must_use]
    pub fn grouped(&self, types: &[Option<Ty>]) -> Vec<(usize, Option<Ty>)> {
        let args = self.slots.iter().zip(types);
        let mut grouped: Vec<(usize, Option<Ty>)> = args
            .clone()
            .filter(|(slot, _)| Some(**slot) != self.pack)
            .map(|(&slot, &ty)| (slot, ty))
            .collect();
        if let Some(pack) = self.pack {
            let items = args.filter(|(slot, _)| **slot == pack).map(|(_, &ty)| ty);
            let tuple = items.collect::<Option<Vec<Ty>>>().map(|t| TyKind::Tuple(t).intern());
            grouped.push((pack, tuple));
        }
        grouped
    }

    /// Which item of the pack each written argument is, for those of the pack.
    #[must_use]
    pub fn items(&self) -> Vec<Option<usize>> {
        let mut next = 0;
        let item = |&slot: &usize| {
            (Some(slot) == self.pack).then(|| {
                next += 1;
                next - 1
            })
        };
        self.slots.iter().map(item).collect()
    }

    /// Whether each parameter of `f` is a `$` parameter named in a later parameter's type.
    fn inferable(env: &Env<'_, '_>, f: &Callable) -> Option<Vec<bool>> {
        let written = f.written(env)?;
        let inside = &written.env;
        let hir = &inside.parsed.hir;
        let params = written.params()?;
        Some(
            params
                .iter()
                .enumerate()
                .map(|(i, (binder, _))| {
                    binder.filter(|&b| hir.binders[b].comptime).is_some_and(|b| {
                        params[i + 1..].iter().any(|(_, ty)| ty.is_some_and(|t| Named::names(inside, t, b)))
                    })
                })
                .collect(),
        )
    }
}
