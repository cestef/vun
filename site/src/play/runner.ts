// Builds and runs the editor's code in a worker and says what happened, as it happens and as an
// `Outcome` once it ends. A run in progress is ended by `stop`, which ends its worker.

import * as cache from './cache';
import type {Outcome} from './lessons';
import type {Asked, Reported, Stage, Told, Toolchain} from './protocol';

// What a run tells, as it comes.
export interface Events {
  record(record: Reported): void;
  output(text: string, stage: Stage): void;
  started(stage: Stage): void;
  ended(stage: Stage, code: number): void;
  crashed(stage: Stage, why: string): void;
}

export class Runner {
  private worker: Worker | undefined;
  private stopped: (() => void) | undefined;

  constructor(private readonly toolchain: Toolchain) {}

  get running(): boolean {
    return this.stopped !== undefined;
  }

  // Ends the run in progress; its `run` settles with what it had.
  stop() {
    this.worker?.terminate();
    this.worker = undefined;
    this.stopped?.();
  }

  run(code: string, events: Events): Promise<Outcome> {
    const {toolchain} = this;
    const outcome: Outcome = {built: false, diagnostics: [], output: '', exit: undefined, trap: undefined, notes: ''};
    if (this.worker === undefined) {
      this.worker = new Worker(new URL('./build.ts', import.meta.url), {type: 'module'});
      this.worker.postMessage({kind: 'setup', toolchain} satisfies Asked);
    }
    const worker = this.worker;
    return new Promise((done) => {
      const settle = () => {
        this.stopped = undefined;
        done(outcome);
      };
      this.stopped = settle;
      worker.onmessage = ({data}: MessageEvent<Told>) => {
        if (data.kind === 'saved') {
          // A worker started after this one stops begins from them too.
          toolchain.saved = data.files;
          void cache.keep(toolchain.id, data.files);
        } else if (data.kind === 'record') {
          events.record(data.record);
          if (data.record.type === 'diagnostic') outcome.diagnostics.push(data.record);
        } else if (data.kind === 'output') {
          events.output(data.text, data.stage);
          if (data.stage === 'run') outcome.output += data.text;
          else outcome.notes += data.text;
        } else if (data.kind === 'started') {
          events.started(data.stage);
        } else if (data.kind === 'ended') {
          events.ended(data.stage, data.code);
          if (data.stage === 'build') outcome.built = data.code === 0;
          else outcome.exit = data.code;
          if (data.stage === 'run' || data.code !== 0) settle();
        } else {
          outcome.trap = data.why;
          events.crashed(data.stage, data.why);
          settle();
        }
      };
      worker.postMessage({kind: 'run', code} satisfies Asked);
    });
  }
}
