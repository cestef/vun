import {makeProject} from '@motion-canvas/core';

import release from './scenes/release/release?scene';

// A release of `cards/release/releases.toml`, named by `version`; `scheme` names a scheme of
// `palette.toml`.
export default makeProject({scenes: [release], variables: {scheme: 'dark'}});
