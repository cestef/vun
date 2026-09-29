import {makeProject} from '@motion-canvas/core';

import audio from '../../sound/sting-mascot.wav?url';
import sting from './scenes/sting?scene';

// The sting with the mascot dropping in beside the word; `scheme` names a scheme of
// `palette.toml`.
export default makeProject({audio, scenes: [sting], variables: {scheme: 'dark', mascot: true}});
