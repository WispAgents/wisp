// Selection mode's words in each platform's own (kit strings `select.*`, design/mobile.md §A.4): Android's "n selected"
// and Select all; iOS's "n Selected" (Select Items with none) and Select All / Deselect All. SelectionBar (Android, the
// browser) and TopAppBar's iOS edit mode share them. Pure.
import type { Translate } from '../core/i18n.tsx';

export interface SelectionWords {
  /** the bar's title */
  readonly title: string;
  /** Select all, or its undo when every row is picked */
  readonly all: string;
}

export function selectionWords(t: Translate, ios: boolean, count: number, allPicked: boolean): SelectionWords {
  const title = ios ? (count ? t('select.count-ios', { count }) : t('select.none-ios')) : t('select.count', { count });
  const all = t(allPicked ? (ios ? 'select.deselect-ios' : 'select.deselect') : ios ? 'select.all-ios' : 'select.all');
  return { title, all };
}
