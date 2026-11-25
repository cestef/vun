// The panel under the editor: tabs over views, one shown at a time, and a button that folds it to
// its tab bar so that the editor has the height.

import {For, Show, createEffect} from 'solid-js';

import {Icon} from '../ui/Icon';
import type {IconName} from '../icons';
import type {Bench, TabId} from './bench';
import {Problems} from './Problems';

const TABS: {id: TabId; label: string; icon: IconName}[] = [
  {id: 'problems', label: 'Problems', icon: 'alert'},
  {id: 'output', label: 'Output', icon: 'terminal'},
];

function Output(props: {bench: Bench}) {
  let view!: HTMLDivElement;
  createEffect(
    () => props.bench.output.value,
    () => void (view.scrollTop = view.scrollHeight),
  );
  return (
    <div class="report" ref={view}>
      <Show when={props.bench.output.value !== ''}>
        <pre class="program">{props.bench.output.value}</pre>
      </Show>
    </div>
  );
}

export function Dock(props: {bench: Bench}) {
  const {bench} = props;
  const problems = () => bench.entries.value.length;
  const errors = () => bench.entries.value.filter((entry) => entry.diagnostic.severity === 'error').length;
  return (
    <section class={['dock', {folded: bench.folded.value}]}>
      <div class="tabs" role="tablist">
        <For each={TABS}>
          {(tab) => (
            <button type="button" role="tab" class="tab" aria-selected={String(bench.tab.value === tab.id) as 'true'} onClick={() => bench.show(tab.id)}>
              <Icon name={tab.icon} />
              <span class="label">{tab.label}</span>
              <Show when={tab.id === 'problems' && bench.built.value && problems() > 0}>
                <span class={['badge', errors() > 0 ? 'error' : 'warning']}>{problems()}</span>
              </Show>
            </button>
          )}
        </For>
        <span class="space"></span>
        <button type="button" class="fold" aria-label="Fold the panel" aria-expanded={String(!bench.folded.value) as 'true'} onClick={() => bench.toggleFold()}>
          <Icon name="chevron-down" />
        </button>
      </div>
      <div class="views">
        <div hidden={bench.tab.value !== 'problems'}>
          <Problems bench={bench} />
        </div>
        <div hidden={bench.tab.value !== 'output'}>
          <Output bench={bench} />
        </div>
      </div>
    </section>
  );
}
