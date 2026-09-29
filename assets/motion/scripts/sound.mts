// Writes the sound mark into `assets/sound`, synthesized from the tones below: `mark.wav`, the
// word landing (a low thud under a fifth that wobbles as the v wags), `pop.wav`, the mascot
// landing and cheering, and the tracks of the stings, each sound at the time its scene shows it
// (`src/brand/beats.ts`).

import {SHORT_ARRIVAL, SHORT_LAND, STING_ARRIVAL, STING_LAND} from '../src/brand/beats.ts';
import {Output} from './assets.mts';

const RATE = 48_000;
// The loudest sample, a decibel under full scale, and 16-bit samples' full scale.
const PEAK = 0.89;
const FULL = 32_767;

// A sine gliding from `from` to `to` hertz over about `glide` seconds, dying away over about
// `decay`, `gain` loud, with `harmonic` of its octave, wobbling by `wobble` of its pitch.
interface Tone {
  from: number;
  to?: number;
  glide?: number;
  decay: number;
  gain: number;
  harmonic?: number;
  wobble?: {depth: number; rate: number};
}

// A tone at a time, in seconds.
type Cue = [number, Tone];

// How soon a tone reaches its gain, and how long it sounds for its decay.
const ATTACK = 0.004;
const TAIL = 6;

class Track {
  private readonly samples: Float32Array;

  constructor(seconds: number) {
    this.samples = new Float32Array(Math.ceil(seconds * RATE));
  }

  static of(cues: Cue[]): Track {
    const end = Math.max(...cues.map(([at, tone]) => at + tone.decay * TAIL));
    const track = new Track(end);
    for (const [at, tone] of cues) {
      track.add(at, tone);
    }
    return track;
  }

  add(at: number, {from, to = from, glide = 1, decay, gain, harmonic = 0, wobble}: Tone) {
    const start = Math.round(at * RATE);
    let phase = 0;
    for (let i = 0; i < decay * TAIL * RATE && start + i < this.samples.length; i++) {
      const t = i / RATE;
      const bend = wobble === undefined ? 1 : 1 + wobble.depth * Math.exp(-t / decay) * Math.sin(2 * Math.PI * wobble.rate * t);
      phase += (2 * Math.PI * (to + (from - to) * Math.exp(-t / glide)) * bend) / RATE;
      const envelope = Math.min(1, t / ATTACK) * Math.exp(-t / decay);
      this.samples[start + i] += gain * envelope * (Math.sin(phase) + harmonic * Math.sin(2 * phase));
    }
  }

  // As a WAV file: 16-bit mono PCM, scaled to the peak.
  wav(): Buffer {
    const loudest = this.samples.reduce((most, s) => Math.max(most, Math.abs(s)), 0);
    const scale = loudest === 0 ? 0 : (PEAK / loudest) * FULL;
    const data = Buffer.alloc(this.samples.length * 2);
    this.samples.forEach((s, i) => data.writeInt16LE(Math.round(s * scale), i * 2));
    const header = Buffer.alloc(44);
    header.write('RIFF', 0, 'ascii');
    header.writeUInt32LE(36 + data.length, 4);
    header.write('WAVEfmt ', 8, 'ascii');
    header.writeUInt32LE(16, 16);
    header.writeUInt16LE(1, 20);
    header.writeUInt16LE(1, 22);
    header.writeUInt32LE(RATE, 24);
    header.writeUInt32LE(RATE * 2, 28);
    header.writeUInt16LE(2, 32);
    header.writeUInt16LE(16, 34);
    header.write('data', 36, 'ascii');
    header.writeUInt32LE(data.length, 40);
    return Buffer.concat([header, data]);
  }
}

// The word landing: a thud, and A3 with its fifth, wobbling as the v wags on its tip.
const WAG = {depth: 0.012, rate: 6};
const LAND: Cue[] = [
  [0, {from: 140, to: 55, glide: 0.04, decay: 0.12, gain: 0.9}],
  [0.01, {from: 220, decay: 0.45, gain: 0.35, harmonic: 0.25, wobble: WAG}],
  [0.01, {from: 329.63, decay: 0.4, gain: 0.25, harmonic: 0.2, wobble: WAG}],
];

// The mascot landing, a quick drop in pitch, then cheering, a rising pair.
const GLEE = 0.15;
const ARRIVE: Cue[] = [
  [0, {from: 700, to: 280, glide: 0.03, decay: 0.06, gain: 0.6}],
  [GLEE, {from: 659.25, decay: 0.12, gain: 0.25, harmonic: 0.3}],
  [GLEE + 0.07, {from: 880, decay: 0.16, gain: 0.25, harmonic: 0.3}],
];

const at = (time: number, cues: Cue[]): Cue[] => cues.map(([when, tone]) => [time + when, tone]);

const out = new Output('sound');
out.write('mark.wav', Track.of(LAND).wav());
out.write('pop.wav', Track.of(ARRIVE).wav());
out.write('sting.wav', Track.of(at(STING_LAND, LAND)).wav());
out.write('sting-mascot.wav', Track.of([...at(STING_LAND, LAND), ...at(STING_ARRIVAL, ARRIVE)]).wav());
out.write('sting-short.wav', Track.of([...at(SHORT_LAND, LAND), ...at(SHORT_ARRIVAL, ARRIVE)]).wav());
