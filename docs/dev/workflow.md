# Working on Vun

How the project is worked on, for people and agents. The rules of the code itself are in
`CLAUDE.md` at the root (on disk, ignored by the owner's global gitignore, so `jj st` never shows
it); the design rules are in [`spec/00-principles.md`](../spec/00-principles.md).

## How the owner works

- Fast iteration: requirements arrive mid-task and are integrated at once. Work continues without
  stopping, and each finished batch is committed.
- Choices that are genuinely the owner's are asked with concrete options and a recommendation;
  everything else is picked and reported.
- Design: one rule per concept, no edge cases, minimal keywords, deep namespacing, lints for
  conventions, full traceability, typed codegen. Clean, elegant, performant code.
- Code: generic and data-driven (tables and handles, never names as text), single source of
  truth, enums over flags, structured errors, methods on owning types, a verifier per
  representation, files at most 300 lines with tests in `tests.rs`, clippy pedantic clean,
  `cargo nextest run`. More crates rather than bigger ones.
- New language rules go into [`decisions/`](../spec/decisions/README.md) as proposed until the owner
  approves them.

## Version control

- `jj`, never raw git. Conventional commits with short one-line subjects, no mention of AI.
- Several sessions may share the working copy at once. Coordinate by message, claim files before
  rewriting them, and commit only your paths (`jj commit <paths>`). Large changes go in a `jj`
  workspace of their own and land by rebase; a workspace needs its own `target/`.
- Never rebase or forget another session's workspace commit; port the change instead.
- jj runs no git hooks: `cargo xtask hooks` sets the repository's jj aliases instead, `jj check`
  and `jj push`, which run the xtask tasks of their names (`jj push -c @-` checks, then runs
  `jj git push -c @-`). Tasks are declared in `compiler/tools/xtask/src/task/table.rs`, with what each needs
  first; the external programs they call are typed `Call`s.

## Writing

- No em dashes, no unicode arrows in prose.
- Specs and the whitepaper are precise and minimal; the whitepaper is academic in tone (Typst).
- The whitepaper quotes measured numbers (the state table and code-size figure of section 12, the
  pipeline figure), the proposed rules of section 14 and the diagnostic of section 11; these go
  stale, so refresh them when an area lands.
- The whitepaper's title page and the root README carry a notice on AI use (the design is the
  owner's, most code and prose written by agents under their direction); keep it when rewriting
  either.

## Documents

| Document | Holds |
|----------|-------|
| [`HANDOFF.md`](../HANDOFF.md) | what is in flight and next, under 40 lines, nothing durable |
| [`open/`](../open/) | what is open, one file per area; an item leaves when done |
| [`dev/repository.md`](repository.md) | where things are, generated directories |
| [`dev/assets.md`](assets.md) | the brand: cards, videos, sound, emoji, stickers, diagrams, stream kit |
| [`dev/commands.md`](commands.md) | checks, running, inspecting, cross builds, measuring speed |
| [`dev/crates.md`](crates.md) | the compiler's crates by area |
| [`dev/pitfalls.md`](pitfalls.md) | traps that cost a session before |
| [`spec/`](../spec/00-principles.md) | the language and the compiler's design |
| [`spec/decisions/`](../spec/decisions/README.md) | every decision and its reasoning, a file each |

A fact learned while working goes where it lasts: a rule of the language in the spec and
`decisions/`, a command or tool in `commands.md`, a trap in `pitfalls.md`. What is open goes in
the `open/` file of its area; `HANDOFF.md` keeps only the next session's start.
