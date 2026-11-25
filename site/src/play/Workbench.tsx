// The workbench both pages are made of: the editor, the panel under it with the problems and what
// the program wrote, the run button and the status line.

import './workbench.css';
import './play.css';

import type {JSX} from '@solidjs/web';
import {onSettled} from 'solid-js';

import {Icon} from '../ui/Icon';
import {Action, KeySheet} from '../ui/Keycaps';
import type {Bench} from './bench';
import {Dock} from './Dock';

export function Workbench(props: {bench: Bench; tools?: JSX.Element; class?: string}) {
  const {bench} = props;
  let host!: HTMLElement;
  onSettled(() => bench.mount(host));
  return (
    <div class={['workbench', props.class]}>
      <div class="tools">
        <button type="button" class="tool" aria-pressed={String(bench.helix.value) as 'true'} title="Helix keys on or off" onClick={() => bench.toggleHelix()}>
          <Icon name="keyboard" />
          Helix keys
        </button>
        {props.tools}
        <Action keys={bench.keys} action="run" class={['run', {running: bench.running.value}]} icon={bench.running.value ? 'stop' : 'play'} caps disabled={!bench.ready.value}>
          {bench.running.value ? 'Stop' : 'Run'}
        </Action>
      </div>
      <section class="pane source" ref={host}></section>
      <Dock bench={bench} />
      <footer class="status">
        <span class="dot" data-state={bench.state.value}></span>
        <span>{bench.status.value}</span>
        <span class="space"></span>
        <Action keys={bench.keys} action="keys" class="hint" icon="keyboard" caps>
          Shortcuts
        </Action>
      </footer>
      <KeySheet keys={bench.keys} />
    </div>
  );
}
