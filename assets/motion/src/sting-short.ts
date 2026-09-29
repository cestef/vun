import {makeProject} from '@motion-canvas/core';

import audio from '../../sound/sting-short.wav?url';
import short from './scenes/clip/short?scene';

// The short sting that opens a clip, with the mascot; `scheme` names a scheme of `palette.toml`.
export default makeProject({audio, scenes: [short], variables: {scheme: 'dark', mascot: true}});
