import {For, Show, createEffect} from 'solid-js';
import type {JSX} from '@solidjs/web';

import {Icon} from './Icon';
import {type Combo, type Group, bindingOf, caps} from './keys';
import type {Keys} from './keys';
import type {IconName} from '../icons';

export function Keycaps(props: {combo: Combo}) {
  return (
    <span class="caps">
      <For each={caps(props.combo)}>{(cap) => <kbd>{cap}</kbd>}</For>
    </span>
  );
}

// A button that does `action`, with the keys that do it on it and in its tooltip; a link when it
// has an `href`, which goes where it points by itself.
export function Action(props: {
  keys: Keys;
  action: string;
  class?: JSX.HTMLAttributes<HTMLElement>['class'];
  icon?: IconName;
  caps?: boolean;
  href?: string;
  disabled?: boolean;
  children?: JSX.Element;
}) {
  const binding = () => bindingOf(props.action);
  const title = () => {
    const found = binding();
    return found === undefined ? undefined : `${found.say} (${caps(found.combo).join(' ')})`;
  };
  const inside = (
    <>
      <Show when={props.icon}>{(name) => <Icon name={name()} />}</Show>
      {props.children}
      <Show when={props.caps && binding()}>{(found) => <Keycaps combo={found().combo} />}</Show>
    </>
  );
  return (
    <Show
      when={props.href === undefined}
      fallback={
        <a class={props.class} href={props.href} title={title()} target="_blank" rel="noopener">
          {inside}
        </a>
      }
    >
      <button type="button" class={props.class} title={title()} disabled={props.disabled} onClick={() => props.keys.fire(props.action)}>
        {inside}
      </button>
    </Show>
  );
}

const GROUPS: Group[] = ['Editor', 'Panel', 'Course', 'Page'];

// The cheat sheet, written from the table.
export function KeySheet(props: {keys: Keys}) {
  let dialog!: HTMLDialogElement;
  createEffect(
    () => props.keys.open,
    (open) => {
      if (open && !dialog.open) dialog.showModal();
      if (!open && dialog.open) dialog.close();
    },
  );
  return (
    <dialog class="keys" ref={dialog} onClose={() => (props.keys.open = false)} onClick={(event) => event.target === dialog && (props.keys.open = false)}>
      <h2>Keyboard</h2>
      <For each={GROUPS.filter((group) => props.keys.listed.some((binding) => binding.group === group))}>
        {(group) => (
          <section>
            <h3>{group}</h3>
            <For each={props.keys.listed.filter((binding) => binding.group === group)}>
              {(binding) => (
                <p class="binding">
                  <span>{binding.say}</span>
                  <Keycaps combo={binding.combo} />
                </p>
              )}
            </For>
          </section>
        )}
      </For>
      <p class="note">Outside the editor, ? opens this list.</p>
    </dialog>
  );
}
