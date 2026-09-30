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
assets/{logo,cards}, assets/palette.toml     logo SVGs and animations; Typst cards (code, social, icon) and their PNGs
site/                                        the website: landing page and playground (Vite, pnpm; see commands.md)
README.md, CONTRIBUTING.md, LICENSES/       for people: pitch, how to try, how to contribute; DBAD or QUACK
```

## Generated directories

Never edited by hand: edit the `.vun` source, then run `cargo gen`, which also removes files no
longer generated. Generated files are held to the 300-line limit too.

| Directory | Source |
|-----------|--------|
| `compiler/lang/grammar/src/generated/` | `lib/lang/lang/syntax/grammar.vun` |
| `compiler/front/desugar/src/generated/` | `lib/lang/lang/syntax/desugar.vun` |
| `compiler/base/diag/src/errors/` | `lib/lang/lang/errors.vun` |
| `compiler/lang/primitives/src/generated/` | the `= _` declarations of `lib/lang` |
| `compiler/lang/items/src/{lang,std,targets}/` | a handle per public declaration of `lib`; the modules of a namespace's descendants in a directory `<namespace>_/` (`arch_/`), re-exported flat |

Rust names `lang` and `std` items only through `vun_items`.

## Assets

The brand's sources, cards, videos and kits: see [`assets.md`](assets.md).
