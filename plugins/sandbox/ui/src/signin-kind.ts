// The Claude sign-in row's entity kind (host/src/launches.rs SIGNIN). Its id reads like the `sandbox.signin` string,
// so it lives beside the lazy strings its users (Launching, Sandboxes) load anyway, not in the first screen's
// launches.ts (ADR-0011: a module naming a lazy key imports the loader).
import './i18n/lazy.ts';

export const SIGNIN = 'sandbox.signin';
