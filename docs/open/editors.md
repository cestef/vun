# Open: editors and the server

Editors: Zed, Helix, VS Code, Neovim. The owner's current focus.

- Polish: hover on VS Code commands; per-editor details (Zed has no code lenses).
- The owner looks at VSCodium and installs Zed's dev extension (`editors/zed`). Whether Zed lets the
  last pattern win is unverified.
- A doc comment on a record's field reads as a doc of an assignment in tree-sitter (`editors.vun`
  uses `//` inside records).
- `FromLiteral` could replace the hand walks of `Literal` in manifests, the lock file and the
  toolchain file.
- Server: views, hovers, inlays, run and test lenses and the run input file (`vun.input`) are
  built and tested, but untried in a real editor. Views narrow to the function or declaration at
  the cursor, the others show the file.
- Inline assembly: `for` over a witness made per use (`Env::witness`, `Typer::iteration`) is not
  checked for conformance; an instruction value whose table holds types (`[0]type`) has no run-time
  layout.
