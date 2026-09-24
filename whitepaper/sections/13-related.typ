#import "../template.typ": *

= Related work <sec-related>

@tab-comparison places Vun among the languages it draws on most, along the dimensions discussed in
this paper. The comparison concerns the default, safe subset of each language.

#figure(
  table(
    columns: (auto, 1fr, 1fr, 1fr, 1fr),
    align: (left, left, left, left, left),
    toprule,
    [], [C], [Rust], [Zig], [Vun],
    midrule,
    [integer overflow], [undefined or modular], [profile-dependent], [mode-dependent], [impossible; narrowing explicit],
    [resource release], [manual], [destructors], [`defer`, manual], [linear values, `defer`],
    [borrow regions], [none], [lifetime annotations], [none], [one scoping rule],
    [generics], [none], [traits], [compile-time evaluation], [compile-time evaluation],
    [metaprogramming], [preprocessor], [token macros], [compile-time evaluation], [typed quotation],
    [asynchronous frames], [none], [pinned futures], [not in the language], [fixed kind, sized frames],
    [entry point], [known name], [known name, attributes], [root inspection], [exports, typed link symbols],
    [thread safety], [none], [`Send`, `Sync`], [none], [synchronized capabilities],
    bottomrule,
  ),
  caption: [Comparison with related systems languages.],
) <tab-comparison>

*Compile-time evaluation.* Zig demonstrated that a single language can serve for generic
programming and code generation through compile-time evaluation @zig-0-16, and its experience
informs the design of Vun's evaluator @zig-4055. Zig builds types by reifying descriptors, which
cannot carry declarations, so names cannot be computed; Vun instead generates declarations with
computed names inside ordinary type literals. Jai extends compile-time execution to arbitrary input
and output, at the cost of caching and reproducibility. C++26 adds static reflection with
declarations as data @p2996 and has deferred token-based injection @p3294, whose authors chose
untyped token sequences because typed fragments are difficult to check out of context. Typed
multi-stage programming in MetaOCaml @metaocaml and Scala 3 @scala-macros is the model for Vun's
quotations, with the difference that in Vun a function body with compile-time parameters is already
a typed template, so quotation is needed only for code passed as an argument. Hygiene by scope sets
@sets-of-scopes @lean-notations is designed for user-extensible syntax, which Vun excludes.

*Memory.* Rust established that ownership and borrowing can provide memory safety without a
collector. Vun keeps exclusivity and replaces lifetime annotations with a scoping rule, in the
spirit of the parameter conventions of Hylo @hylo, whose `let`, `inout`, `sink` and `set`
correspond to Vun's `&`, `&mut`, passing by value and `&out`; Vun's consuming borrow `&take` has no
Hylo counterpart. Austral makes linear types the primary discipline for resources @austral, as Vun
does for every linear value; Vun adds the static resolution of `defer` against place states.

*Integers.* Range-typed integers appear in Ada and Pascal as subtypes with run-time checks on
assignment. Vun differs in making arithmetic exact by widening result types, so checks occur only at
explicit narrowing, and in narrowing ranges by comparisons of immutable bindings.

*Effects and suspension.* Capability-passing compilation of effect handlers @effekt and lexically
scoped handlers @lexa show how far effects can be made zero-cost; Vun adopts their capability view
and declines general handlers, which require stack switching. Rust's asynchronous functions
motivated the specified frame layout @rust-async-size.

*Programs and linking.* Rust's externally implementable items @rust-eii turn language items into
declarations defined downstream; Vun's typed link symbols generalize them to every hook between a
program and its runtime. Roc platforms @roc-platform and WASI worlds @wasi-cli are the precedent for
hosts that state a typed requirement.

*Tooling.* Go's minimal version selection @go-mvs, checksum files and analysis-driven fixes
@go-fix, Zig's content-addressed packages @unboxed-zig, defmt's deferred formatting @defmt, rustc's
and Ruff's fix applicability @rustc-diag @ruff-fixes, and TigerBeetle's deterministic simulation
@tigerbeetle-fuzz are adopted with small changes. The query engine follows the verifying traces of
Mokhov et al. @build-systems as implemented in Salsa, whose memory growth when adopted by
rust-analyzer @salsa-memory motivates the bounded cache for function bodies.
