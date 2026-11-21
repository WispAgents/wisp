// The browser scenes of the controls that grew a phone look (ADR-0065 build step 6): Button, IconButton, Switch, Tabs,
// Input, TextArea, Field and Select, every variant, size and state, rendered with no data-platform.
// test/goldens/platform-browser-controls.json holds their HTML from before the phone variants landed;
// platform-controls.test.tsx renders them again (with the phone-only props passed too) and expects the same bytes, so
// the browser's look (v1 parity, ADR-0016) can't move.
import type { JSX } from 'solid-js';
import { Button, Field, IconButton, Input, Select, Switch, Tabs, TextArea, type SelectOption } from '../src/index.ts';

const noop = (): void => {};
export const FEW: readonly SelectOption[] = [
  { value: 'claude', label: 'Claude Code' },
  { value: 'codex', label: 'Codex' },
  { value: 'opencode', label: 'opencode', disabled: true },
];
export const MANY: readonly SelectOption[] = ['Studio', 'Laptop', 'Cloud', 'Mini', 'Pi', 'Desk', 'Build box', 'Grid'].map((label) => ({ value: label.toLowerCase(), label }));
const TABS = [
  { value: 'all', label: 'All', count: 12 },
  { value: 'docs', label: 'Docs', icon: 'file' as const },
  { value: 'media', label: 'Media' },
  { value: 'apps', label: 'Apps', disabled: true },
];

/** Each scene's view; `extra` adds what a phone-aware caller passes too (the browser must ignore it). */
export function controlScenes(extra: (kind: string) => Record<string, unknown> = () => ({})): ReadonlyArray<readonly [string, () => JSX.Element]> {
  return [
    ['button-variants', () => <span>{(['primary', 'secondary', 'ghost', 'danger', 'chip', 'link', 'accent'] as const).map((v) => <Button variant={v} {...extra('button')}>{v}</Button>)}</span>],
    ['button-sizes', () => <span>{(['xs', 'sm', 'md', 'lg'] as const).map((s) => <Button variant="primary" size={s} icon="plus" {...extra('button')}>New task</Button>)}</span>],
    ['button-states', () => <span><Button disabled>Off</Button><Button loading variant="primary">Sending</Button><Button pressed variant="chip">On</Button><Button tone="good" variant="ghost" icon="done">Mark done</Button><Button fill kbd="⌘↵" iconEnd="chevron">Run</Button></span>],
    ['icon-buttons', () => <span>{(['ghost', 'secondary', 'primary'] as const).map((v) => (['xs', 'sm', 'md', 'lg'] as const).map((s) => <IconButton icon="dots" label="More" variant={v} size={s} {...extra('icon-button')} />))}<IconButton icon="trash" label="Delete" tone="bad" disabled /><IconButton icon="x" label="Close" size="xs" wide loading /></span>],
    ['switches', () => <span><Switch checked onChange={noop} aria-label="On" /><Switch checked={false} onChange={noop} label="Stay connected" name="stay" /><Switch checked disabled onChange={noop} label="Locked" /></span>],
    ['tabs-line', () => <Tabs items={TABS} aria-label="Kinds" {...extra('tabs')} />],
    ['tabs-pill', () => <Tabs items={TABS.slice(0, 3)} variant="pill" fill defaultValue="docs" {...extra('tabs')} />],
    ['tabs-seg', () => <Tabs items={TABS.slice(0, 3)} variant="seg" fieldset {...extra('tabs')} />],
    ['inputs', () => <span><Input value="Studio" aria-label="Name" {...extra('input')} /><Input size="sm" placeholder="Search" icon="search" type="search" {...extra('input')} /><Input size="lg" prefix="https://" value="trywisp.dev" mono {...extra('input')} /><Input invalid disabled value="x" aria-label="Bad" {...extra('input')} /></span>],
    ['textarea', () => <TextArea value="Fix the flaky test" minRows={2} aria-label="Prompt" {...extra('input')} />],
    ['field', () => <Field label="Task name" hint="Shown in the list." error="Too long" required requiredMark="*"><Input value="A name" {...extra('input')} /></Field>],
    ['field-textarea', () => <Field label="Notes"><TextArea placeholder="Anything else" {...extra('input')} /></Field>],
    ['select', () => <Select items={FEW} value="codex" onChange={noop} aria-label="Harness" {...extra('select')} />],
    ['select-empty', () => <Select items={FEW} value={undefined} onChange={noop} placeholder="Pick one" size="sm" invalid name="h" {...extra('select')} />],
    ['select-many', () => <Field label="Computer"><Select items={MANY} value="studio" onChange={noop} {...extra('select')} /></Field>],
  ];
}
