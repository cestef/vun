// What every scene of a clip stands on: the scheme's background, the frame, the echo behind in
// the same place from scene to scene, so the fades between them keep it still; under a segment of
// a clip, its footer.

import type {View2D} from '@motion-canvas/2d';
import {type ThreadGenerator, fadeTransition} from '@motion-canvas/core';

import {Mark} from '../brand/Mark';
import {type Scheme, useScheme} from '../brand/palette';
import {Footer} from './Footer';
import type {Placed} from './episode';
import {type Area, Frame} from './frame';
import {Mode, useMode} from './mode';

// How long a scene fades in from the one before.
const FADE = 0.5;

export class Stage {
  readonly scheme: Scheme;
  readonly frame: Frame;
  readonly echo: Mark;
  // Where the scene lays out: the frame within its margins, above the footer when there is one.
  readonly area: Area;
  // Whether a scene plays before it, to fade from.
  private readonly after: boolean;

  constructor(view: View2D, placed?: Placed) {
    this.scheme = useScheme();
    this.frame = new Frame(view.size());
    this.echo = Mark.echo(this.scheme, this.frame);
    view.fill(this.scheme.background);
    view.add(this.echo.root);
    // A clip opens with the short sting; a talk with its first segment.
    this.after = placed === undefined || placed.index > 0 || useMode() === Mode.Clip;
    const footed = placed !== undefined && useMode() === Mode.Clip;
    this.area = footed ? this.frame.area.above(Footer.height(this.frame)) : this.frame.area;
    if (footed) {
      view.add(new Footer(this.scheme, this.frame, placed).root);
    }
  }

  // Fades in from the scene before, if one is.
  *enter(): ThreadGenerator {
    if (this.after) {
      yield* fadeTransition(FADE);
    }
  }
}
