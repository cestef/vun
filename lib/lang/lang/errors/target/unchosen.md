# target.unchosen

No target could be chosen, so nothing that depends on one (the width of `usize`, layouts, code)
can be computed.

A target is a `lang.target.Spec` value of the package `targets` (spec 13, section 7). `--target`
names one by its path in that package, `<os>.<arch>.<abi>`, such as `bare.thumbv7em.eabihf`
(`vun target list` lists them). Without it, the first of `targets.hosts` whose `arch` and `os`
are the running machine's is used. The diagnostic's cause says which failed: the path leads to no
value, the value is not a literal `Spec`, or no target of `targets.hosts` is for the machine.

## Example

```console
$ vun check --target linux.x86_64.musl main.vun
error[target.unchosen]: no target is chosen
  caused by: it has no value: a name leads nowhere
```

## Fix

Name a target the package declares (`lib/targets/targets/<system>.vun`), or add a row for the machine to
`targets.hosts`.

```console
$ vun check --target linux.x86_64.kernel main.vun
```
