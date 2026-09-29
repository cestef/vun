// A picture, a diagram of the cards, under its title if it has one, as large as the room allows.

import {Img, Layout, Txt, makeScene2D} from '@motion-canvas/2d';
import {sequence} from '@motion-canvas/core';

import {BOLD, FONT, SIZE} from '../../brand/type';
import {Stage} from '../../clip/Stage';
import {beat, useSegment} from '../../clip/mode';
import {lower, rise} from '../../clip/rise';

const HOLD = 3;

export default makeScene2D(function* (view) {
  const placed = useSegment('image');
  const stage = new Stage(view, placed);
  const {scheme, frame, area} = stage;
  const size = (share: number) => frame.side * share;
  const {title, src} = placed.segment;

  const heading = size(SIZE.heading);
  const room = title === undefined ? area : area.above(heading * 2);
  const picture = (<Img src={src} />) as Img;
  // As large as the room allows, once its size is known.
  const scale = () => {
    const {x: width, y: height} = picture.naturalSize();
    return room.fit({width, height}, 1);
  };
  picture.size(() => picture.naturalSize().scale(scale()));
  const parts = [
    ...(title === undefined ? [] : [<Txt fontFamily={FONT} fontSize={heading} fontWeight={BOLD} fill={scheme.ink} text={title} />]),
    picture,
  ] as Layout[];
  view.add(
    <Layout layout direction="column" alignItems="center" gap={heading} position={area.center}>
      {parts}
    </Layout>,
  );
  lower(parts);
  // Its size is known once it has loaded.
  yield picture.toPromise();

  yield* stage.enter();
  yield* sequence(0.2, ...parts.map(rise));
  yield* beat('image', HOLD);
});
