# backend.unchosen

None of the backends the target prefers for the profile can build.

A target lists the backends it builds with under each profile, most preferred first
(`lang.target.Backends`: `debug` for `vun build`, `release` for `vun build --release`), and a
build takes the first that this toolchain has, that generates code, and that generates it for
the target's architecture (spec 13, section 8). This diagnostic says why each was passed over:
missing from the toolchain (its Cargo feature left out), registered but not generating code yet,
or not generating code for the architecture.

## Example

```console
$ vun build --target wasi.wasm32 main.vun
error[backend.unchosen]: no backend the target prefers for debug builds can build
  = note: `llvm` cannot generate code yet
```

## Fix

Name a backend that can with `--backend`, build for another target, or add a backend that can
to the target's preferences in `lib/targets`.

```console
$ vun build --target wasi.wasm32 --backend c main.vun
```
