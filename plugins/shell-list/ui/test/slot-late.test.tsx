// @vitest-environment happy-dom
// round 324: the extensions on a list input (shell.row:hover, shell.list:crew…) come from the shell's registry, an
// alien signal; the slot and hasExtensions read it through useSignal, so a plugin whose later part (or that switches
// on) attaches to the row after the list painted shows on the rows already there, and one switched off goes.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { Show } from 'solid-js';
import { signal } from '@wisp/client';
import { Env } from '../src/ui/env.ts';
import { Slot, hasExtensions } from '../src/ui/Slot.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

test('a_row_extension_registered_after_the_list_painted_shows_and_one_removed_goes', () => {
  const ext = (id: string, text: string) => ({ id, config: undefined, def: { id, component: () => <span data-testid={id}>{text}</span> } });
  const attached = signal<readonly ReturnType<typeof ext>[]>([]);
  const env = { shell: { extensions: () => attached }, client: {} };
  const host = document.createElement('div');
  document.body.append(host);
  const Has = () => {
    const has = hasExtensions('shell.row:hover' as never);
    return <Show when={has()}>{<i data-testid="has">has</i>}</Show>;
  };
  const stop = render(
    () => (
      <Env.Provider value={env as never}>
        <Slot input={'shell.row:hover' as never} />
        <Has />
      </Env.Provider>
    ),
    host,
  );
  const q = (id: string): Element | null => host.querySelector(`[data-testid="${id}"]`);
  expect(q('demo.pin'), 'nothing attached yet').toBeNull();
  expect(q('has')).toBeNull();
  attached([ext('demo.pin', 'Pin')]);
  expect(q('demo.pin')?.textContent, 'the late extension on the row').toBe('Pin');
  expect(q('has'), 'hasExtensions follows it').not.toBeNull();
  attached([]);
  expect(q('demo.pin'), 'switched off: it goes').toBeNull();
  expect(q('has')).toBeNull();
  stop();
});
