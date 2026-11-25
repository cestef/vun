import {For} from 'solid-js';

import type {Message as Told} from '../play/protocol';

// A compiler message's parts, code and types set apart in `<code>`.
export function Message(props: {message: Told}) {
  return <For each={props.message.parts}>{(part) => (part.kind === 'text' ? part.value : <code class={`part ${part.kind}`}>{part.value}</code>)}</For>;
}
