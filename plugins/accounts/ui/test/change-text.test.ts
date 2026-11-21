// TRY-17 (a): Settings → Agents' change line read "Codex changed since 17 Nov: , but its program’s bytes differ" when
// the harness reports no version (the server sends `version: ""`, and `??` kept the empty string). With no version it
// says the program's bytes differ, or where it moved; a version still reads as before.
import { expect, test } from 'vitest';
import type { HarnessChange, Pin } from '../src/types.ts';
import { changeText, changeTitle, changeWhat } from '../src/words.ts';

const at = Date.UTC(2026, 10, 17, 12);
const pin = (o: Partial<Pin> = {}): Pin => ({ path: '/usr/local/bin/codex', real: '/opt/codex/bin/codex', version: '', sha256: 'a', at, ...o });
const change = (from: Partial<Pin>, to: Partial<Pin>, kind: HarnessChange['kind'] = 'bytes'): HarnessChange => ({ kind, at, from: pin(from), to: pin({ sha256: 'b', ...to }) });

test('a_change_with_no_version_reads_well_never_an_empty_what', () => {
  const same = changeText('Codex', change({}, {}), 'en-GB');
  expect(same).toBe('Codex changed since 17 Nov: its program’s bytes differ');
  expect(same).not.toContain(': ,');
  // moved, no versions: from where to where
  expect(changeText('Codex', change({}, { path: '/opt/homebrew/bin/codex', real: '/opt/homebrew/bin/codex' }, 'moved'), 'en-GB')).toBe('Codex changed since 17 Nov: /usr/local/bin/codex → /opt/homebrew/bin/codex');
  // versions read as before
  expect(changeText('Codex', change({ version: '0.1.2' }, { version: '0.1.3' }, 'update'), 'en-GB')).toBe('Codex changed since 17 Nov: 0.1.2 → 0.1.3');
  expect(changeText('Codex', change({ version: '0.1.2' }, { version: '0.1.2' }), 'en-GB')).toBe('Codex changed since 17 Nov: 0.1.2, but its program’s bytes differ');
});

// QA phone r4 #5: the notice's title was the whole sentence, cut where the width ran out: "Codex changed since 16 Nov:…"
// on Android. The title is who and since when (it fits); what changed opens the description.
test('the_change_notices_title_is_who_and_since_when_and_what_changed_leads_its_description', () => {
  const c = change({}, {});
  expect(changeTitle('Codex', c, 'en-GB')).toBe('Codex changed since 17 Nov');
  expect(changeTitle('Codex', c, 'en-GB')).not.toMatch(/:/);
  expect(changeWhat(c)).toBe('its program’s bytes differ');
  expect(changeWhat(change({ version: '0.1.2' }, { version: '0.1.3' }, 'update'))).toBe('0.1.2 → 0.1.3');
});
