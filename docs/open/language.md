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
