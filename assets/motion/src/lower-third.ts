import {makeProject} from '@motion-canvas/core';

import lower from './scenes/clip/lower?scene';
import {EPISODE} from './episodes/tour/episode';

// The speaker's name over footage, rendered with no background for it to show through.
export default makeProject({scenes: [lower], variables: {scheme: 'dark', episode: EPISODE}});
