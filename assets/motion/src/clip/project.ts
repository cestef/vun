// Projects of an episode: a clip, the short sting then its segments fading into each other, or a
// talk, its segments as slides. Each segment is played by the scene of its kind, copied under a
// name of its own; the scene finds its segment by that name (`useSegment`).

import type {FullSceneDescription} from '@motion-canvas/core';
import {makeProject} from '@motion-canvas/core';

import sting from '../../../sound/sting-short.wav?url';
import callout from '../scenes/clip/callout?scene';
import chapter from '../scenes/clip/chapter?scene';
import code from '../scenes/clip/code?scene';
import diff from '../scenes/clip/diff?scene';
import end from '../scenes/clip/end?scene';
import image from '../scenes/clip/image?scene';
import points from '../scenes/clip/points?scene';
import short from '../scenes/clip/short?scene';
import split from '../scenes/clip/split?scene';
import stats from '../scenes/clip/stats?scene';
import terminal from '../scenes/clip/terminal?scene';
import title from '../scenes/clip/title?scene';
import type {Episode, Kind, Placed} from './episode';
import {Mode} from './mode';

const SCENES: Record<Kind, FullSceneDescription> = {
  title,
  chapter,
  points,
  code,
  diff,
  terminal,
  split,
  callout,
  stats,
  image,
  end,
};

// The scenes playing `episode`'s segments, and each scene's segment by its name.
function sequence(episode: Episode) {
  const placed: Record<string, Placed> = {};
  const scenes = episode.segments.map((segment, index) => {
    const scene = {...SCENES[segment.kind], name: `${index + 1} ${segment.kind}`};
    placed[scene.name] = {segment, index, count: episode.segments.length};
    return scene;
  });
  return {scenes, placed};
}

export function clip(episode: Episode) {
  const {scenes, placed} = sequence(episode);
  return makeProject({
    scenes: [short, ...scenes],
    audio: sting,
    variables: {scheme: 'dark', mascot: true, mode: Mode.Clip, episode, placed},
  });
}

export function talk(episode: Episode) {
  const {scenes, placed} = sequence(episode);
  return makeProject({scenes, variables: {scheme: 'dark', mascot: true, mode: Mode.Talk, episode, placed}});
}
