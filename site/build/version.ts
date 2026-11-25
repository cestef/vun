// The version the workspace's manifest declares, the one every crate and the toolchain share.

import {parse} from 'smol-toml';

import {source} from './source';

const MANIFEST = 'Cargo.toml';

export const version = () => ((parse(source(MANIFEST)).workspace as {package: {version: string}}).package.version);
