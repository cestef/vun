# 11. Testing, fuzzing, benchmarking

Built in, without a keyword and without global constructors. Research notes, section 6, has the
survey behind these choices.

## 1. Tests are bindings

A test is an ordinary binding marked `#[test]`. The attribute is `std.testing.test`, plain data: the
test runner is library code, `std.testing.runner(pkg)`, which finds every `#[test]` binding in a
package by reflection and is itself an ordinary program (spec 08). Nothing registers anything at
load time.

**The parameters select what kind of test it is:**

```ov
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
| none | unit test | `overt test` |
| values of ordinary types | property test | `overt test` (seeded cases), `overt fuzz` (coverage-guided) |
| `&[u8]` | fuzz target | same |
| `&mut testing.Bench` | benchmark | `overt bench` |
| `&mut testing.Sim` | simulation | `overt test`, `overt sim --seeds N` |

Attributes: `#[test.fuel(n)]` sets a deterministic step budget instead of a wall-clock timeout;
`#[test.ignore(.reason = "...")]`; `#[test.on(.target)]` restricts a test to target runs.

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
every `overt test`.

## 3. Execution modes

`overt test --on <mode>`:

- **`interp`** (default): the reference interpreter. It traps deterministically on every item of
  appendix C in `unsafe` code, including aliasing violations (spec 05, section 8), so every test run
  is also an undefined-behavior check.
- **`native`**: compiled for the host, for speed.
- **`qemu`**, **`probe`**: compiled for the target, run in an emulator or on a board. The test binary
  is `std.testing.runner(pkg)` wrapped in the board's runtime; it flashes once, resets between
  tests, streams results back over deferred logging (spec 12), and exits through semihosting or a
  breakpoint.

All modes emit the same JSON result stream, so editors run single tests the same way everywhere.

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
- Baselines live in `.overt/bench/`; `--fail-over 3%` fails the run on regression.

## 6. Other checks

- **Snapshots**: `testing.snapshot(v)` renders a value by reflection into
  `tests/snapshots/<test path>.snap`; `overt test --accept` updates them.
- **Doctests**: code blocks in doc comments compile and run in the interpreter.
- **Rejection tests**: files in `tests/reject/` annotated with expected diagnostics (spec 10,
  section 7).
- **Coverage**: `overt cover` uses MIR region counters, identical on every backend; output is lcov.
- **Mutation testing** (later): `overt mutate` mutates MIR and reruns tests in the interpreter,
  without rebuilding.
- **C dependencies** can be built with sanitizers by the build step that compiles them.
