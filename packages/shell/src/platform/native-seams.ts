// The phone apps' native seams (ADR-0065 step 8) through the app's WispSsh plugin. The phone app's own bundle adds them
// to its platform (@wisp/mobile phonePlatform), so the browser's first download never carries them.
import { pluginFn } from './capacitor.ts';
import type { CapacitorGlobal, Platform, PushRegistration, Scanned, SecureStore } from './types.ts';

const rec = (v: unknown): Record<string, unknown> => (v && typeof v === 'object' ? (v as Record<string, unknown>) : {});
const str = (v: unknown): string => (typeof v === 'string' ? v : '');

/** The native seams (ADR-0065 step 8) through the app's WispSsh plugin: `secureGet`/`secureSet`/`secureRemove` (the
 * Keystore / Keychain), `scanCode`, `registerPush`. A native side without a method rejects the call: that answers
 * 'unsupported' (or null), never a throw. */
export function nativeSeams(cap: CapacitorGlobal | undefined, kind: 'android' | 'ios'): Required<Pick<Platform, 'secureStore' | 'scanCode' | 'registerPush'>> {
  const call = async (method: string, args?: unknown): Promise<Record<string, unknown> | undefined> => {
    const fn = pluginFn(cap, 'WispSsh', method);
    if (!fn) return undefined;
    try {
      return rec(await fn(args));
    } catch {
      return undefined;
    }
  };
  const secureStore: SecureStore = {
    get: async (key) => {
      const v = (await call('secureGet', { key }))?.['value'];
      return typeof v === 'string' ? v : null;
    },
    set: async (key, value) => ((await call('secureSet', { key, value })) ? 'set' : 'unsupported'),
    remove: async (key) => void (await call('secureRemove', { key })),
  };
  return {
    secureStore,
    async scanCode(): Promise<Scanned> {
      const r = await call('scanCode');
      if (!r) return 'unsupported';
      if (r['denied'] === true) return 'denied';
      const text = str(r['text']);
      return text ? { text } : 'cancelled';
    },
    async registerPush(): Promise<PushRegistration> {
      const r = await call('registerPush');
      if (!r) return 'unsupported';
      if (r['denied'] === true) return 'denied';
      const handle = str(r['handle']);
      const publicKey = str(r['publicKey']);
      return handle && publicKey ? { handle, publicKey, platform: kind } : 'unsupported';
    },
  };
}

