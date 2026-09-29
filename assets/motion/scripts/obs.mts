// Writes `assets/stream/obs/vun.json`, an OBS scene collection of the overlays of
// `assets/stream` (Scene Collection, Import): a scene per moment of a stream, each overlay a
// browser source over the whole canvas. The sources name the overlays by absolute `file:` URLs,
// so the collection is written per checkout and ignored by version control. `pnpm obs [scheme]`
// paints them in a scheme of `palette.toml`, dark unless given.

import {createHash} from 'node:crypto';

import {ASSETS, Output, SCHEMES} from './assets.mts';

const CANVAS = {x: 1920, y: 1080};
// What OBS gives a browser source by default, so pages without a background let the scene show.
const TRANSPARENT = 'body { background-color: rgba(0, 0, 0, 0); margin: 0px auto; overflow: hidden; }';
// OBS's alignment of an item to the top left of its position.
const TOP_LEFT = 5;

interface Overlay {
  page: string;
  params?: Record<string, string>;
}

interface Scene {
  name: string;
  // Back to front.
  overlays: Overlay[];
}

// The camera goes into `Live` by hand, under the frame: a capture device is named per machine.
const SCENES: Scene[] = [
  {name: 'Starting', overlays: [{page: 'starting'}]},
  {name: 'Live', overlays: [{page: 'webcam'}, {page: 'ticker'}, {page: 'lower'}]},
  {name: 'Be right back', overlays: [{page: 'brb'}]},
  {name: 'Ending', overlays: [{page: 'ending'}]},
];

const scheme = process.argv[2] ?? 'dark';
if (!(scheme in SCHEMES)) {
  throw new Error(`no scheme ${scheme}; there are ${Object.keys(SCHEMES).join(', ')}`);
}

// A stable identity for `name`, from its content, so importing again finds the same sources.
function uuid(name: string): string {
  const hex = createHash('sha256').update(`vun-obs/${name}`).digest('hex');
  return [hex.slice(0, 8), hex.slice(8, 12), `5${hex.slice(13, 16)}`, `8${hex.slice(17, 20)}`, hex.slice(20, 32)].join('-');
}

const source = (id: string, name: string, settings: object) => ({
  id,
  versioned_id: id,
  name,
  uuid: uuid(name),
  settings,
  enabled: true,
  flags: 0,
  volume: 1,
  mixers: 0,
});

// Each overlay once, whichever scenes show it.
const pages = [...new Set(SCENES.flatMap((scene) => scene.overlays.map((overlay) => overlay.page)))];
const title = (page: string) => `Vun ${page}`;
const browsers = pages.map((page) => {
  const url = new URL(`stream/${page}.html`, ASSETS);
  url.searchParams.set('scheme', scheme);
  const overlay = SCENES.flatMap((scene) => scene.overlays).find((o) => o.page === page);
  for (const [key, value] of Object.entries(overlay?.params ?? {})) {
    url.searchParams.set(key, value);
  }
  return source('browser_source', title(page), {
    url: url.href,
    is_local_file: false,
    width: CANVAS.x,
    height: CANVAS.y,
    css: TRANSPARENT,
    // Showing a scene again plays its overlays' animations from the start.
    restart_when_active: true,
    shutdown: false,
    reroute_audio: false,
  });
});

const scenes = SCENES.map(({name, overlays}) =>
  source('scene', name, {
    custom_size: false,
    id_counter: overlays.length,
    items: overlays.map(({page}, i) => ({
      name: title(page),
      source_uuid: uuid(title(page)),
      id: i + 1,
      visible: true,
      locked: false,
      pos: {x: 0, y: 0},
      scale: {x: 1, y: 1},
      rot: 0,
      align: TOP_LEFT,
      bounds_type: 0,
      bounds: {x: 0, y: 0},
    })),
  }),
);

const collection = {
  name: 'Vun',
  current_scene: SCENES[0].name,
  current_program_scene: SCENES[0].name,
  scene_order: SCENES.map(({name}) => ({name})),
  sources: [...browsers, ...scenes],
  groups: [],
  transitions: [],
  current_transition: 'Fade',
  transition_duration: 300,
  quick_transitions: [],
  saved_projectors: [],
  resolution: CANVAS,
};

new Output('stream/obs').write('vun.json', `${JSON.stringify(collection, null, 2)}\n`);
