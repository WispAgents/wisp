// perf r2 #8: a tab switch in the side panel keeps the panel when the same code draws the next tab.
import { describe, expect, it } from 'vitest';
import { componentIds, sideKey, type DrawnBy } from '../src/frame/side-key.ts';

const TaskSide = {};
const Other = {};
const by: Record<string, DrawnBy> = {
  changes: { plugin: 'wisp.shell-task', component: TaskSide, config: {} },
  shell: { plugin: 'wisp.shell-task', component: TaskSide, config: {} },
  pr: { plugin: 'wisp.shell-task', component: TaskSide, config: {} },
  preview: { plugin: 'wisp.preview', component: Other, config: {} },
};

describe('sideKey', () => {
  it('keeps one key for a task\'s tabs drawn by the same code', () => {
    const idOf = componentIds();
    const k = (kind: string, id: string): string => sideKey({ kind, id }, (x) => by[x], idOf);
    expect(k('changes', 'local:a')).toBe(k('shell', 'local:a'));
    expect(k('changes', 'local:a')).toBe(k('pr', 'local:a'));
  });

  it('builds afresh for another task, another plugin\'s tab, other config or a tab nobody fills', () => {
    const idOf = componentIds();
    const k = (kind: string, id: string, f: (x: string) => DrawnBy | undefined = (x) => by[x]): string => sideKey({ kind, id }, f, idOf);
    expect(k('changes', 'local:a')).not.toBe(k('changes', 'local:b'));
    expect(k('changes', 'local:a')).not.toBe(k('preview', 'local:a'));
    expect(k('changes', 'local:a')).not.toBe(k('changes', 'local:a', (x) => ({ ...(by[x] as DrawnBy), config: { wide: true } })));
    expect(k('watch', 'local:a')).toBe('watch:local:a');
    expect(k('term', 'local:a')).not.toBe(k('watch', 'local:a'));
  });

  it('closed is empty and a view is keyed by its id', () => {
    const idOf = componentIds();
    expect(sideKey({ kind: 'none' }, () => undefined, idOf)).toBe('');
    expect(sideKey({ kind: 'view', id: 'stuff.app' }, () => undefined, idOf)).toBe('view:stuff.app');
  });
});
