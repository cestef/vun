# Overt documentation

> Working name. Source extension `.ov`, CLI `overt`.

Draft 2 of the design. Each file in `spec/` covers one area and can be read on its own;
`spec/00-principles.md` is the entry point and everything else refers back to it.

| File | Contents |
|------|----------|
| [spec/00-principles.md](spec/00-principles.md) | thesis, layering rule, guarantees, the seven concepts |
| [spec/01-syntax.md](spec/01-syntax.md) | tokens, the 15 keywords, symbols, bindings, functions, visibility, modules |
| [spec/02-types.md](spec/02-types.md) | types as values, `$`, composite types, interfaces, operators, attributes |
| [spec/03-metaprogramming.md](spec/03-metaprogramming.md) | templates, code values, names and hygiene, reflection, inception, comptime semantics |
| [spec/04-integers.md](spec/04-integers.md) | range integers, floats, SIMD vectors |
| [spec/05-memory.md](spec/05-memory.md) | kinds, place states, borrows, `defer`, pointers, layout, aliasing facts |
| [spec/06-coroutines.md](spec/06-coroutines.md) | suspension: iterators, generators, async |
| [spec/07-capabilities.md](spec/07-capabilities.md) | capabilities, interior mutability, traps, function types, FFI |
| [spec/08-programs.md](spec/08-programs.md) | what a program is: exports, entry symbols, runtimes, typed link symbols |
| [spec/09-performance.md](spec/09-performance.md) | runtime performance guarantees and controls |
| [spec/10-diagnostics.md](spec/10-diagnostics.md) | diagnostics, provenance, lints, conventions, fixes |
| [spec/11-testing.md](spec/11-testing.md) | tests, properties, fuzzing, benchmarks, simulation |
| [spec/12-ecosystem.md](spec/12-ecosystem.md) | packages, manifest, lockfile, layout, CLI, toolchain, embedded, C interop |
| [spec/13-compiler.md](spec/13-compiler.md) | implementation architecture |
| [spec/14-roadmap.md](spec/14-roadmap.md) | phases and exit criteria |
| [spec/appendices.md](spec/appendices.md) | `lang` module map, desugaring table, UB list, grammar sketch |
| [spec/decisions.md](spec/decisions.md) | decision log and open questions |
| [research/2026-09-survey.md](research/2026-09-survey.md) | the survey behind draft 2, with sources |
| [archive/draft-1.md](archive/draft-1.md) | previous draft, kept for reference |
| [HANDOFF.md](HANDOFF.md) | state of the project for the next agent |

Conventions in these docs: "must" is a rule the compiler enforces; "lean" marks a tentative
choice listed in `decisions.md`; every code sample is meant to compile under this draft.
