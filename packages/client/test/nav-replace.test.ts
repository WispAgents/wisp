// What makes a history entry (src/nav.ts pushes): Back undoes those; a replacing open takes the current one's place
// (j/k stepping tasks, Settings switching panes beside its list: TRY-26).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { pushes } from '../src/nav.ts';

const settings = (pane: string) => ({ kind: 'viewer' as const, id: 'shell-settings.sheet' as never, params: { pane } });

test('opening an overlay makes a history entry unless it replaces the open one', () => {
  assert.equal(pushes({ t: 'openOverlay', overlay: settings('cloud') }), true);
  assert.equal(pushes({ t: 'openOverlay', overlay: settings('cloud'), replace: true }), false);
  assert.equal(pushes({ t: 'openTask', id: 'a:b' as never, replace: true }), false);
  assert.equal(pushes({ t: 'home' }), true);
  assert.equal(pushes({ t: 'openCrew', id: 'c' as never }), true);
});
