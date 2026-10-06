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
package around the current directory when there is none, which a first word naming nothing on
disk is (`vun test crc_vector` runs that test); a file outside any package is in no namespace
(`test.no_namespace`).

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
    clock := testing.Clock.init();
    mut nodes := [node(&clock, 0), node(&clock, 1), node(&clock, 2)];
    _ = s.run(&clock, .{ &mut nodes[0], &mut nodes[1], &mut nodes[2] }, .seconds(60));
    assert(leaders(&nodes) == 1);
};
```

| Parameters | Kind | Run by |
|------------|------|--------|
| none | unit test | `vun test` |
| values of ordinary types | property test | `vun test` (seeded cases), `vun fuzz` (coverage-guided) |
| `&[u8]` | fuzz target | same |
| `&mut testing.Bench` | benchmark | `vun bench` |
| `&mut testing.Sim` | simulation | `vun test` (seeded cases, as a property's), `vun fuzz` |

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
reduce a choice among as many as their range holds (`lang.int.count`) into it as `wrap` does
(an `Int(0..=100)` is never 101), floats, whose
encoding is one choice (so the least is `0.0`), and arrays, item by item. A parameter whose type
borrows is lent what its case holds (`testing.property.lend.Lent`): `&T` holds a `T`, and a type with
no value of its own stores one its way (`testing.property.lend.Store`), a slice `[T]` as a prefix of
up to 32 generated items, so `&[u8]` is lent 0 to 32 bytes. Every value is read from a **choice stream**
(`testing.Choices`: each choice a `u64` at most a bound, drawn from a seeded splitmix64 generator
and recorded), so:

- the first case that traps is **shrunk** on its choices, whatever generated them: shorter
  streams, then each choice at 0 or moved halfway towards it, as long as the case still traps.
  The least choice is the first variant, and an integer is drawn as its distance from the one of
  its range nearest 0, then, where both sides hold that distance, a side, above first; so the case
  shrinks towards the first variants and the integers of least magnitude, keeping their signs:
  `(x: u8) { assert(x < 200); }` reports `x = 200`, `(n: i8) { assert(n >= -100); }` `n = -101`;
- the search runs its cases quietly (`std.os.thread.reporting`); the smallest failing case runs
  once more and reports its trap, and the runner writes its arguments before the test's line,
  one line each, `{"argument":{"name":...,"value":...}}` (`std.testing.Argued`), the value as
  JSON (`std.fmt.json.write`, derived by reflection where a type has no `Json`); `vun test`
  shows them as `x = 200`, and the language server names them in the trap it shows;
- a test's cases are drawn from a seed, the FNV-1a hash of its name, so every run draws the same
  cases; the line reports the seed and the case. `vun test --seed n` draws every property's
  cases from `n` instead, replaying a reported failure, and `--cases n` runs `n` cases of each
  (`std.testing.Options`, read from the runner's arguments; decision 363).

A fuzz target (`&[u8]`) runs under `vun test` as the property it is. `vun fuzz [path] [names]`
fuzzes the properties alone, fuzz targets and simulations included, guided by coverage: it builds
the tests' program covered (section 6) and runs each over `--cases` cases (10000 unless given,
the runner's `--fuzz`, `std.testing.fuzz`). A case replays the choices of a stream kept, one
choice mutated (a value drawn, 0, the next value, its half, or the stream cut there), then draws;
or draws a new stream. It is kept, up to 64 streams, when it set a flag of `lang.cover.map` no
case before set, so the streams reach ever more of the code; a failing case is shrunk and
reported as a property's, and one passing reports how many cases ran, streams it kept and flags
they set (`fuzzed`). Three nested tests of a byte each, which random cases pass once in 2^24, fall
in about 8000 cases.

Not yet: corpus and crash inputs in `fuzz/<test path>/`, committed and replayed as regressions on
every `vun test`, and comparisons traced to guide mutations to the values code tests for.

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
one line per test run, `{"test":{"name":...,"outcome":...,"nanos":n}}`, its outcome `"passed"`,
`{"ignored":{"reason":...}}` or
`{"trapped":{"site":n,"said":{"message":...,"at":{"file":...,"line":...,"column":...}}}}` as the
program's table says it (`std.trace.Site.of`), or for a property
`{"falsified":{"trapped":...,"seed":n,"case":n,"shrinks":n}}`, then the totals,
`{"totals":{"passed":n,"trapped":n,"falsified":n,"ignored":n,"nanos":n}}`. `nanos` is how long the test,
or the whole run, took by the monotonic clock (`std.time.Stopwatch`), `null` where the system gives the
program none; `vun test` shows it beside each name and after the totals. The lines are `std.testing.Line` written by
`std.fmt.json.derive`, read by the driver as `vun_driver::testing::Line`. `vun test` reads the
program's standard output and error as one stream, so what a test writes, the backtrace of its
trap included, comes before its line and is reported with it. Unit tests, properties, fuzz
targets, benchmarks and simulations run.

**Parallel runs.** `vun test` runs the tests as nextest does: the program lists them (`--list`: one
`{"listed":{"name":...,"kind":...}}` line each, `std.testing.Listed`), then each runs in a process
of its own, `--jobs n` (`-j`, `VUN_TEST_JOBS`) at a time, the machine's cores if not given, and is
reported as its process ends, in the order they end; the totals are the sum, the time the run's.
A test that crashes its process, or never ends, costs that test alone, and a result missing from a
process that ended is reported as a failure with its status. `--jobs 1` runs them in one process in
the order they are declared, as the program does without arguments. Benchmarks and fuzzing run in
one process, in order: they measure and guide.

## 4. Deterministic simulation

Time, network, storage and randomness reach code only through capabilities (spec 07), and executors
are library code (spec 06). A test whose one parameter is `&mut testing.Sim` is a simulation: its
tasks, `std.task.Async` frames, run on a `testing.Clock` through `s.run(&clock, tasks, limit)`,
`tasks` a tuple of their borrows.

- The clock is virtual: a task sleeps on it (`yield ..clock.sleep(span)`), and it moves only when
  no task is ready, to the earliest time one sleeps until, waking every task (one resumed early
  finds its event not fired and waits again).
- One task runs at a time, the next chosen among those ready by the case's choices: the choices a
  property's arguments are read from (section 2). A simulation therefore runs over a property's
  cases, a failing schedule is shrunk on its choices to the fewest and smallest, and the seed and
  case a failure reports replay it exactly (`--seed`).
- `run` ends `.done` when every task finished, `.stalled` when those left wait on events nothing
  will fire, `.timed_out` when the clock has moved `limit`. The tasks stay for the test to end.

This is TigerBeetle's and FoundationDB's approach, with no runtime support. Not yet: a simulated
network and disk that drop, reorder, delay and corrupt on command, and choices drawn by the tasks
themselves.

## 5. Benchmarks

- `b.loop()` runs setup once, prevents the loop body from being optimized away, and hides the
  iteration count from the code (the semantics Go adopted in 1.24). Measured, it runs the body in
  batches, each as many times as all before it, reading the clock between batches only, until the
  loop has run `std.testing.bench.least` (500 ms); `b.keep(&x)` reads `x` as volatile memory, so
  what makes it stays.
- `vun test` runs a benchmark as a test: its loop runs the body once. `vun bench [path] [names]`
  builds the same program and runs the benchmarks alone, measured (the runner's `--bench`): each
  reports `measured` with how many times its body ran and how long that took. `vun bench` takes
  `--samples n` (10) of that, each a run of the program, and reports per benchmark the median time
  per iteration, the spread (the median absolute deviation as a part of the median), the number of
  samples and of outliers, which lie more than 1.5 interquartile ranges past the quartiles and are
  left out of the mean and range (decision 600).
- `--measure time` (default): wall-clock samples. `--measure instructions`: the instructions the
  process retires in user mode, counted with `perf stat`, per iteration; where that is not possible
  the run is timed instead and reports why (decision 601). Not yet: the interpreter's count of LIR
  operations, which is deterministic, and on Cortex-M the DWT cycle counter.
- Baselines live in `.vun/bench/<name>.json` of the package (decision 602): `--save` keeps the
  statistics of a run as the baseline `--baseline <name>` names (`base`), and a run shows the
  change of each median against it. A change is a regression when it is more than `--fail-over 3%`
  and more than the noise, the larger spread of the two runs; with `--fail-over` given, a
  regression fails the run.

## 6. Other checks

- **Snapshots**: `testing.snapshot(v)` renders a value by reflection into
  `tests/snapshots/<test path>.snap`; `vun test --accept` updates them.
- **Doctests**: code blocks in doc comments compile and run in the interpreter.
- **Rejection tests**: files in `tests/reject/` annotated with expected diagnostics (spec 10,
  section 7).
- **Coverage**: `--cover` builds a program that marks each block it runs: MIR sets the block's flag
  in `lang.cover.map` (`lang.cover.size` flags, a block's number the hash of its function's
  instantiation and its position) as the block starts, identical on every backend; naked
  functions hold their assembly alone and are not marked. `vun fuzz` reads the flags. Not yet:
  `vun cover`, counting per region and writing lcov.
- **Mutation testing** (later): `vun mutate` mutates MIR and reruns tests in the interpreter,
  without rebuilding.
- **C dependencies** can be built with sanitizers by the build step that compiles them.
