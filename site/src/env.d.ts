/// <reference types="vite/client" />

declare module 'virtual:brand.css';

declare module 'virtual:highlight' {
  const highlight: {grammar: {name: string}; themes: {name: string}[]};
  export default highlight;
}

declare module 'virtual:toolchain' {
  const id: string;
  export default id;
}

declare module 'virtual:lessons' {
  import type {Lesson} from './play/lessons';
  const lessons: Lesson[];
  export default lessons;
}

declare module '*.vun?raw' {
  const text: string;
  export default text;
}
