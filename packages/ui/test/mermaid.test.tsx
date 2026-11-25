// ```mermaid fences (content/Mermaid.tsx, ADR-0118), with mermaid itself mocked: an open fence stays code and loads
// nothing; a closed one draws the SVG; a source that won't render falls back to code and a note; a theme change draws
// it again in the other theme; the bar's toggle and copy work.
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { render } from 'solid-js/web';
import { createSignal } from 'solid-js';
import { Markdown } from '../src/content/rich.ts';
import { resetMermaid } from '../src/content/Mermaid.tsx';
import { until } from './helpers.tsx';

const m = vi.hoisted(() => ({
  imported: 0,
  initialize: vi.fn<(c: Record<string, unknown>) => void>(),
  render: vi.fn<(id: string, src: string) => Promise<{ svg: string }>>(),
}));
vi.mock('mermaid', () => {
  m.imported++;
  return { default: { initialize: m.initialize, render: m.render } };
});

let dispose: (() => void) | undefined;
beforeEach(() => {
  resetMermaid();
  m.initialize.mockClear();
  m.render.mockReset();
  m.render.mockImplementation(async (_id, src) => ({ svg: `<svg data-src="${src.length}"><text>drawn</text></svg>` }));
});
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
  document.documentElement.removeAttribute('data-theme');
});

function mount(ui: () => import('solid-js').JSX.Element): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(ui, host);
  return host;
}

/** Lets queued promises run (the mocked import and render resolve in microtasks and a macrotask or two). */
const settle = (): Promise<void> => new Promise((r) => setTimeout(r, 20));

test('an open fence renders as code and never imports mermaid', async () => {
  const host = mount(() => <Markdown source={'```mermaid\ngraph TD\n  A-->B'} />);
  await settle();
  expect(host.querySelector('[data-kit="code-block"] code')?.getAttribute('data-lang')).toBe('mermaid');
  expect(host.querySelector('[data-kit="mermaid"]')).toBeNull();
  expect(m.imported).toBe(0);
  expect(m.render).not.toHaveBeenCalled();
});

test('a closed fence shows a skeleton, then the diagram, drawn strict with the page theme', async () => {
  const host = mount(() => <Markdown source={'```Mermaid\ngraph TD\n  A-->B\n```'} />);
  expect(host.querySelector('[data-kit="mermaid"] [data-kit="skeleton"]')).not.toBeNull();
  await until(() => host.querySelector('[data-kit="mermaid-svg"] svg') !== null);
  expect(host.querySelector('[data-kit="mermaid-svg"] svg')?.textContent).toBe('drawn');
  expect(host.querySelector('[data-kit="mermaid"] [data-kit="skeleton"]')).toBeNull();
  expect(m.initialize).toHaveBeenCalledWith(expect.objectContaining({ startOnLoad: false, securityLevel: 'strict', theme: 'default' }));
  expect(m.render).toHaveBeenCalledWith(expect.stringMatching(/^wisp-mermaid-\d+$/), 'graph TD\n  A-->B');
});

test('the same source is drawn once: the SVG is cached by source and scheme', async () => {
  const src = '```mermaid\ngraph LR\n  X-->Y\n```';
  mount(() => (
    <>
      <Markdown source={src} />
      <Markdown source={src} />
    </>
  ));
  await settle();
  expect(m.render).toHaveBeenCalledTimes(1);
});

test('a source that fails to render shows as code with a note naming the error', async () => {
  m.render.mockRejectedValue(new Error('Parse error on line 2:\n...A-->\n----^'));
  const host = mount(() => <Markdown source={'```mermaid\ngraph TD\n  A-->\n```'} />);
  await until(() => host.querySelector('[data-kit="mermaid-note"]') !== null);
  expect(host.querySelector('[data-kit="mermaid"] [data-kit="code-block"]')?.textContent).toContain('A-->');
  expect(host.querySelector('[data-kit="mermaid-note"]')?.textContent).toBe('Diagram couldn’t render: Parse error on line 2:');
  expect(host.querySelector('[data-kit="mermaid-svg"]')).toBeNull();
});

test('a theme change draws it again in the other theme', async () => {
  const host = mount(() => <Markdown source={'```mermaid\ngraph TD\n  A-->B\n```'} />);
  await until(() => host.querySelector('[data-kit="mermaid-svg"] svg') !== null);
  expect(m.render).toHaveBeenCalledTimes(1);
  document.documentElement.setAttribute('data-theme', 'dark');
  await until(() => m.render.mock.calls.length === 2);
  expect(m.initialize).toHaveBeenLastCalledWith(expect.objectContaining({ theme: 'dark', darkMode: true }));
  expect(host.querySelector('[data-kit="mermaid-svg"] svg')).not.toBeNull();
});

test('rapid source changes draw once they settle', async () => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  try {
    const [src, setSrc] = createSignal('```mermaid\ngraph TD\n  A-->B\n```');
    mount(() => <Markdown source={src()} />);
    await vi.advanceTimersByTimeAsync(0);
    setSrc('```mermaid\ngraph TD\n  A-->C\n```');
    setSrc('```mermaid\ngraph TD\n  A-->D\n```');
    await vi.advanceTimersByTimeAsync(400);
    expect(m.render.mock.calls.map((c) => c[1])).toEqual(['graph TD\n  A-->B', 'graph TD\n  A-->D']);
  } finally {
    vi.useRealTimers();
  }
});

test('the bar: View code toggles to the source and back, Copy copies the source', async () => {
  const writeText = vi.fn(async () => undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  const host = mount(() => <Markdown source={'```mermaid\ngraph TD\n  A-->B\n```'} />);
  await until(() => host.querySelector('[data-kit="mermaid-svg"] svg') !== null);
  const toggle = host.querySelector<HTMLButtonElement>('[data-part="ui.markdown.mermaid.toggle"]');
  expect(toggle?.getAttribute('aria-label')).toBe('View code');
  toggle?.click();
  expect(host.querySelector('[data-kit="mermaid"] [data-kit="code-block"]')?.textContent).toBe('graph TD\n  A-->B');
  expect(host.querySelector('[data-kit="mermaid-svg"]')).toBeNull();
  expect(toggle?.getAttribute('aria-label')).toBe('View diagram');
  toggle?.click();
  expect(host.querySelector('[data-kit="mermaid-svg"] svg')).not.toBeNull();
  host.querySelector<HTMLButtonElement>('[data-part="ui.markdown.mermaid.copy"]')?.click();
  await until(() => writeText.mock.calls.length > 0);
  expect(writeText).toHaveBeenCalledWith('graph TD\n  A-->B');
  expect(host.querySelector('[data-part="ui.markdown.mermaid.full"]')?.getAttribute('aria-label')).toBe('Open full size');
});
