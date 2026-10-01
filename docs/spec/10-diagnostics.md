# 10. Diagnostics, provenance, lints and conventions

Diagnostics are a product surface, specified like the language: precise, pedantic, and every fix
applicable by a machine when it is safe to do so.

## 1. Provenance

Guarantee 5: every entity in every stage carries an origin chain to the source that caused it.

```rust
pub struct Span   { pub file: FileId, pub lo: u32, pub hi: u32 }
pub struct Origin { pub span: Span, pub via: Via, pub parent: Option<OriginId>, pub anchor: Anchor }
pub enum Anchor {
    File,                                             // tokens and syntax, before declarations exist
    Declaration { within: Option<OriginId>, name: Symbol }, // a declaration, by its name
    Within(OriginId),                                 // inside a declaration, relative to its start
}
pub enum Via {
    Source,                                   // written by the user
    Desugar(DesugarRule),                     // an appendix B rewrite, e.g. `for` or `?`
    Instantiate { generic: OriginId },        // a $-parameterized body instantiated here
    Splice { quote: OriginId },               // code from a #{ } or a template, spliced here
    ComputedName { producer: OriginId },      // a $(...) in a name position
    Lower(PassId),                            // a MIR or LIR pass: defer, coroutines, ABI, ...
}
```

- Tokens and CST nodes are the source: they hold byte ranges into their file directly. Every entity
  derived from them (HIR, MIR statements, LIR instructions, backend output, link-plan entries)
  holds an `OriginId`, created from those ranges. Origins are interned in a table; chains are
  shared, so the cost is one id per entity.
- **Stable identity.** A file is keyed by its package and its path inside the package, a typed
  key (`FileRef`), a loose file by its path. A package in the store, which never changes, is named
  by its content hash, so a fork of a release is a package of its own; a package on disk, edited
  in place, by the name, identity and version its manifest declares, which its edits keep
  (decision 247). An origin is identified by how it came to
  exist and its anchor: a declaration by its name inside the declaration around it and how many
  declared before it there share that name (two locals of one name in sibling blocks are two
  declarations), anything inside a declaration by its span relative to the declaration's start. `FileId` and `OriginId`
  hash by these identities, computed once, so a hash of anything holding them (query interfaces,
  persisted results) is the same in every process of one toolchain build (a fixed-seed hash,
  as query fingerprints are) and survives edits elsewhere in the file, as
  rustc's `DefPathHash` and item-relative spans do. Origins are equal by identity too: an origin
  recorded again, as when read from a saved query, is the same origin. Each entity has its own:
  a further entity lowered from the same syntax is derived from the first (`Via::Again(n)`), and
  the HIR verifier checks that no two nodes of an arena share one. Diagnostics are sorted by the
  source position of their primary label.
- **Positions now.** A result reused after an edit elsewhere in its file, as a saved query is,
  holds origins recorded in the earlier text. A compilation shows positions through its view of
  the files it reads (`vun_origin::Resolve`): an origin whose file it now reads in another text is
  found at the same place relative to its declaration as that text declares it, and a
  declaration where that text declares it. The process's tables hold every text read, since
  compilations in one process may read two texts of one file at once; the view, not the table,
  says which is current (decision 255).
- A generated construct points at the code that generated it, and that code at its caller, up to
  user code. Diagnostics show this as a trace; go-to-definition follows it.
- The C backend emits `#line` directives from origins; native backends emit DWARF line tables and
  an entry per function, naming it and the line declaring it, from them; runtime trap site ids index into a table of origins (spec 07).
- Every IR dump (`vun debug dump mir`, `vun debug dump lir`) prints origins, so any line of IR can be traced
  to source.
- Losing an origin is a compiler bug; IR verifiers check that every entity has one.

## 2. The diagnostic model

```rust
pub struct Diagnostic {
    pub id: DiagId,              // a binding path, e.g. lang.errors.int.does_not_fit
    pub severity: Severity,      // Error | Warning | Info | Hint, resolved from the lint level
    pub source: DiagSource,      // Compiler | Lint { group } | Package { name, version }
    pub message: Msg,            // structured parts: Text, Code, Type(TypeId), Name, Num
    pub primary: Option<Label>,  // None only without a source location (missing file)
    pub labels: Vec<Label>,      // secondary, any file
    pub children: Vec<Child>,    // Note | Help | Cause, each with optional labels and fixes
    pub fixes: Vec<Fix>,
    pub trace: Vec<TraceFrame>,  // "while evaluating", "instantiated from", "generated by"
}
pub struct Label { pub origin: OriginId, pub msg: Option<Msg> }
pub struct Fix   { pub msg: Msg, pub safety: Safety, pub edits: Vec<Edit>, pub preferred: bool }
pub struct Edit  { pub span: Span, pub text: String, pub holes: Vec<Hole> }
pub enum Safety  { Safe, Unsafe, Placeholder, DisplayOnly }
```

- **Causes.** A failure that comes from outside the compiler (a file system, a network, an
  archive) keeps its error chain: each error and its sources become `Cause` children, outermost
  first, rendered as `caused by:`. Every error type of the toolchain names its source, so no
  reason is flattened into text.
- **Fix safety.** `Safe`: preserves meaning and compiles; the only kind applied automatically.
  `Unsafe`: may change behavior; applied only with `--unsafe`. `Placeholder`: contains holes the
  user must fill; offered as an editor snippet. `DisplayOnly`: shown, never applied.
- **No cascades.** Sema produces `Poisoned(ErrorGuaranteed)` types and values; the token can only be
  obtained by emitting an error. Any diagnostic that mentions a poisoned entity is dropped.
- **Identity.** Every diagnostic id is a binding path to a `lang.diag.Diag` value, so ids nest and
  are namespaced like everything else: `lang.errors.int.does_not_fit`, `std.lints.perf.padding`,
  `mydrivers.lints.raw_mmio`. Compiler errors are shown relative to `lang.errors`
  (`int.does_not_fit`). A renamed id keeps its old path as an alias.
- **Explanations.** Every id has a page, the file its binding path names on disk as it names a
  namespace's source (spec 01, section 8), with the extension `md`: `lang.errors.names.duplicate`
  is `lang/errors/names/duplicate.md` in the `lang` package, beside `lang/errors.vun`, and
  `mydrivers.lints.raw_mmio` is `mydrivers/lints/raw_mmio.md` in its package. A `Diag` holds no
  path (`duplicate := diag.Diag{};`), so the page cannot drift from the id (decision 272). A page
  has an `Example` section, whose annotated file reports exactly its `//~` annotations, and a
  `Fix` section, whose version of that file reports nothing; blocks may name
  their file (```` ```vun build.vun ````), so an example can be a package. Both are checked with the
  corpus; examples that only a command shows are `console` transcripts. `vun explain <id>` prints
  the page offline, suggesting the closest id for a misspelled one; the LSP links to the page.
- **Messages are written inline**, in English, with structured arguments. Translation is left
  possible but not built (research notes, section 5: rustc removed its translation files in 2026).

### Writing a diagnostic

- *The message says what is wrong, in the source's terms*: the name, the type, the value or the
  form as written, never a stage or a structure of the compiler (MIR, LIR, lowering, an
  instantiation, a witness). What the toolchain does not do yet is said as such, naming the form
  (`` `asm` blocks with outputs are not supported yet ``), with a way around it when there is one.
- *Its shape*: lowercase, no final period, one clause; names and code as `code` parts, types as
  `ty` parts, numbers as `num` parts, never formatted into text.
- *The primary label* marks the exact span, and says what is there when the message does not
  (`` this is `u16` ``); *secondary labels* show the other places the mistake involves (`first
  declared here`, `the borrow is used later here`).
- *Every error and warning says what to do*: a help, in the imperative, naming the change (`` add
  `;` ``, `` borrow it with `&` ``); a name that resolves nowhere offers the closest one
  (`vun-suggest`). When the change is determined, a fix carries it, the fix's message being the
  help, and its safety stated honestly:
  `Safe` when the program keeps its meaning and the fix leaves no error of its own, `Unsafe`
  when it may change behavior, `Placeholder` with holes to fill, `DisplayOnly` otherwise. The
  fix `vun fix` applies is the preferred one.
- *A note* states the rule broken, or why it holds, in a sentence.
- *One mistake, one error*: what follows from an error already reported is not reported again.

### Rendering

`vun-diag` owns the model; renderers are adapters behind `DiagSink` (spec 13):

- **terminal**: an adapter over `annotate-snippets`, which renders labels, notes, and fixes as
  diffs, with ASCII or Unicode decoration and hyperlinked ids. It is rustc's renderer since 2025;
- `--diagnostics=ariadne`: an adapter over `ariadne`, for its style of one color per label role.
  It has no patch view, so each fix is a help line naming where its edits apply. The model depends
  on neither renderer;
- `--diagnostics=short`: `file:line:col: severity[id]: message`;
- `--format=json` (the output format of every command, spec 12, section 7): one diagnostic per
  line, `"type": "diagnostic"`, with a `schema` version; ids (relative and full), severity,
  messages as text and as typed parts (`text`, `code`, `type`, `number`), every label, child, fix
  (edits with their holes) and trace frame with its location resolved to file, start and end
  line and column, and bytes;
- `--diagnostics=sarif` (still to come): for CI and code-scanning services;
- the LSP, section 6.

Terminal renderers style what each part is, never how it looks: a severity and a child kind name
their style (`Severity::style`, `ChildKind::style`), and one palette (`vun_term::Style`, cargo's)
maps styles to colors for diagnostics, help, status lines, tables and logs alike. Colors follow
`--color` (spec 12, section 7).

```
error[int.does_not_fit]: value does not fit the result type
 --> src/geom.vun:2:5
  |
1 | norm1 := (p: Point) -> u32 {
  |                        --- result type is u32 = Int(0..2^32)
2 |     p.x.abs() + p.y.abs()
  |     ^^^^^^^^^^^^^^^^^^^^^ this is Int(0..=2^32)
  |
  = note: i32.abs() is Int(0..=2^31), so the sum reaches 2^32
help: widen the result type [safe]
  |
1 | norm1 := (p: Point) -> u64 {
  |                        ~~~
help: or narrow, choosing what happens at the limit [placeholder]
  |
2 |     (p.x.abs() + p.y.abs()).${1|to,sat,wrap|}(u32)
  |
  = explain: vun explain lang.errors.int.does_not_fit
```

Types are shown as their values: an integer type by its range, whose bounds of more than 17 bits
are written by the power of two they lie within 16 of (`2^64 - 1`), and a range ending just below
a power of two as the half-open range to it, so `u64` reads `Int(0..2^64)`.

Errors in generated code show the chain:

```
error[types.mismatch]: expected `&[u8]`, found `u32`
 --> src/board.vun:14:9                 (generated)
   = generated by: svd.Peripheral at src/board.vun:3:10
   = from template: svd/peripheral.vun:22:13
   = for register `CR1` described at boards/stm32f4.svd:1180:7
```

## 3. Library diagnostics

Compile-time code reports diagnostics with the same model, through `std.diag` over `lang.diag`:

```vun
format := ($spec: Code(&[u8])) {
    $if .err(bad) = fmt.parse($spec) {
        $std.diag.error(.{
            .at = .some(spec.within(bad.at)),               // the bytes inside the literal
            .msg = bad.reason.text(),
            .fixes = &[.{ .msg = "write `{}`", .safety = .safe, .text = "{}" }],
        });
    }
};
```

- `std.diag.error` stops evaluation with a real diagnostic (`comptime.reported`, over
  `lang.diag.error($message, $at)`); `warn` and `note` do not. A report without `.at` is shown at
  the call that led to it from outside the reporting code, the place it was reported labeled; a
  `$e` is run while typing wherever it surely runs and all it reads is known, and a `$if` or
  `$for` not decided there runs none of its `$e`. `.fixes` are carried, not shown yet.
- A report outside any `$` form runs only if the program runs it: it is no error where it is
  written, and a program built that reaches it is refused, the report shown at the call that led
  to it from outside the reporting code as above, with the calls between as frames. A function no
  build reaches reports nothing. A library's report is shown before any other failure to build.
- Places come from code: a `Code(T)` parameter knows where its argument is written
  (`c.span()`), and `c.within(bytes)` where the bytes `bytes` of the string literal it writes are,
  escapes mapped to the source that writes them (`lang.meta.within`, an origin derived from the
  literal's). A value carries no place: two equal strings are one value. A report placed in the
  reporting function's own code moves to the call that led to it; one placed in the caller's code,
  as inside a literal it passed, stays there.
- `std.fmt.parse(format)` reads a format string once: its pieces, or its first problem
  (`fmt.Error`, a `fmt.Problem` and the bytes it covers); `print` reports that problem.
- `#[deprecated(.{ .note = "...", .use = new_fn })]` generates a `Safe` fix that rewrites calls,
  when `new_fn` is a drop-in replacement.
- A library can attach help to failed interface constraints on its own types
  (`#[diag.unsatisfied(Writer, "wrap the port in uart.Writer")]`).

## 4. Lints

A lint is a compile-time value; configuring a lint names that value, so a typo in a lint name is a
compile error.

```vun
// mydrivers/lints/raw_mmio.vun
#[lint.define]
raw_mmio := lang.lint.Lint{
    .group = .suspicious,
    .default = .some(.warn),
    .doc = std.meta.embed("raw_mmio.md"),
    .call = (cx: &mut lang.lint.Cx, call: lang.lint.Call) {
        if call.calls(lang.mem.volatile_store) and !cx.inside(std.hw) {
            cx.report(call.origin, "raw volatile store outside a register abstraction", &[]);
        }
    },
};
```

- A lint states a check for each kind of node it looks at, `.decl` (`lang.lint.Decl`: a
  declaration's name, what it binds, where it is written, whether it is public and documented),
  `.local` (`lang.lint.Local`: a local binding or parameter of a function literal, whether it is
  a parameter, whether any name reads it, whether a pattern testing a value binds it, and whether
  it hides a constant from outside the file: a name of the prelude, the package or a dependency
  whose value is data, not a function, a type or a namespace) and `.call` (`lang.lint.Call`: the value of what it
  calls, where it is written); a check left at its default (`lang.lint.unchecked`) is not run. The
  compiler walks each file once and runs every check on every node of its kind. A function type's
  parameters are no locals, and the binders of an or-pattern's alternatives after the first are
  the first's (or names nothing can read), so each local is one node.
- Every lint report notes the level it reports at and proposes, as a placeholder fix, the
  attribute allowing it on the declaration around (`#[lang.lint.allow(lint, "why it is allowed")]`),
  whose reason its author writes.
- Lints run in the compile-time evaluator: deterministic, sandboxed, cached by the query engine.
  A check reports by appending to its `Cx`, a plain `lang` struct, so running it is a pure call
  (`lang.lint.reports`) whose value is what it reported; a report's id is the lint's path
  (`std.lints.naming.case`), and a fix it proposes replaces what the report points at.
- They see a read-only typed view of the code (`lang.lint.Cx` and the node structs), versioned as
  a `lang` module. MIR is never exposed, so compiler internals can change.
- A declarative form covers pattern lints, written as a quote with `_` holes, never as a string:
  `lint.pattern(#{ lang.mem.volatile_store(_, _) }, ...)`.
- A file is checked with the lints its package, the packages it depends on and `std` define.

### Levels and groups

A group is a namespace of `std.lints` (`std.lints.pedantic`), which configuring names and which
declares its default `level`; a library's group is a namespace of its own.

Levels: `allow`, `warn`, `deny`, `forbid` (cannot be lowered below), `expect` (warns if the lint does
not fire). Every override takes a reason.

| Group | Default |
|-------|---------|
| `correctness` | deny |
| `naming` | deny |
| `layout` | warn |
| `suspicious` | warn |
| `style` | warn |
| `perf` | warn |
| `pedantic` | allow |
| `restriction` | allow |
| `nursery` | allow |

Configuration, nearest scope wins:

```vun
// build.vun
b.lints(.{
    .{ std.lints.pedantic, .warn },
    .{ std.lints.perf.large_copy, .warn, .{ .bytes = 256 } },     // lints declare typed options
    .{ mydrivers.lints.raw_mmio, .deny },
});

// in source: on a declaration, a file (//! position) or a directory's own file
#[lint.allow(std.lints.naming.case, "mirrors the vendor register names")]
```

- The command line overrides the build; the build overrides package defaults; `forbid` is final.
- Dependencies' lints are capped at `warn` when building a dependent.

## 5. Conventions

Conventions are lints in the `naming` and `layout` groups: enabled by default, configurable per
project, extensible by libraries with the mechanism of section 4.

### Naming (`std.lints.naming.*`)

Case is decided by how a binding is written, never by evaluating it: a binding is a type when it
is annotated `type`, `lang.Interface` or a function type giving one, or when its value is a
`struct`, an `enum`, a function type, an `interface` or a function literal giving a type. A name
or a call that happens to give a type is a value, so an alias is annotated to bind one:
`Index: type = Int(0..9);`, `Error: type = Full;`, and `()` or `(u8, u16)` are values unless
annotated.

| Binding | Case | Examples |
|---------|------|----------|
| a type, an interface, or a function giving one | `PascalCase` | `RingBuffer`, `Writer`, `Pair` |
| a namespace: a struct of declarations alone, none a method receiving the struct, or a function giving one (a witness, a derive) | `snake_case` | `symbols`, `int_narrow`, `derive_eq` |
| everything else: values, functions, fields, variants, modules, files, directories, packages | `snake_case` | `ring`, `write_all`, `.some`, `drivers/uart.vun` |

- Primitive and prelude type names keep their conventional lowercase (`usize`, `f64`, `bool`,
  `never`, `type`, `interface`), and so does an integer type named by its width, a letter of
  `std.lints.naming.width_prefixes` then digits (`u8`, `i32`, `u24`): these are the exemptions of
  `naming.case`, `std.lints.naming.lowercase_types` among them, and extending them is a lint
  configuration like any other.
- A struct of declarations alone with a method whose first parameter is the struct itself (by
  value, borrow or pointer) is a type with no data, not a namespace: `Dma := struct { start :=
  (self: &mut Dma) {..} }`.
- A field holding a type binds a type. A leading `_`, which marks a private name, is not part of
  the case.
- Acronyms are words: `Uart`, `HttpClient`, `usb_hid`.
- No type-name suffixes that restate the namespace: `mem.alloc.Arena`, not `mem.alloc.ArenaAllocator`
  (`naming.stutter`).
- Foreign names are never exempt; `#[link.symbol("HAL_UART_Init")] hal_uart_init := ...` keeps the
  binding compliant.
- Case carries no meaning (unlike Go) and is not ignored (unlike Nim).

### Organization (`std.lints.layout.*`)

- `layout.file_per_type`: a file whose single public declaration is a type is named after it.
- `layout.reexport_depth`: a re-export chain is at most one hop.
- `layout.cycles`: no import cycles between directories of one package.
- `layout.depth`: warns when a namespace nests deeper than a configured limit, or when a directory
  holds a single file (flatten it).
- `layout.pub_doc` (restriction): public declarations have doc comments.
- `layout.private_leak`: a public signature mentions a private type.

### Style (`std.lints.style.*`)

- `style.unused`: a local binding no name reads is reported, with a fix binding `_` in its place,
  applied on request only (`vun fix --unsafe`), since a discarded value ends where it is made.
  Parameters are not reported: they belong to the function's signature, which callers name and
  interfaces fix.

### Formatting

`vun fmt` has no configuration. Output is stable within an edition.

## 6. Tools

**`vun fix [path]`** applies the fixes checking each file the path names proposes (spec 12,
section 7), to the files of its package
(`vun_driver::fix`):
- `Safe` fixes by default, `--unsafe` for those that may change behavior as well; per diagnostic,
  its preferred fix of that safety, or else its first;
- takes no edit overlapping one taken before in the round, checks again, and repeats to a fixed
  point, at most four rounds;
- gives a file whose fixes add errors its text of before the round, and stops;
- then reports what is left, as `vun check` does.

Planned: `--id` or `--group` to select; refusing a dirty working copy (jj or git) unless
`--allow-dirty`; `vun fix --edition` migrating to a new edition.

**LSP mapping.** Primary label to `range`; id to `code`; explain page to `codeDescription.href`;
source to `source`; secondary labels and children to `relatedInformation`; unused and deprecated to
`tags`; fixes to quick-fix code actions (`isPreferred` for the preferred fix, placeholders as snippet
edits); a "suppress with reason" action inserts `#[lint.allow(...)]`.

## 7. Testing diagnostics

- **Must-fail corpus.** Files annotated `//~ int.does_not_fit` (this line) or `//~^ ...` (the line
  above). The harness checks the exact set of (line, id) pairs, so a cascade fails the test. An
  annotation may pin the column (`//~ id @12`) and require fragments of the rendered message,
  notes and help (`//~ id "escape it"`). An id must be declared in `lang.errors`; a file waiting
  only on stages that do not exist yet is reported as pending.
- **Planned corpus.** Programs written ahead of the libraries they use (`corpus/planned`) must
  parse, and may report only unresolved names, which the harness lists as what each waits for; a
  planned file that checks clean fails until it moves to the accepted corpus.
- **Snapshots** of the terminal rendering at fixed width without color, and of the JSON.
- **Fix tests.** Every fix of the rejected corpus, `Unsafe` ones included, applied to a copy of
  its file: the fixed file has no more errors than before, and fixing it again changes nothing.
- **Explain pages** are compiled.
- **Cascade budget.** Fuzzing injects syntax errors and fails if one error produces more than a set
  number of diagnostics.
