import {makeProject} from '@motion-canvas/core';

import mascot from './scenes/mascot?scene';

// `scheme` names a scheme of `palette.toml`.
export default makeProject({scenes: [mascot], variables: {scheme: 'dark'}});
