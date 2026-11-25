// Puts the lesson a page names in its place: the page leaves its material as JSON in the element.

import {render} from '@solidjs/web';

import {Lesson, type Material} from './Lesson';

export function mountLesson(root: HTMLElement) {
  const lesson = JSON.parse(root.dataset.lesson ?? '') as Material;
  render(() => <Lesson lesson={lesson} />, root);
}
