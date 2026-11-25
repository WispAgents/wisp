// Scan the code on Android (audit mobile r1, M1): the app's WispSsh had no scanCode, yet the seam was always given to
// Add computer, so its main button always ended in "type it by hand". The scanner is offered only when the native side
// lists the method (Capacitor's PluginHeaders, or the Plugins map its JSExport fills); with none, Add computer has no
// Scan section and the typed address is its main action. With one, a scanned code saves the computer with its key pinned.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { addSheet, scanOutcome } from '../src/add.ts';
import { exposePlugins, type CapacitorLike } from '../src/bridge.ts';
import { phonePlatform, type PhoneWindow } from '../src/boot.ts';
import { fakeBridge } from './helpers.ts';

const FP = 'SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU';
const PAIRING = `wisp+ssh://mo@studio.local:2222?fp=${FP}&name=Studio`;
const WISP_SSH = ['list', 'saveComputer', 'connect', 'secureGet', 'secureSet', 'secureRemove', 'openExternal'];

function androidWindow(cap: CapacitorLike): PhoneWindow {
  const media = (q: string): MediaQueryList => ({ matches: q.includes('pointer: coarse'), media: q, addEventListener: () => {}, removeEventListener: () => {} }) as unknown as MediaQueryList;
  return {
    navigator: { platform: 'Linux armv8l', userAgent: 'Android' } as Navigator,
    document: { documentElement: {}, visibilityState: 'visible', addEventListener: () => {}, removeEventListener: () => {} } as unknown as Document,
    matchMedia: media,
    addEventListener: () => {},
    removeEventListener: () => {},
    open: () => null,
    innerWidth: 390,
    innerHeight: 844,
    location: { reload: () => {}, origin: 'https://localhost', pathname: '/', search: '' },
    Capacitor: { getPlatform: () => 'android', ...cap },
  } as unknown as PhoneWindow;
}

/** Capacitor as the Android app injects it: the plugin headers of this build's Java, its native bridge. */
function nativeSide(methods: readonly string[], answers: Record<string, unknown> = {}): { cap: CapacitorLike; calls: string[] } {
  const calls: string[] = [];
  const cap: CapacitorLike = {
    isNativePlatform: () => true,
    nativePromise: async (_plugin, method) => {
      calls.push(method);
      if (!(method in answers)) throw new Error(`"WispSsh.${method}()" is not implemented on android`);
      return answers[method];
    },
    PluginHeaders: [{ name: 'WispSsh', methods: methods.map((name) => ({ name, rtype: 'promise' })) }],
  };
  return { cap, calls };
}

test('an_app_without_a_scanner_offers_the_typed_address_alone', async () => {
  const { cap, calls } = nativeSide(WISP_SSH);
  exposePlugins(cap);
  const platform = phonePlatform(androidWindow(cap), fakeBridge());
  assert.equal(platform.scanCode, undefined, 'no scanCode in this build: no scanner');
  assert.deepEqual(addSheet(platform.scanCode), { scan: false, primary: 'address' }, 'no Scan section; Save is the main button');
  assert.deepEqual(calls, [], 'nothing was called to find out');
  // a Plugins map Capacitor filled itself (its JSExport) without the method: the same
  const filled: CapacitorLike = { isNativePlatform: () => true, nativePromise: async () => ({}), Plugins: { WispSsh: { list: async () => ({}) } } };
  exposePlugins(filled);
  assert.equal(phonePlatform(androidWindow(filled), fakeBridge()).scanCode, undefined);
});

test('an_app_with_a_scanner_saves_the_scanned_computer_with_its_key_pinned', async () => {
  const { cap, calls } = nativeSide([...WISP_SSH, 'scanCode'], { scanCode: { text: PAIRING } });
  exposePlugins(cap);
  const platform = phonePlatform(androidWindow(cap), fakeBridge());
  assert.deepEqual(addSheet(platform.scanCode), { scan: true, primary: 'scan' });
  const scanned = await platform.scanCode?.();
  assert.ok(scanned);
  const o = scanOutcome(scanned);
  assert.notEqual(o.kind, 'manual', 'never "type it by hand"');
  assert.deepEqual(o, { kind: 'save', computer: { kind: 'ssh', name: 'Studio', target: 'mo@studio.local:2222', sshPort: 2222, fingerprint: FP } });
  assert.deepEqual(calls, ['scanCode']);
});
