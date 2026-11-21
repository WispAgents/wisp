// The `platform` module (plan §7): what differs between the browser, the Tauri desktop app and the
// Capacitor phone apps (Android, iPhone: ADR-0065) goes through this one interface, so no feature checks MOBILE or sniffs a
// user agent. Every call answers what happened ('unsupported' is an answer, never a throw), so a
// feature can offer the action everywhere and say so where it can't.

import type { Target } from '@wisp/client';

export type PlatformKind = 'browser' | 'desktop' | 'android' | 'ios';

/** The phone apps' look (ADR-0065 §5): what the shell writes as `data-platform` on <html>. The browser has none. */
export type PlatformLook = 'android' | 'ios';

/** Edge insets in CSS px: the status bar, the notch, the home indicator, the gesture bar. */
export interface Insets {
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly left: number;
}

/** The window's width class (M3 window size classes; design/mobile.md §A.3): compact < 600, medium < 840. */
export type SizeClass = 'compact' | 'medium' | 'expanded';

export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface WindowLayout {
  readonly width: number;
  readonly height: number;
  readonly size: SizeClass;
  /** a foldable's hinge (or the gap between its two screens) in page px; absent on a flat screen */
  readonly hinge?: Rect;
}

/** Predictive Back (Android 14+) and the interactive edge swipe (iOS): the gesture's progress, then its end. */
export type BackEvent =
  | { readonly phase: 'start'; readonly edge: 'left' | 'right' }
  | { readonly phase: 'progress'; readonly edge: 'left' | 'right'; /** 0–1 */ readonly progress: number }
  | { readonly phase: 'cancel' }
  | { readonly phase: 'commit' };

export interface NotifyOptions {
  title: string;
  body?: string;
  /** one notification per tag: a newer one replaces it (one per task) */
  tag?: string;
  /** where tapping it goes: the shell navigates there with openFromToast (#573) */
  target?: Target;
  silent?: boolean;
}

export type Permission = 'granted' | 'denied' | 'prompt' | 'unsupported';
export type Shown = 'shown' | 'denied' | 'unsupported';

export interface ShareData {
  title?: string;
  text?: string;
  url?: string;
  files?: readonly File[];
}

export interface PickOptions {
  /** 'image/*,.pdf' */
  accept?: string;
  multiple?: boolean;
  /** the phone's camera instead of its files */
  capture?: boolean;
}

export type Haptic = 'tap' | 'select' | 'success' | 'warning' | 'error';

/** Why dictation stopped (the Android app's Dictation.java words). */
export type DictationError = 'denied' | 'blocked' | 'silence' | 'network' | 'busy' | 'audio' | 'failed';
export type DictationEvent =
  | { readonly state: 'listening' }
  | { readonly state: 'partial' | 'final'; readonly text: string }
  | { readonly state: 'error'; readonly error: DictationError; readonly text?: string }
  | { readonly state: 'end' };

/** Speech to text (the Android app's SpeechRecognizer). One listens at a time. */
export interface Dictation {
  /** starts listening; the returned function stops it (the final result still comes, then `end`) */
  start(onEvent: (e: DictationEvent) => void): () => void;
}

/** added 2026-11-15 (ADR-0065 step 8): small values sealed on the device (the Android Keystore, the iOS Keychain), never
 * in web storage. `get` answers null for a key never set (or a platform without one). */
export interface SecureStore {
  get(key: string): Promise<string | null>;
  set(key: string, value: string): Promise<'set' | 'unsupported'>;
  remove(key: string): Promise<void>;
}

/** What the camera's code scanner read (the desktop's *Add to your phone* QR, ADR-0065 §2.1). */
export type Scanned = { readonly text: string } | 'cancelled' | 'denied' | 'unsupported';

/** The push relay's handle for this device and the key its notifications are sealed to (ADR-0065 §3.1). */
export type PushRegistration = { readonly handle: string; readonly publicKey: string; readonly platform: 'android' | 'ios' } | 'denied' | 'unsupported';

export interface Platform {
  readonly kind: PlatformKind;
  /** touch-first and narrow: the phone layout's affordances (a FAB, press feedback) */
  readonly mobile: boolean;
  /** added 2026-11-14 (additive, ADR-0065): a phone app (Android, iPhone), not a page in a phone's browser. The
   * computer wisp runs on is elsewhere (no Show in folder), and rows swipe. Features ask this, never `kind`. */
  readonly phone: boolean;
  /** installed (home-screen web app, or a native shell): the service worker serves the page */
  readonly standalone: boolean;
  /** ⌘ on Apple platforms, Ctrl elsewhere: how `mod+k` is shown and matched */
  readonly apple: boolean;
  /** the window's own title bar: 'mac' when the window buttons sit over the page's top-left corner and
   * the page is the title bar (the desktop app on a Mac: room for them, and the top bar drags the
   * window); 'none' when the window has its own, or there's no window (a browser tab, a phone) */
  readonly titlebar: 'mac' | 'none';
  /** the page's own origin (`https://box.grid.trywisp.dev`, `http://127.0.0.1:4791`): where a sign-in returns to */
  readonly origin: string;
  /** where the page is the title bar ('mac'): a press on a region marked to drag moves the window.
   * Returns the stop function; a no-op elsewhere. */
  dragRegions(root: Element): () => void;
  notifyPermission(): Promise<Permission>;
  /** asks once; a second ask after a refusal answers 'denied' without a prompt */
  requestNotify(): Promise<Permission>;
  notify(n: NotifyOptions): Promise<Shown>;
  /** the app icon's count (0 clears it) */
  badge(count: number): Promise<'set' | 'unsupported'>;
  share(d: ShareData): Promise<'shared' | 'cancelled' | 'unsupported'>;
  pickFiles(o?: PickOptions): Promise<readonly File[]>;
  /** added 2026-11-12 (additive): the system's folder dialog, where there is one (the desktop app); null when you
   * cancel. Absent in a browser: there, a folder is typed with the server's suggestions (kit PathField). */
  pickFolder?(): Promise<string | null>;
  haptic(kind: Haptic): void;
  /** added 2026-11-15 (additive, design/mobile.md §A.6): gives `el` custom accessibility actions in the native tree
   * (TalkBack `AccessibilityNodeInfo` custom actions, VoiceOver `UIAccessibilityCustomAction`), so a screen reader runs
   * every gesture without making it; returns the function that takes them off. Absent (a browser, the desktop app, a
   * phone app whose native side doesn't offer it yet): the actions stay in the DOM, beside their visible twins. */
  accessibilityActions?(el: Element, actions: readonly { readonly label: string; readonly run: () => void }[]): () => void;
  /** added 2026-11-13 (additive): the attach button offers Take photo and Record video (the Android app's camera
   * handler, Capture.java). Absent or false: the file picker, as in a browser. */
  readonly camera?: boolean;
  /** added 2026-11-13 (additive): a mic beside the prompt types what you say (the Android app). Absent: no mic. */
  readonly dictation?: Dictation;
  copy(text: string): Promise<boolean>;
  openExternal(url: string): void;
  /** the app came back to the foreground (a phone unlocking, a window focused) */
  onResume(fn: () => void): () => void;
  /** a notification was tapped: where it goes */
  onOpen(fn: (target: Target) => void): () => void;
  /** loads the page again (a new build is ready) */
  reload(): void;
  /** added 2026-11-14 (additive, ADR-0065 §5): what the system draws over the page's edges now. A browser and
   * the desktop app answer the page's own `env(safe-area-inset-*)` (zero where there is none). */
  safeArea(): Insets;
  /** added 2026-11-14 (additive): the window's size class and a foldable's hinge (the Viewport Segments API on the
   * web; WindowLayoutInfo in the Android app) */
  windowLayout(): WindowLayout;
  /** added 2026-11-14 (additive): a page shown over the app and back (Custom Tabs, SFSafariViewController) for a
   * preview; a browser opens a tab and the desktop app the system browser, as `openExternal` */
  openInApp(url: string): void;
  /** added 2026-11-14 (additive): the system's Back gesture with its progress (predictive Back on Android, the
   * edge swipe on iOS). Returns the stop function. A browser and the desktop app never call it (their Back is
   * the history's). */
  onBack(fn: (e: BackEvent) => void): () => void;
  /** added 2026-11-15 (additive, ADR-0065 step 8): the phone apps' native seams, absent elsewhere. Each answers
   * 'unsupported' (or null) when the app's native side doesn't have it yet; none throws. */
  readonly secureStore?: SecureStore;
  /** the camera's code scanner, for *Scan from your computer* (step 13) */
  scanCode?(): Promise<Scanned>;
  /** asks the system for notifications and the relay for a handle, for `push.pair` (step 13; ADR-0065 §3) */
  registerPush?(): Promise<PushRegistration>;
}

/** The slice of `window` the implementations read; tests pass a fake. */
export interface PlatformWindow {
  readonly navigator: Navigator;
  readonly document: Document;
  matchMedia(q: string): MediaQueryList;
  addEventListener(type: string, fn: (e: Event) => void): void;
  removeEventListener(type: string, fn: (e: Event) => void): void;
  open(url: string, target?: string, features?: string): unknown;
  readonly innerWidth?: number;
  readonly innerHeight?: number;
  /** the Viewport Segments API: one segment per screen of a foldable (Chrome 138+) */
  readonly viewport?: { readonly segments?: readonly Rect[] | null };
  getComputedStyle?(el: Element): CSSStyleDeclaration;
  readonly location: { reload(): void; readonly origin: string };
  readonly Notification?: typeof Notification;
  readonly __TAURI_INTERNALS__?: { invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> };
  readonly Capacitor?: CapacitorGlobal;
  /** the Android app's page bridge (MainActivity's `wispAndroid`): an older app lacks some of it */
  readonly wispAndroid?: AndroidBridge;
}

/** The part of MainActivity's `window.wispAndroid` the platform reads (each method may be missing). */
export interface AndroidBridge {
  camera?(): boolean;
  dictation?(): boolean;
  dictate?(): void;
  dictateStop?(): void;
}

/** The part of Capacitor's global the phone implementations use (no @capacitor dependency). */
export interface CapacitorGlobal {
  isNativePlatform?(): boolean;
  getPlatform?(): string;
  Plugins?: Record<string, Record<string, ((args?: unknown) => Promise<unknown>) | undefined> | undefined>;
}
