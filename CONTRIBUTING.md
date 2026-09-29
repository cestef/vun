# Contributing

Vun is early and its design moves fast; open an issue before a large change.

## Before you start

- [`docs/spec/00-principles.md`](docs/spec/00-principles.md): the rules every change follows.
- [`docs/dev/workflow.md`](docs/dev/workflow.md): how work is done and where notes go.
- [`docs/dev/commands.md`](docs/dev/commands.md): building, running and inspecting.
- [`docs/dev/crates.md`](docs/dev/crates.md): where the code for each stage lives.

A change to the language updates the spec and [`decisions.md`](docs/spec/decisions.md).

## Checks

```sh
cargo fmt && cargo clippy --all-targets && cargo nextest run && cargo gen --check && cargo xtask lines && cargo xtask layers
```

Tests run with [nextest](https://nexte.st). Generated files are never edited by hand: change the
`.vun` source and run `cargo gen` ([`docs/dev/repository.md`](docs/dev/repository.md)).

## Commits

[Conventional commits](https://www.conventionalcommits.org) with a short one-line subject:
`feat(sema): ...`, `fix(native): ...`.

## License

Unless you state otherwise, any contribution you submit is dual-licensed as in the
[README](README.md#license), without additional terms or conditions.
