// ADR-0065 step 13: Add computer (the QR through the native scanner, typed by hand when there is none), Trust the new
// key on a computer whose key changed, and Stay connected ON by default for an SSH computer without push.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { alreadySaved, keyChanged, manualComputer, needsStay, scanOutcome, stayConnected } from '../src/add.ts';
import type { Computer } from '../src/bridge.ts';

const FP = 'SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU';

test('a_scanned_wisp_code_saves_the_computer_pinned_and_no_scanner_or_a_refused_camera_falls_back_to_typing', () => {
  assert.deepEqual(scanOutcome({ text: `wisp+ssh://mo@studio.local?fp=${FP}&name=Studio` }), { kind: 'save', computer: { kind: 'ssh', name: 'Studio', target: 'mo@studio.local', fingerprint: FP } });
  assert.deepEqual(scanOutcome('unsupported'), { kind: 'manual', why: 'unsupported' });
  assert.deepEqual(scanOutcome('denied'), { kind: 'manual', why: 'denied' });
  assert.deepEqual(scanOutcome({ text: 'https://example.com' }), { kind: 'manual', why: 'notWisp' }, 'another QR code: type it in');
  assert.deepEqual(scanOutcome('cancelled'), { kind: 'none' }, 'Back out of the scanner: nothing happens');
});

test('a_computer_typed_by_hand_is_you_at_host_with_an_optional_port_and_no_pin', () => {
  assert.deepEqual(manualComputer('', 'mo@studio.local'), { computer: { kind: 'ssh', name: 'studio.local', target: 'mo@studio.local' } });
  assert.deepEqual(manualComputer(' Lab ', ' mo@10.0.0.7:2222 '), { computer: { kind: 'ssh', name: 'Lab', target: 'mo@10.0.0.7:2222', sshPort: 2222 } });
  assert.deepEqual(manualComputer('', 'mo@[fd7a::1]:22'), { computer: { kind: 'ssh', name: 'fd7a::1', target: 'mo@[fd7a::1]' } });
  assert.deepEqual(manualComputer('x', ''), { error: 'empty' });
  assert.deepEqual(manualComputer('x', 'studio.local'), { error: 'noUser' }, 'SSH needs the user');
  assert.deepEqual(manualComputer('x', 'mo@studio:99999'), { error: 'port' });
  assert.deepEqual(manualComputer('x', 'mo@studio:0'), { error: 'port' });
  const typed = manualComputer('', 'mo@studio.local');
  assert.ok('computer' in typed && typed.computer.fingerprint === undefined, 'typed by hand: the first key is shown and kept, not pinned');
});

test('the_same_address_again_is_the_saved_computer_not_a_second_one', () => {
  const list: Computer[] = [{ id: 'a', kind: 'ssh', name: 'Studio', target: 'mo@studio.local' }];
  assert.equal(alreadySaved(list, { kind: 'ssh', name: 'S', target: 'mo@studio.local' })?.id, 'a');
  assert.equal(alreadySaved(list, { kind: 'ssh', name: 'S', target: 'mo@studio.local:2222' }), undefined);
});

test('a_refused_host_key_on_a_computers_status_offers_trust_the_new_key_and_other_failures_dont', () => {
  assert.deepEqual(keyChanged({ kind: 'unreachable', reason: `This computer’s key changed: it is now ${FP}.` }), { fingerprint: FP, pinned: false });
  assert.deepEqual(keyChanged({ kind: 'unreachable', reason: `This computer’s key isn’t the one its QR code showed: it is now ${FP}.` }), { fingerprint: FP, pinned: true });
  assert.equal(keyChanged({ kind: 'unreachable', reason: 'Connection refused' }), undefined);
  assert.equal(keyChanged({ kind: 'online' }), undefined);
  assert.equal(keyChanged(undefined), undefined);
});

test('stay_connected_is_on_by_default_for_an_ssh_computer_without_push_and_the_persons_choice_wins', () => {
  const ssh: Computer = { id: 's', kind: 'ssh', name: 'Studio', target: 'mo@studio' };
  const pushed: Computer = { ...ssh, id: 'p', push: true };
  const grid: Computer = { id: 'g', kind: 'grid', name: 'Cloud', url: 'https://mo.trywisp.dev' };
  assert.equal(stayConnected([ssh], null), true, 'v1 had it on whenever connected; ADR-0065 §3.6 keeps it for SSH without push');
  assert.equal(stayConnected([pushed, grid], null), false, 'push carries the alerts: nothing to hold');
  assert.equal(stayConnected([], null), false);
  assert.equal(stayConnected([ssh], '0'), false, 'turned off stays off');
  assert.equal(stayConnected([grid], '1'), true);
  assert.equal(needsStay([ssh, grid]), true);
  assert.equal(needsStay([pushed, grid]), false);
});

test('the_stay_connected_switch_asks_the_native_side_once_per_change_and_keeps_the_persons_choice', async () => {
  const { createScope } = await import('@wisp/client');
  const { followStayConnected } = await import('../src/stay.ts');
  const { STAY_CONNECTED } = await import('../src/add.ts');
  let list: Computer[] = [{ id: 's', kind: 'ssh', name: 'Studio', target: 'mo@studio' }];
  const asked: boolean[] = [];
  const fns = new Set<() => void>();
  const bridge = {
    list: async () => list,
    onComputers: (fn: () => void) => (fns.add(fn), () => fns.delete(fn)),
    setStayConnected: async (on: boolean) => (asked.push(on), on),
  };
  const store = new Map<string, string>();
  const storage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v), removeItem: (k: string) => void store.delete(k) };
  const scope = createScope();
  const stay = followStayConnected(bridge, storage, scope);
  await new Promise((r) => setTimeout(r, 0));
  assert.deepEqual(asked, [true], 'an SSH computer without push: on, without a press');
  assert.equal(stay.on(), true);
  for (const fn of fns) fn(); // the list reloads, same answer: not asked again
  await new Promise((r) => setTimeout(r, 0));
  assert.deepEqual(asked, [true]);
  stay.set(false);
  assert.equal(store.get(STAY_CONNECTED), '0');
  assert.deepEqual(asked, [true, false]);
  list = [...list, { id: 't', kind: 'ssh', name: 'Lab', target: 'mo@lab' }];
  for (const fn of fns) fn();
  await new Promise((r) => setTimeout(r, 0));
  assert.deepEqual(asked, [true, false], 'turned off stays off when another computer is added');
  scope.dispose();
  assert.equal(fns.size, 0, 'the scope takes its listener with it');
});
