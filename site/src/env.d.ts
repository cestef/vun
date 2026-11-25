/// <reference types="astro/client" />

// The build of the compiler by its hash, and the grammar and themes the playground colors with:
// both read by the config at build time.
declare const __TOOLCHAIN__: string;
// The workspace's version, from its `Cargo.toml`.
declare const __VERSION__: string;
declare const __HIGHLIGHT__: {grammar: {name: string}; themes: {name: string}[]};

declare module '*.vun?raw' {
  const text: string;
  export default text;
}
