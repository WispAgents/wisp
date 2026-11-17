// The kit shows no copy of its own: labels come from props. The exceptions are accessible names
// it must always have (a dialog's close button, a toast's dismiss), which go through `t`.
import { createContext, useContext, type JSX } from 'solid-js';

/** The kit's own strings, in English. A shell passes `t` to translate them. */
export const KIT_STRINGS = {
  close: 'Close',
  dismiss: 'Dismiss',
  loading: 'Loading',
  'loading.slow': 'Still waiting: the computer it needs may be away or busy',
  send: 'Send',
  resize: 'Resize',
  more: 'More',
  back: 'Back',
  clear: 'Clear',
  'clear-all': 'Clear all',
  notifications: 'Notifications',
  'show-lines': 'Show {n} more lines',
  'show-file': 'Show {n} lines',
  'edit-file': 'Edit {path}',
  'time.now': 'now',
  'time.seconds': '{n}s ago',
  'time.minutes': '{n}m',
  'time.hours': '{n}h',
  'time.days': '{n}d',
  'time.in-minutes': 'in {n} min',
  'no-results': 'No results',
  'diff.binary': 'Binary file not shown',
  // the phone apps' gestures as accessibility actions (design/mobile.md §A.6), in the menus' words
  'gesture.actions': 'Actions',
  'gesture.reconnect': 'Reconnect',
  'gesture.next-task': 'Next task',
  'gesture.previous-task': 'Previous task',
  'gesture.side-panel': 'Side panel',
  'gesture.move-up': 'Move up',
  'gesture.move-down': 'Move down',
  // the platform's own word for its own control (design/mobile.md: *Select* on both)
  select: 'Select',
  // a list's selection mode (SelectionBar): Android's contextual bar, iOS's edit mode, each in its platform's words
  'select.count': '{count} selected',
  'select.count-ios': '{count} Selected',
  'select.none-ios': 'Select Items',
  'select.all': 'Select all',
  'select.all-ios': 'Select All',
  'select.deselect': 'Deselect all',
  'select.deselect-ios': 'Deselect All',
  'select.exit': 'Close selection',
  done: 'Done',
} as const;

export type KitString = keyof typeof KIT_STRINGS;
export type Translate = (key: KitString, vars?: Readonly<Record<string, string | number>>) => string;

/** Fills `{name}` placeholders. */
export function fill(text: string, vars?: Readonly<Record<string, string | number>>): string {
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in vars ? String(vars[name]) : whole));
}

export const englishT: Translate = (key, vars) => fill(KIT_STRINGS[key], vars);

const I18n = createContext<Translate>(englishT);

/** Gives every kit component below it a translator for the kit's own strings. */
export function KitProvider(props: { readonly t: Translate; readonly children?: JSX.Element }): JSX.Element {
  return <I18n.Provider value={props.t}>{props.children}</I18n.Provider>;
}

/** The translator: the component's own `t` prop if it has one, else the provider's, else English. */
export function useT(own?: () => Translate | undefined): Translate {
  const fromContext = useContext(I18n);
  return (key, vars) => (own?.() ?? fromContext)(key, vars);
}
