# 11. Testing, fuzzing, benchmarking

Built in, without a keyword and without global constructors. Research notes, section 6, has the
survey behind these choices.

## 1. Tests are bindings

A test is an ordinary binding marked `#[test]`. The attribute is `std.testing.test`, plain data: the
test runner is library code, `std.testing.runner(ns)`, which finds every `#[test]` binding of a
namespace and the namespaces in it by reflection (`std.meta.collect`) and gives the exports of an
ordinary program (spec 08): `start := std.testing.runner(app);`. Nothing registers anything at
load time.

The runner runs each test on a thread of its own and joins it: a trap ends that thread alone and
joining gives the trap (spec 07, section 3), so a failing test ends only itself. Where the system
runs one thread, tests run on the calling one and the first trap ends the run. The program's
arguments, when it has some, are the names of the tests to run.

`vun test [path] [names]...` builds that program for a namespace without source naming it: the
driver calls `std.testing.runner` at compile time with the namespace as a value, and the program's
exports are the members of the type it gives (a program's roots may be a type's members as well
as a root file's, decision 355). The path is a file, a directory or a package's directory, the
package around the current directory when there is none; a file outside any package is in no
namespace (`test.no_namespace`).

**The parameters select what kind of test it is:**

```vun
#[test]
crc_vector := () { assert(crc32("123456789") == 0xCBF4_3926); };

#[test]                                            // property: inputs generated from the types
roundtrip := (p: Packet) { assert(Packet.decode(&p.encode()) == .ok(p)); };

#[test]                                            // fuzz target: raw bytes
decode_raw := (input: &[u8]) { _ = Packet.decode(input); };

#[test]                                            // benchmark
memcpy_4k := (b: &mut testing.Bench) {
    src: [4096]u8 = [0; 4096];
    mut dst: [4096]u8;
    for b.loop() { mem.copy(&out dst, &src); b.keep(&dst); }
};

#[test]                                            // deterministic simulation
election := (s: &mut testing.Sim) {
    net := s.network(.{ .drop = .percent(5), .reorder = true });
    for i in 0..3 { s.spawn(node(net, i)); }
    s.run_until(.virtual(60 * s.second));
    assert(leaders(s) == 1);
};
```

| Parameters | Kind | Run by |
|------------|------|--------|
| none | unit test | `vun test` |
| values of ordinary types | property test | `vun test` (seeded cases), `vun fuzz` (coverage-guided) |
| `&[u8]` | fuzz target | same |
| `&mut testing.Bench` | benchmark | `vun bench` |
| `&mut testing.Sim` | simulation | `vun test`, `vun sim --seeds N` |

Attributes: `#[test.fuel(n)]` sets a deterministic step budget instead of a wall-clock timeout,
where a mode counts steps (`interp`); `#[test.ignore("why")]` keeps a test from running, the
runner reporting it with its reason (`std.testing.Ignored`, read with `std.meta.attr`);
`#[test.on(.target)]` restricts a test to target runs.

`assert` takes `Code(bool)` (spec 03), so a failure prints the asserted expression and its span.

## 2. Inputs from types

`testing.gen(T)` derives a generator from `T` by reflection: structs field by field, enums by
variant, arrays and slices by length and element. It reads from a typed **choice stream**, so:

- range types bound generation for free: an `Int(0..=100)` is never 101;
- shrinking works on the choice stream, so custom generators shrink too;
- the same test runs as a seeded property test and under a coverage-guided fuzzer, sharing one
  corpus.

A type overrides its generator by implementing `testing.Arbitrary`. A single test picks one with
`#[test.gen(...)]` on a parameter.

Corpus and crash inputs live in `fuzz/<test path>/`, are committed, and replay as regressions on
every `vun test`.

## 3. Execution modes

`vun test --on <mode>`:

- **`interp`** (default): the reference interpreter. It traps deterministically on every item of
  appendix C in `unsafe` code, including aliasing violations (spec 05, section 8), so every test run
  is also an undefined-behavior check.
- **`native`**: compiled for the host, for speed.
- **`qemu`**, **`probe`**: compiled for the target, run in an emulator or on a board. The test binary
  is `std.testing.runner(pkg)` wrapped in the board's runtime; it flashes once, resets between
  tests, streams results back over deferred logging (spec 12), and exits through semihosting or a
  breakpoint.

All modes emit the same JSON result stream, so editors run single tests the same way everywhere:
one line per test run, `{"test":{"name":...,"outcome":...}}`, its outcome `"passed"`,
`{"ignored":{"reason":...}}` or
`{"trapped":{"site":n,"said":{"message":...,"at":{"file":...,"line":...,"column":...}}}}` as the
program's table says it (`std.trace.Site.of`), then the totals,
`{"totals":{"passed":n,"trapped":n,"ignored":n}}`. The lines are `std.testing.Line` written by
`std.fmt.json.derive`, read by the driver as `vun_driver::testing::Line`. `vun test` reads the
program's standard output and error as one stream, so what a test writes, the backtrace of its
trap included, comes before its line and is reported with it. Only unit tests run yet.

## 4. Deterministic simulation

Time, network, storage and randomness reach code only through capabilities (spec 07), and executors
are library code (spec 06). `testing.Sim` supplies simulated versions of all of them: a virtual
clock, a seeded scheduler that chooses which ready task resumes, and a network and disk that drop,
reorder, delay and corrupt on command. A failure prints its seed; `--seed` replays it exactly. This
is TigerBeetle's and FoundationDB's approach, with no runtime support.

## 5. Benchmarks

- `b.loop()` runs setup once, prevents the loop body from being optimized away, and hides the
  iteration count from the code (the semantics Go adopted in 1.24).
- `--measure time` (default natively): statistics over wall-clock samples.
- `--measure instructions`: counts LIR operations in the interpreter; deterministic, suitable as a CI
  gate. Natively, hardware counters where available; on Cortex-M, the DWT cycle counter.
- Baselines live in `.vun/bench/`; `--fail-over 3%` fails the run on regression.

## 6. Other checks

- **Snapshots**: `testing.snapshot(v)` renders a value by reflection into
  `tests/snapshots/<test path>.snap`; `vun test --accept` updates them.
- **Doctests**: code blocks in doc comments compile and run in the interpreter.
- **Rejection tests**: files in `tests/reject/` annotated with expected diagnostics (spec 10,
  section 7).
- **Coverage**: `vun cover` uses MIR region counters, identical on every backend; output is lcov.
- **Mutation testing** (later): `vun mutate` mutates MIR and reruns tests in the interpreter,
  without rebuilding.
- **C dependencies** can be built with sanitizers by the build step that compiles them.
