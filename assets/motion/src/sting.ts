import {makeProject} from '@motion-canvas/core';

import audio from '../../sound/sting.wav?url';
import sting from './scenes/sting?scene';

// `scheme` names a scheme of `palette.toml`.
export default makeProject({audio, scenes: [sting], variables: {scheme: 'dark'}});
