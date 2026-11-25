// The page's icons, drawn on one 24 unit grid with the same round stroke so that they sit together:
// a table of paths by name, drawn by `ui/Icon` in an island and `components/Icon` on a page.

export const PATHS = {
  arrow: ['M4.5 12h15', 'M13.5 6l6 6-6 6'],
  alert: ['M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17', 'M12 8v5', 'M12 16.2h.01'],
  book: ['M5 5.5A1.5 1.5 0 0 1 6.5 4H18v13H6.5A1.5 1.5 0 0 0 5 18.5z', 'M5 18.5A1.5 1.5 0 0 0 6.5 20H18v-3'],
  bulb: ['M9.5 18h5', 'M10.5 21h3', 'M12 3a6 6 0 0 0-3.6 10.8c.7.6 1.1 1.3 1.1 2.2h5c0-.9.4-1.6 1.1-2.2A6 6 0 0 0 12 3z'],
  check: ['M5 12.5l4.5 4.5L19 7.5'],
  'chevron-down': ['M6 9.5l6 6 6-6'],
  'chevron-left': ['M14.5 6l-6 6 6 6'],
  'chevron-right': ['M9.5 6l6 6-6 6'],
  'chevron-up': ['M6 14.5l6-6 6 6'],
  circle: ['M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0-16'],
  close: ['M6.5 6.5l11 11', 'M17.5 6.5l-11 11'],
  copy: ['M9.5 9.5h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1z', 'M15.5 9.5v-4a1 1 0 0 0-1-1h-9a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h3'],
  edit: ['M4.5 19.5l1-4L16 5a2.1 2.1 0 0 1 3 3L8.5 18.5z', 'M14 7l3 3'],
  external: ['M13.5 4.5h6v6', 'M19.5 4.5l-8 8', 'M17.5 14v4a1.5 1.5 0 0 1-1.5 1.5H6A1.5 1.5 0 0 1 4.5 18V8A1.5 1.5 0 0 1 6 6.5h4'],
  keyboard: ['M3.5 6.5h17a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-17a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1z', 'M6.5 10.5h.01', 'M10 10.5h.01', 'M14 10.5h.01', 'M17.5 10.5h.01', 'M7.5 14h9'],
  link: ['M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1', 'M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1'],
  list: ['M9 6.5h11', 'M9 12h11', 'M9 17.5h11', 'M4.5 6.5h.01', 'M4.5 12h.01', 'M4.5 17.5h.01'],
  locate: ['M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8', 'M12 3v3', 'M12 18v3', 'M3 12h3', 'M18 12h3'],
  play: ['M8 5.5v13L19 12z'],
  reset: ['M4.5 12a7.5 7.5 0 1 0 2.2-5.3', 'M4.5 4.5v4h4'],
  stop: ['M7 7h10v10H7z'],
  terminal: ['M4.5 5.5h15a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1v-11a1 1 0 0 1 1-1z', 'M7.5 10l3 2.5-3 2.5', 'M13 15h4'],
  warning: ['M12 4.5l9 15.5H3z', 'M12 10v4', 'M12 16.8h.01'],
  wand: ['M4.5 19.5L15 9', 'M15.5 3.5v3', 'M14 5h3', 'M19.5 9.5v3', 'M18 11h3'],
} satisfies Record<string, string[]>;

export type IconName = keyof typeof PATHS;
