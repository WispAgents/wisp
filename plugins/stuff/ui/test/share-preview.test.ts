// QA round 13 #1: the Share dialog crashed on open. It read `targets` (and `shared`, `work`) from `share.preview`,
// which the server never answered, so `ready()` threw and Share stayed disabled. The dialog's readers run here on a
// real answer: test/goldens/share-preview.json, written by the e2e on a real `wisp serve`
// (serve_share_redact.rs `the_share_dialogs_preview_is_the_golden_its_readers_are_tested_on`). A field one side
// drops fails one of the two.
import { expect, test } from 'vitest';
import goldenRaw from './goldens/share-preview.json?raw';
import lazy from '../src/i18n/en.lazy.json' with { type: 'json' };
import { canShare, carryChips, previewNote, workOption } from '../src/share/model.ts';
import type { SharePreview } from '../src/share/types.ts';

const S = lazy as Record<string, string>;
const t = (k: string, v: Readonly<Record<string, string | number>> = {}): string => {
  const n = v['count'];
  const key = typeof n === 'number' && S[`${k}.${n === 1 ? 'one' : 'other'}`] ? `${k}.${n === 1 ? 'one' : 'other'}` : k;
  return (S[key] ?? k).replace(/\{(\w+)\}/g, (_m, name: string) => String(v[name] ?? `{${name}}`));
};
const golden = JSON.parse(goldenRaw) as SharePreview;

test('the dialog reads the server’s real preview: nowhere to go yet says sign in, its files can go', () => {
  // the shape the server answers, as the dialog types it
  expect(Object.keys(golden).sort()).toEqual(['bundle', 'redactions', 'shared', 'targets', 'work']);
  expect(canShare(golden)).toBe(false);
  expect(previewNote(t, golden)).toEqual({ text: S['stuff.share.signIn'], tone: 'bad' });
  expect(workOption(t, golden.work)).toEqual({ can: true, hint: S['stuff.share.work.files'] });
  expect(carryChips(t, golden, 1).map((c) => c.text)).toEqual(['1 turn']);
});

test('with a ready destination Share can go, and the note says where', () => {
  const p: SharePreview = { ...golden, targets: [{ id: 'wisp-cloud', host: 'wisp Cloud' }] };
  expect(canShare(p)).toBe(true);
  expect(previewNote(t, p).text).toContain('On wisp Cloud.');
});

test('an answer without the dialog’s fields (the server before the fix) never throws: nothing to share it with', () => {
  const old: SharePreview = { bundle: golden.bundle, redactions: [] };
  expect(() => canShare(old)).not.toThrow();
  expect(canShare(old)).toBe(false);
  expect(previewNote(t, old).tone).toBe('bad');
  expect(workOption(t, old.work)).toEqual({ can: false, hint: S['stuff.share.work.none'] });
  expect(canShare(undefined)).toBe(false);
});
