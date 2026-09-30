# backend.unavailable

The backend named cannot generate code yet.

Backends are components registered with the toolchain, each declaring its facts once in a
`BackendInfo`, among them whether it generates code (spec 13, sections 7 and 8). Some are
registered before they generate anything (Cranelift, QBE, LLVM, external backends), so that
targets can already name them among their preferences; a build that names one with `--backend`
reports this diagnostic instead of emitting anything. A build that names none skips them and
takes the next backend its target prefers.

## Example

```console
$ vun build --backend cranelift main.vun
error[backend.unavailable]: backend `cranelift` cannot generate code yet
```

## Fix

Name a backend that generates code for the target, or none: the target's preferences for the
profile then choose. `vun toolchain show` lists the backends of this toolchain and which generate
code; `vun target show` says which one a build for the target takes, and why.

```console
$ vun build main.vun
```
