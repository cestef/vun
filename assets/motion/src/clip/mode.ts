// How a segment's scene is played: as part of a clip, holding each beat for its time with a
// caption and a progress bar under it, or as slides of a talk, holding each beat until the
// speaker moves on.

import {type ThreadGenerator, beginSlide, useScene, waitFor} from '@motion-canvas/core';

import type {Episode, Kind, Placed, Segment} from './episode';

export enum Mode {
  Clip = 'clip',
  Talk = 'talk',
}

export function useMode(): Mode {
  return useScene().variables.get('mode', Mode.Clip)();
}

export function useEpisode(): Episode {
  const episode = useScene().variables.get<Episode | undefined>('episode', undefined)();
  if (episode === undefined) {
    throw new Error('a clip scene needs the project variable `episode`');
  }
  return episode;
}

// The segment this scene plays, of kind `kind`, and where it stands.
export function useSegment<K extends Kind>(kind: K): Placed & {segment: Extract<Segment, {kind: K}>} {
  const scene = useScene();
  const placed = scene.variables.get<Record<string, Placed>>('placed', {})()[scene.name];
  if (placed === undefined || placed.segment.kind !== kind) {
    throw new Error(`the scene ${scene.name} plays no ${kind} segment; build its project with \`clip\` or \`talk\``);
  }
  return placed as Placed & {segment: Extract<Segment, {kind: K}>};
}

// A moment to hold, named `name`: `seconds` in a clip, until the next slide in a talk.
export function* beat(name: string, seconds: number): ThreadGenerator {
  if (useMode() === Mode.Talk) {
    yield* beginSlide(name);
  } else {
    yield* waitFor(seconds);
  }
}
