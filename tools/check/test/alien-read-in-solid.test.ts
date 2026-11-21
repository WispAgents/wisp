import assert from 'node:assert/strict';
import test from 'node:test';
import { alienReadInSolid } from '../rules/alien-read-in-solid.ts';
import { fixture, messages, run } from './helpers.ts';

test('alien-read-in-solid: store, settings, nav, computed, family, readers, shaped objects, handed-back getters and getters handed to a call fail inside createMemo/createEffect/on deps/JSX; the bridge, handlers, onMount, timers, on callbacks, untracked, shadowed names, kit readable props, a reasoned allow, the client package and tests pass', async () => {
  const root = fixture('alien-read-in-solid');
  assert.deepEqual(await run(alienReadInSolid, root), [
    'plugins/demo/ui/src/Bad.tsx:11 alien-read-in-solid', // createEffect over a const bound to computed(…)
    'plugins/demo/ui/src/Bad.tsx:12 alien-read-in-solid', // an imported reader (memberOf reads crewOf(c)())
    'plugins/demo/ui/src/Bad.tsx:14 alien-read-in-solid', // an imported function's returned object: its `value` is a computed
    'plugins/demo/ui/src/Bad.tsx:17 alien-read-in-solid', // a c.settings(…) handle's get(…)()
    'plugins/demo/ui/src/Bad.tsx:19 alien-read-in-solid', // on(…)'s deps
    'plugins/demo/ui/src/Bad.tsx:20 alien-read-in-solid', // a getter handed to a call made now, over a Readable-typed function
    'plugins/demo/ui/src/Bad.tsx:22 alien-read-in-solid', // the allow comment has no reason
    'plugins/demo/ui/src/Bad.tsx:25 alien-read-in-solid', // JSX: the shell's nav
    'plugins/demo/ui/src/Bad.tsx:26 alien-read-in-solid', // JSX: the shell's selection
    'plugins/demo/ui/src/Bad.tsx:27 alien-read-in-solid', // JSX: a getter an imported function hands back
    'plugins/demo/ui/src/Bad.tsx:28 alien-read-in-solid', // JSX: a family's f(k)()
    'plugins/demo/ui/src/Bad.tsx:29 alien-read-in-solid', // JSX: a props member typed by an interface with a Readable prop
    'plugins/demo/ui/src/Bad.tsx:9 alien-read-in-solid', // createMemo: c.entities.row(…)() in a .map callback
    'plugins/demo/ui/src/More.tsx:5 alien-read-in-solid', // a const bound to c.entities
    'plugins/demo/ui/src/More.tsx:7 alien-read-in-solid', // createMemo(reader): a reader handed over by name
    'plugins/demo/ui/src/More.tsx:8 alien-read-in-solid', // createDeferred
  ]);
  const all = (await messages(alienReadInSolid, root)).join('\n');
  assert.match(all, /useSignal\(…\) or useDerived\(…\)/);
  assert.match(all, /check:allow alien-read-in-solid <reason>/);
});
