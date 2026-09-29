// A release, from `cards/release/releases.toml`: the wordmark is traced and lands, the version
// types in under it with its date, the highlights come in one by one, the mascot drops in beside
// the word and cheers. The project variable `version` picks the release, the newest by default.

import {Layout, Txt, makeScene2D} from '@motion-canvas/2d';
import {all, delay, easeOutCubic, sequence, useScene, waitFor} from '@motion-canvas/core';
import {parse} from 'smol-toml';

import releases from '../../../../cards/release/releases.toml?raw';
import {Wordmark} from '../../brand/Wordmark';
import {LOGO} from '../../brand/logo';
import {entry} from '../../brand/palette';
import {FONT} from '../../brand/type';
import {Stage} from '../../clip/Stage';

interface Release {
  version: string;
  date: string;
  highlights: string[];
}

const RELEASES = (parse(releases) as unknown as {releases: Release[]}).releases;

// The share of the frame the word takes and where it sits above the middle; sizes of the version,
// the date and the highlights, and where the version stands, shares of the shorter side.
const SHARE = 0.3;
const WORD = 0.27;
const SIZE = {version: 0.1, date: 0.03, highlight: 0.032, gap: 0.025};
const VERSION = -0.12;
// How much faster than the sting the word is traced; seconds per character typed, between
// highlights, at the end.
const PACE = 0.5;
const TYPING = 0.08;
const BETWEEN = 0.25;
const HOLD = 2;

export default makeScene2D(function* (view) {
  const {scheme, frame, echo} = new Stage(view);
  const byVersion = Object.fromEntries(RELEASES.map((r) => [r.version, r]));
  const release = entry(byVersion, useScene().variables.get('version', RELEASES[0].version)(), 'release');
  const size = (share: number) => frame.side * share;

  const word = new Wordmark(scheme, frame.area.fit(LOGO.view, SHARE), {echo});
  word.root.y(-size(WORD));
  word.erase();
  const version = (<Txt fontFamily={FONT} fontWeight={700} fontSize={size(SIZE.version)} fill={scheme.ink} text="" />) as Txt;
  const date = (<Txt fontFamily={FONT} fontSize={size(SIZE.date)} fill={scheme.muted} text="" />) as Txt;
  const highlights = release.highlights.map(
    (line) =>
      (
        <Txt fontFamily={FONT} fontSize={size(SIZE.highlight)} fill={scheme.text} opacity={0}>
          <Txt fill={scheme.accent} text="+ " />
          {line}
        </Txt>
      ) as Txt,
  );
  view.add([
    word.root,
    <Layout layout direction="column" alignItems="center" gap={size(SIZE.gap)} offset={[0, -1]} y={size(VERSION)}>
      {version}
      {date}
      <Layout layout direction="column" alignItems="start" gap={size(SIZE.gap) / 2} marginTop={size(SIZE.gap)}>
        {highlights}
      </Layout>
    </Layout>,
  ]);

  yield* word.draw(PACE);
  yield* all(
    word.land(),
    delay(0.2, version.text(release.version, release.version.length * TYPING)),
    delay(0.5, date.text(release.date, 0.4)),
    delay(0.9, sequence(BETWEEN, ...highlights.map((line) => line.opacity(1, 0.4, easeOutCubic)))),
    delay(0.9 + BETWEEN * highlights.length, word.welcome()),
  );
  yield* waitFor(HOLD);
});
