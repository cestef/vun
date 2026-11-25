import {compiler} from '../../../build/compiler';

export const GET = () => new Response(new Uint8Array(compiler()), {headers: {'Content-Type': 'application/wasm'}});
