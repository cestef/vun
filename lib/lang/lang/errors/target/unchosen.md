# target.unchosen

No target could be chosen, so nothing that depends on one (the width of `usize`, layouts, code)
can be computed.

A target is a `lang.target.Spec` value of the package `targets` (spec 13, section 7). `--target`
names one by its path in that package, such as `bare.cortex_m4f`. Without it, the row of
`targets.hosts` for the machine running the toolchain is used, the machine named as the toolchain
names it (the architecture and system Rust's `std::env::consts` report). The diagnostic's cause
says which failed: the path leads to no value, the value is not a literal `Spec`, or no row of
`targets.hosts` names the machine.

## Example

```console
$ vun check --target linux.x86_64_musl main.vun
error[target.unchosen]: no target is chosen
  caused by: it has no value: a name leads nowhere
```

## Fix

Name a target the package declares (`lib/targets/targets/<system>.vun`), or add a row for the machine to
`targets.hosts`.

```console
$ vun check --target linux.x86_64 main.vun
```
