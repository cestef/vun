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

Attributes: `#[test.gen(g)]` on a parameter of a property generates its argument with `g`,
`(c: &mut testing.Choices) -> T`, rather than from its type; `#[test.fuel(n)]` sets a deterministic step budget instead of a wall-clock timeout,
where a mode counts steps (`interp`); `#[test.ignore("why")]` keeps a test from running, the
runner reporting it with its reason (`std.testing.Ignored`, read with `std.meta.attr`);
`#[test.on(.target)]` restricts a test to target runs.

`assert` takes `Code(bool)` (spec 03), so a failure prints the asserted expression and its span.

## 2. Inputs from types

A test with parameters is a **property**: the runner calls it with arguments generated from its
parameters' types (`lang.meta.params`), over `std.testing.property.cases` cases (100), each on a
thread of its own. `testing.gen(T, c)` makes a value of `T`: by `T`'s `testing.Arbitrary` where
it has one, and otherwise, for a struct, an enum, a tuple or a record, by reflection, a variant,
then each of its parts (`lang.meta.make`). The `Arbitrary` impls of `std` cover integers, which
reduce a choice into their range as `wrap` does (an `Int(0..=100)` is never 101), floats, whose
encoding is one choice (so the least is `0.0`), and arrays, item by item. A parameter whose type
borrows is lent what its case holds (`testing.property.lend.Lent`): `&T` holds a `T`, and a type with
no value of its own stores one its way (`testing.property.lend.Store`), a slice `[T]` as a prefix of
up to 32 generated items, so `&[u8]` is lent 0 to 32 bytes. Every value is read from a **choice stream**
(`testing.Choices`: each choice a `u64` at most a bound, drawn from a seeded splitmix64 generator
and recorded), so:

- the first case that traps is **shrunk** on its choices, whatever generated them: shorter
  streams, then each choice at 0 or moved halfway towards it, as long as the case still traps.
  The least choice is the first variant and the integer congruent to 0, so the case shrinks
  towards those; `(x: u8) { assert(x < 200); }` reports `x = 200`;
- the search runs its cases quietly (`std.os.thread.reporting`); the smallest failing case runs
  once more and reports its trap, and the runner writes its arguments, one line each, `name = `
  and the value as JSON (`std.fmt.json.write`, derived by reflection where a type has no
  `Json`), before the test's line;
- a test's cases are drawn from a seed, the FNV-1a hash of its name, so every run draws the same
  cases; the line reports the seed and the case. `vun test --seed n` draws every property's
  cases from `n` instead, replaying a reported failure, and `--cases n` runs `n` cases of each
  (`std.testing.Options`, read from the runner's arguments; decision 363).

A fuzz target (`&[u8]`) runs under `vun test` as the property it is. Not yet: the same test run
under a coverage-guided fuzzer, sharing one corpus with the seeded runs.

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
program's table says it (`std.trace.Site.of`), or for a property
`{"falsified":{"trapped":...,"seed":n,"case":n,"shrinks":n}}`, then the totals,
`{"totals":{"passed":n,"trapped":n,"falsified":n,"ignored":n}}`. The lines are `std.testing.Line` written by
`std.fmt.json.derive`, read by the driver as `vun_driver::testing::Line`. `vun test` reads the
program's standard output and error as one stream, so what a test writes, the backtrace of its
trap included, comes before its line and is reported with it. Unit tests, properties and fuzz
targets run; benchmarks and simulations do not yet.

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
