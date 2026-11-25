import {For} from 'solid-js';

import {type IconName, PATHS} from '../icons';

export function Icon(props: {name: IconName}) {
  return (
    <svg viewBox="0 0 24 24" class={['icon', `icon-${props.name}`]} aria-hidden="true">
      <For each={PATHS[props.name]}>{(d) => <path d={d} />}</For>
    </svg>
  );
}
