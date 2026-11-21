import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fromPairingCode } from '../src/pair.ts';

const FP = 'SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU';

// ADR-0065 §2: the desktop's *Add to your phone* QR pins the host key before the first connect.
test('a_qr_code_pins_its_fingerprint', () => {
  assert.deepEqual(fromPairingCode(`wisp+ssh://mo@studio.local:2222?fp=${encodeURIComponent(FP)}&name=Studio`), { kind: 'ssh', name: 'Studio', target: 'mo@studio.local:2222', sshPort: 2222, fingerprint: FP });
  assert.deepEqual(fromPairingCode(`wisp+ssh://mo@100.64.0.7?fp=${FP}`), { kind: 'ssh', name: '100.64.0.7', target: 'mo@100.64.0.7', fingerprint: FP }, 'a raw + in the fingerprint survives');
  assert.deepEqual(fromPairingCode(`wisp+ssh://mo@[fd7a::1]:22?fp=${FP}&name=Lab`), { kind: 'ssh', name: 'Lab', target: 'mo@[fd7a::1]', fingerprint: FP });
});

test('a code without a user, a valid pin or the scheme is not a pairing code', () => {
  assert.equal(fromPairingCode(`wisp+ssh://studio?fp=${FP}`), undefined, 'no user');
  assert.equal(fromPairingCode('wisp+ssh://mo@studio?fp=SHA256:short'), undefined, 'a broken pin');
  assert.equal(fromPairingCode('wisp+ssh://mo@studio'), undefined, 'no pin: typed-by-hand rules apply, not a QR');
  assert.equal(fromPairingCode(`https://mo@studio?fp=${FP}`), undefined);
  assert.equal(fromPairingCode('hello'), undefined);
});
