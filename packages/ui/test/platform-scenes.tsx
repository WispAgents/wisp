// The browser scenes of the components that grew a phone look (ADR-0065 build step 3): Frame and Toolbar,
// rendered with no data-platform. test/goldens/platform-browser.json holds their HTML from before the phone
// variants landed; platform-frame.test.tsx renders them again (with the phone-only props passed too) and
// expects the same bytes, so the browser's look (v1 parity, ADR-0016) can't move.
import type { JSX } from 'solid-js';
import { Frame, Text, Toolbar, type ToolbarItem } from '../src/index.ts';

const noop = (): void => {};
export const TOOLBAR_ITEMS: readonly ToolbarItem[] = [
  { id: 'done', label: 'Mark done', icon: 'done', priority: 3, onSelect: noop },
  { id: 'later', label: 'Save for later', icon: 'bookmark', priority: 2, onSelect: noop },
  { id: 'share', label: 'Share', icon: 'share', priority: 1, onSelect: noop },
  { id: 'send', label: 'Send', icon: 'send', priority: 'pin', onSelect: noop },
];

/** Each scene's view; `extra` adds what a phone-aware caller passes too (the browser must ignore it). */
export function scenes(extra: (name: string) => Record<string, unknown> = () => ({})): ReadonlyArray<readonly [string, () => JSX.Element]> {
  const top = (): JSX.Element => <Text>top</Text>;
  const pane = (s: string): JSX.Element => <Text>{s}</Text>;
  return [
    ['frame-split-side', () => <Frame layout="split" page="main" sideOpen top={top()} list={pane('list')} main={pane('main')} side={pane('side')} onListResize={noop} {...extra('frame')} />],
    ['frame-split-closed', () => <Frame layout="split" page="main" listOpen={false} top={top()} list={pane('list')} main={pane('main')} {...extra('frame')} />],
    ['frame-stack-list', () => <Frame layout="stack" page="list" top={top()} list={pane('list')} main={pane('main')} {...extra('frame')} />],
    ['frame-contained', () => <Frame contained layout="stack" page="main" top={top()} list={pane('list')} main={pane('main')} {...extra('frame')} />],
    ['toolbar', () => <Toolbar aria-label="Task actions" items={TOOLBAR_ITEMS} {...extra('toolbar')} />],
    ['toolbar-compact-end', () => <Toolbar aria-label="Bar" compact justify="end" items={TOOLBAR_ITEMS.slice(0, 2)} {...extra('toolbar')} />],
  ];
}
