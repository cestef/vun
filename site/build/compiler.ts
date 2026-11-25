// The compiler the playground runs in the browser, built for WASI with
// `cargo build -p vun-cli --target wasm32-wasip1 --profile dist`, and the library it compiles against.

import {createHash} from 'node:crypto';

import {bytes, files, source} from './source';

const COMPILER = 'target/wasm32-wasip1/dist/vun.wasm';
const LIBRARY = 'lib';
// How many hexadecimal digits of the compiler's hash name its build.
const BUILD_DIGITS = 16;

export const compiler = () => bytes(COMPILER);

// The build by the hash of its bytes, which keys what a browser keeps of it.
export const build = () => createHash('sha256').update(compiler()).digest('hex').slice(0, BUILD_DIGITS);

// The library's files by path.
export const library = () => Object.fromEntries(files(LIBRARY).map((path) => [path, source(`${LIBRARY}/${path}`)]));
