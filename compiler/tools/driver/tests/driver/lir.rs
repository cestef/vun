//! Lowering programs to LIR (spec 13, section 3): what the exports reach becomes LIR that
//! verifies.

use vun_driver::Session;
use vun_harness::{ACCEPT, repo_root};
use vun_lir::{Conv, Linkage};
use vun_sema_ty::trap::Failure;

use crate::common::Fixture;

#[test]
fn a_hosted_program_lowers_to_lir_that_verifies() {
    let mut session = Session::new();
    let file = session.load(&repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    let lowered = session.lir(file);
    let unit = lowered.as_ref().as_ref().unwrap();
    assert_eq!(unit.verify(session.origins), Ok(()));
    // Linux programs start at the kernel runtime's `_start`, entered as its code.
    let entry = unit
        .functions
        .iter()
        .find(|(_, f)| matches!(f.linkage, Linkage::Export(s) if s.as_str() == "_start"));
    assert_eq!(entry.map(|(_, f)| f.signature.conv), Some(Conv::Naked));
}

/// The most instructions a function of a hosted program has: its largest have a few hundred, and a
/// form lowered item by item where a loop fits (`[0; 4096]` was twelve thousand) goes far past,
/// slowing every pass and every build of every program that reaches it.
const LARGEST: usize = 2000;

#[test]
fn no_function_of_a_hosted_program_is_larger_than_its_budget() {
    let fixture = Fixture::new("lir-budget");
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/files.vun")).unwrap();
    let path = fixture.root.write("main.vun", &text);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let lowered = session.lir(file);
    let unit = lowered.as_ref().as_ref().unwrap_or_else(|_| panic!("{:#?}", session.diag));
    for (_, f) in unit.functions.iter() {
        let name = session.origins.declaration_name(f.origin);
        assert!(f.insts.len() <= LARGEST, "{name:?} has {} instructions", f.insts.len());
    }
}

#[test]
fn a_repeated_item_lowers_to_the_same_code_whatever_its_count() {
    let sizes = [1 << 12, 1 << 16].map(|count| {
        let fixture = Fixture::new(&format!("lir-repeat-{count}"));
        let source = format!(
            "#[link.export(\"fill\")]\nfill := (x: u8) -> u8 {{ xs: [{count}]u8 = [x; {count}]; xs[1] }};\n"
        );
        let path = fixture.root.write("p.vun", &source);
        let mut session = fixture.session();
        let file = session.load(&path).unwrap();
        let lowered = session.lir(file);
        let unit = lowered.as_ref().as_ref().unwrap_or_else(|_| panic!("{:#?}", session.diag));
        unit.functions.iter().map(|(_, f)| f.insts.len()).sum::<usize>()
    });
    assert_eq!(sizes[0], sizes[1]);
}

#[test]
fn integers_arrays_and_runs_lower_with_their_checks() {
    let fixture = Fixture::new("lir-lowering");
    let path = fixture.root.write(
        "p.vun",
        "\
#[link.export(\"mix\"), abi.c]
mix := (x: u32, y: u32) -> u32 { (x ^ y) & 0xff };
#[link.export(\"item\")]
item := (xs: [4]u8, i: usize) -> u8 { xs[i] };
#[link.export(\"tail\")]
tail := (xs: &[u8], n: usize) -> usize { xs[n..].len() };
",
    );
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let lowered = session.lir(file);
    let unit = lowered.as_ref().as_ref().unwrap();
    assert_eq!(unit.verify(session.origins), Ok(()));
    // Indexing and slicing are trap sites.
    assert_eq!(unit.sites.len(), 2);
}

#[test]
fn a_frame_holds_its_state_parameters_and_what_lives_across_a_yield() {
    let fixture = Fixture::new("lir-frames");
    let path = fixture.root.write(
        "p.vun",
        "\
start := std.os.start(main);
counter := (n: u8) -> std.iter.Iter(u8) { mut a: u8 = n; yield a; a ^= 1; yield a; };
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut total: u8 = 0;
    for x in counter(5) { total ^= x; }
    std.os.Exit.of(total % 126)
};
",
    );
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let program = session.program(file);
    let root = session.file_ref(file);
    // `main`'s frame: the runtime's bodies hold frames of their own loops.
    let frame = program
        .instantiations()
        .filter(|i| i.f.literal.file == root)
        .filter_map(|i| session.db.get::<vun_mir::Concrete>(i).ok())
        .flat_map(|body| body.locals.iter().filter_map(|(_, l)| l.ty).collect::<Vec<_>>())
        .find(|ty| matches!(&**ty, vun_sema_ty::TyKind::Frame(_)))
        .expect("main holds a frame");
    let shaped = session.db.get::<vun_lower::Frames>(&frame);
    let shape = shaped.as_ref().as_ref().unwrap();
    assert_eq!((shape.state, shape.points.len()), (vun_lir::Scalar::I8, 2));
    let mut offsets: Vec<u64> = shape.stored.iter().filter_map(|(_, o)| *o).collect();
    offsets.sort_unstable();
    assert_eq!(offsets, [1, 2]);
    assert_eq!((shape.layout.size, shape.layout.align), (3, 1));
}

#[test]
fn an_assertion_expands_at_its_call_and_its_site_says_what_failed() {
    let fixture = Fixture::new("lir-assert");
    let path = fixture.root.write(
        "p.vun",
        "\
#[link.export(\"small\"), abi.c]
small := (x: u8) -> u8 { assert(x < 10); x };
",
    );
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let lowered = session.lir(file);
    let unit = lowered.as_ref().as_ref().unwrap();
    assert_eq!(unit.verify(session.origins), Ok(()));
    // The assertion is built into `small`, which alone calls the trap handler.
    assert_eq!(unit.functions.len(), 1);
    let [site] = unit.sites.as_slice() else { panic!("{:?}", unit.sites) };
    let failure = Failure::Assertion { source: "x < 10".into(), at: site.at };
    assert_eq!(site.msg, Some(failure.msg()));
}

/// An atomic access of a 64-bit integer, which a processor reaching one word cannot make.
const WIDE_ATOMIC: &str = "\
#[link.export(\"bump\"), abi.c]
bump := (p: *u64) -> u64 { unsafe lang.mem.atomic_add(u64, p, 1, .relaxed) };
";

#[test]
fn what_library_code_reports_for_a_target_is_shown_as_it_reports_it() {
    let fixture = Fixture::new("lir-library-report");
    let path = fixture.root.write("bump.vun", WIDE_ATOMIC);
    let mut session = fixture.session();
    session.choice = vun_sema::target::Choice::of(vun_items::targets::bare_riscv32imc::ilp32);
    let file = session.load(&path).unwrap();
    let refused = session.lir(file).as_ref().as_ref().expect_err("riscv32 has no 64-bit atomics").clone();
    let d = refused.diagnostic();
    assert_eq!(d.id, vun_diag::errors::COMPTIME_REPORTED);
    assert_eq!(d.message.to_string(), "no atomic access of an integer wider than an address on this target");
}

#[test]
fn a_small_function_calling_itself_is_inlined_into_itself() {
    use vun_lir::{Callee, InstKind};

    let fixture = Fixture::new("lir-recursion");
    let path = fixture.root.write(
        "p.vun",
        "\
fib := (n: u32) -> u64 { if n < 2 { n.to(u64) } else { (fib(n - 1) + fib(n - 2)).wrap(u64) } };
#[link.export(\"run\")]
run := (n: u32) -> u64 { fib(n) };
",
    );
    let mut session = fixture.session();
    // A debug build keeps the calls of its trace, so inlines no call that returns into it.
    session.set_trace(vun_target::Detail::Sites);
    let file = session.load(&path).unwrap();
    let lowered = session.lir(file);
    let unit = lowered.as_ref().as_ref().unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert_eq!(unit.verify(session.origins), Ok(()));
    let own_calls = |id, f: &vun_lir::Function| {
        f.insts
            .iter()
            .filter(|(_, i)| matches!(&i.kind, InstKind::Call { callee: Callee::Func(g), .. } if *g == id))
            .count()
    };
    let most = unit.functions.iter().map(|(id, f)| own_calls(id, f)).max();
    // One round leaves two calls of itself in each of the two bodies it was inlined for.
    assert!(most.is_some_and(|n| n > 2), "{most:?}");
}
