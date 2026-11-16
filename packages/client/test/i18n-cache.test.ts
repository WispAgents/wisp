// A string is looked up once per catalogue change (the flicker gate's profile: walking ~60 catalogues for each t() was
// 11.5 ms of a Mark done's frame); what a plugin's lazy half adds later still shows, and a catalogue that goes takes
// its strings with it.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createI18n } from '../src/i18n.ts';
import { createScope } from '../src/scope.ts';

test('a string is found once, a later string still shows, and a plugin switched off takes its strings', () => {
  const i = createI18n();
  const scope = createScope();
  const cat: Record<string, string> = { 'p.hi': 'Hi {name}' };
  i.register('p', cat, scope);
  assert.equal(i.t('p.hi', { name: 'Ada' }), 'Hi Ada');
  assert.equal(i.t('p.hi', { name: 'Lin' }), 'Hi Lin', 'the cached string fills its vars each time');
  assert.equal(i.t('p.later'), 'p.later', 'missing for now');
  cat['p.later'] = 'Later';
  assert.equal(i.t('p.later'), 'Later', "the lazy half's string shows once it's there");
  scope.dispose();
  assert.equal(i.t('p.hi'), 'p.hi', 'the plugin went: so did its strings');
  i.register('p', { 'p.hi': 'Hello' }, createScope());
  assert.equal(i.t('p.hi'), 'Hello', 'a new catalogue is read anew');
});
