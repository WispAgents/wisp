// @vitest-environment happy-dom
// QA round 14 #5: the Share dialog drew nothing of what goes out or what is hidden, though share.preview answers both.
// The dialog's part, on the real answer (test/goldens/share-preview.json, from a real `wisp serve`): the ask, with its
// work the answers and the file changed, and each redaction once with how often; without its work, the report's
// parts and their redactions are gone.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { KitProvider, englishT } from '@wisp/ui';
import goldenRaw from './goldens/share-preview.json?raw';
import lazy from '../src/i18n/en.lazy.json' with { type: 'json' };
import { WhatGoes } from '../src/share/DialogParts.tsx';
import type { SharePreview } from '../src/share/types.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

const S = lazy as Record<string, string>;
const t = (k: string, v: Readonly<Record<string, string | number>> = {}): string => {
  const n = v['count'];
  const key = typeof n === 'number' && S[`${k}.${n === 1 ? 'one' : 'other'}`] ? `${k}.${n === 1 ? 'one' : 'other'}` : k;
  return (S[key] ?? k).replace(/\{(\w+)\}/g, (_m, name: string) => String(v[name] ?? `{${name}}`));
};
const golden = JSON.parse(goldenRaw) as SharePreview;

const draw = (work: boolean): HTMLElement => {
  const host = document.createElement('div');
  document.body.append(host);
  render(() => (
    <KitProvider t={englishT}>
      <WhatGoes t={t} info={golden} work={work} />
    </KitProvider>
  ), host);
  return host;
};

test('with its work: the ask, one answer and its file, and each thing hidden once with how often', () => {
  const host = draw(true);
  const text = host.textContent ?? '';
  expect(text).toContain('What goes');
  expect(host.querySelector('[data-part~="share.preview.ask"]')?.textContent).toBe('fix the add bug, <secret> TOPIC[Fix add]');
  expect(text).toContain('1 answer · 1 file changed');
  expect(host.querySelector('[data-part~="share.preview.files"]')?.textContent).toBe('<project>/math.js');
  const rows = [...host.querySelectorAll('[data-part~="share.hidden.row"]')].map((r) =>
    [...r.children].map((c) => c.textContent),
  );
  expect(rows).toEqual([
    ['your folder', '/tmp/…/proj', '→ <project>', '2×'],
    ['a secret', 'toke…efgh', '→ <secret>', '2×'],
  ]);
});

test('without its work: the ask only, and only what is hidden in it', () => {
  const host = draw(false);
  expect(host.textContent).not.toContain('answer');
  expect(host.querySelector('[data-part~="share.preview.files"]')).toBeNull();
  const rows = [...host.querySelectorAll('[data-part~="share.hidden.row"]')].map((r) =>
    [...r.children].map((c) => c.textContent),
  );
  expect(rows).toEqual([
    ['your folder', '/tmp/…/proj', '→ <project>'],
    ['a secret', 'toke…efgh', '→ <secret>'],
  ]);
});
