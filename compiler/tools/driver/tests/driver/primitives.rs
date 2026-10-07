//! Every primitive `lang` declares that a program can call at run time is called by a program of
//! the corpus or of these tests, whose functions all lower (spec 13, section 3).

use std::collections::HashSet;
use std::path::PathBuf;

use vun_driver::{Session, Shared};
use vun_fs::Tree;
use vun_harness::{ACCEPT, repo_root};
use vun_mir::{Concrete, Operand, Rvalue};
use vun_par::Parallel;
use vun_primitives::Primitive;
use vun_primitives::lower::Lower;
use vun_query::Cx;
use vun_sema::attributes::Attributed;
use vun_sema::callable::Callable;
use vun_sema::env::Env;
use vun_sema::link::Linkable;
use vun_sema_ty::Value;
use vun_span::{EXTENSION, FileId};

/// What the programs of the corpus and of these tests call, and what a program could.
#[derive(Default)]
struct Calls {
    called: HashSet<Primitive>,
    /// The primitives a program may call at run time, as the first program read says.
    callable: Option<Vec<Primitive>>,
}

impl Calls {
    fn programs() -> Vec<PathBuf> {
        let own = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/driver/programs");
        let corpus = repo_root().join(ACCEPT);
        [corpus, own].iter().flat_map(|dir| dir.files_with(EXTENSION).unwrap()).collect()
    }

    fn of(session: &mut Session, file: FileId) -> Self {
        let program = session.program(file);
        let bodies: Vec<_> =
            program.instantiations().filter_map(|i| session.db.get::<Concrete>(i).ok()).collect();
        let called = bodies
            .iter()
            .flat_map(|body| body.blocks.iter().flat_map(|(_, b)| &b.stmts))
            .filter_map(|stmt| match &stmt.value {
                Rvalue::Call { callee: Operand::Const(Value::Primitive(p)), .. } => Some(*p),
                _ => None,
            })
            .collect();
        let key = session.file_ref(file);
        let cx = Cx::new(&session.db);
        let callable =
            Env::of(&cx, &key).map(|env| Primitive::all().filter(|&p| Self::runs(&env, p)).collect());
        Self { called, callable }
    }

    /// Whether a call of `p` stays in the code a program runs: it is a function, which nothing
    /// known when compiling computes, which neither folds into a call of another function
    /// (`lower.by`) nor is the linker's, and not one of the namespaces that exist when compiling.
    fn runs(env: &Env<'_, '_>, p: Primitive) -> bool {
        let compile_time =
            matches!(p, Primitive::Arch(_) | Primitive::Link(_) | Primitive::Lint(_) | Primitive::Meta(_));
        let Ok(decl) = env.primitive_decl(p) else { return false };
        let folded = decl
            .attributes(env.cx)
            .is_some_and(|attributes| attributes.get(Primitive::Lower(Lower::by)).is_some());
        let function = Callable::Primitive(p).ty(env).is_ok();
        function
            && !compile_time
            && !folded
            && decl.linked(env.cx).is_none()
            && !Callable::Primitive(p).folds(env)
    }

    fn merge(mut self, other: Self) -> Self {
        self.called.extend(other.called);
        self.callable = self.callable.or(other.callable);
        self
    }
}

#[test]
fn every_primitive_a_program_can_call_is_called_by_a_program_that_lowers() {
    let shared = Shared::default();
    let calls = Calls::programs()
        .par_map(|path| {
            let mut session = shared.session(path);
            session.load(path).map_or_else(Calls::default, |file| Calls::of(&mut session, file))
        })
        .into_iter()
        .fold(Calls::default(), Calls::merge);
    let callable = calls.callable.expect("a program was read");
    let unreached: Vec<_> = callable.iter().filter(|p| !calls.called.contains(p)).copied().collect();
    assert!(unreached.is_empty(), "{:?}", unreached.iter().map(|p| p.item()).collect::<Vec<_>>());
}
