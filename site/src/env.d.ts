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

declare module 'virtual:examples' {
  const examples: {name: string; code: string}[];
  export default examples;
}

declare module '*.vun?raw' {
  const text: string;
  export default text;
}
