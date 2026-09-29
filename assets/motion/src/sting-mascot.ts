import {makeProject} from '@motion-canvas/core';

import sting from './scenes/sting?scene';

// The sting with the mascot dropping in beside the word; `scheme` names a scheme of
// `palette.toml`.
export default makeProject({scenes: [sting], variables: {scheme: 'dark', mascot: true}});
