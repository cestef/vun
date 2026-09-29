# Repository

```
lib/lang/lang/syntax/{grammar,desugar}.vun   tokens, nodes, parse tables; appendix B as tables
lib/lang/lang/errors.vun + errors/           every diagnostic id and its explanation page
lib/lang, lib/std, lib/targets               `lang` primitives, `std`, one `lang.target.Spec` per target
corpus/{accept,reject,planned,support}       programs; `//~ id` annotations; waiting for APIs; stand-ins
compiler/<group>/<crate>                     see crates.md; layering in spec 13 section 9
fuzz/                                        cargo-fuzz targets (a workspace member)
whitepaper/                                  Typst paper; it quotes real numbers and `vun` output
editors/                                     editor grammars, generated from `grammar.vun` by `cargo gen`
docs/                                        spec, decisions, these notes, research, references
assets/                                      wordmark, README code card (`code/card.typ`, `code/colors.toml`)
README.md, CONTRIBUTING.md, LICENSE-*        for people: pitch, how to try, how to contribute; MIT OR Apache-2.0
```

`site/` is the owner's and untracked.

## Generated directories

Never edited by hand: edit the `.vun` source, then run `cargo gen`, which also removes files no
longer generated. Generated files are held to the 300-line limit too.

| Directory | Source |
|-----------|--------|
| `compiler/lang/grammar/src/generated/` | `lib/lang/lang/syntax/grammar.vun` |
| `compiler/front/desugar/src/generated/` | `lib/lang/lang/syntax/desugar.vun` |
| `compiler/base/diag/src/errors/` | `lib/lang/lang/errors.vun` |
| `compiler/lang/primitives/src/generated/` | the `= _` declarations of `lib/lang` |
| `compiler/lang/items/src/{lang,std,targets}/` | a handle per public declaration of `lib` |

Rust names `lang` and `std` items only through `vun_items`.

## The README code card

`assets/code-{light,dark}.png` render `corpus/accept/programs/tour.vun` (so the corpus keeps it
compiling) with `editors/sublime/vun.sublime-syntax` (generated from the grammar), in the colors of `assets/code/colors.toml`
and JetBrains Mono. Regenerate after changing any of them with `cargo xtask typeset`, which also
compiles the whitepaper; the documents it compiles are the table `typeset::ALL` of `xtask`.
