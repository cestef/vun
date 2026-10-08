# Open: language and compiler front

- `std.Dyn` (518 to 520, 646): no type requirement (`Error: type = _`) and no `Interface.Member`
  such as `Encoder.Error`, which need the types an interface leaves open to be parameters of the
  `Dyn`; a table is held by value, since a constant of function pointers cannot be borrowed as
  `&static` (`function_cast` is not known when compiling); an owning `OwnedDyn` does not drop its
  target.
- `vun-sema` still holds one dependency cycle (about 8.5k lines): `env`, `member`, `callable`,
  `eval`, `decl`, `layout`, `target`, `witness` and `apply` call each other through `Env`, whose
  methods are spread over them. Splitting it further means moving an owned interface under `Env`
  for each part that leaves, as `Typist` did for typing.
- Guaranteed tail calls (spec 09, section 3) lack: a large record argument or a struct or wide
  result under the C backend with GCC, which refuses `musttail` there (`TailCalls` could say which
  compilers jump with one); the assisting backend compiles such a function. The x86-64 passing of
  records by address (Windows) is covered by unit tests only, no Windows target runs here.
- Silent limits where a diagnostic belongs: a tuple index past `u32` lowers to `Name::Hole`
  (`front/hir/src/lower/mod.rs`, which has no diagnostics yet), a compile-time integer past `i128`
  has no `Literal` (`sema/ty/src/literal.rs`), and the declaration number of an origin is a `u16`
  (`Anchor::Declaration`).
