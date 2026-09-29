import {makeProject} from '@motion-canvas/core';

import sting from './scenes/sting?scene';

// `scheme` names a scheme of `palette.toml`.
export default makeProject({scenes: [sting], variables: {scheme: 'dark'}});
