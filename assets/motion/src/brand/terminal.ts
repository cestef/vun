// vun's styles (`Style` of `vun-term`) drawn in the code colors of the cards: each in the role
// nearest its terminal color in `style.rs`, bold where that is.

import type {Style} from '../clip/episode';
import type {CodeScheme} from './code';

interface Look {
  color: keyof CodeScheme;
  bold?: boolean;
}

export const STYLES: Record<Style, Look> = {
  heading: {color: 'string', bold: true},
  literal: {color: 'literal', bold: true},
  placeholder: {color: 'literal'},
  good: {color: 'string', bold: true},
  error: {color: 'keyword', bold: true},
  warning: {color: 'literal', bold: true},
  note: {color: 'string', bold: true},
  help: {color: 'literal', bold: true},
  accent: {color: 'literal', bold: true},
  scope: {color: 'literal'},
  key: {color: 'literal'},
  id: {color: 'keyword', bold: true},
  dim: {color: 'comment'},
  strong: {color: 'text', bold: true},
};
