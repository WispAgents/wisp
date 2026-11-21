// createShell: the extension host and the app (plan §6.6, §4.6). It registers plugin UIs (each with
// its own PluginClient and scope), resolves the extension tree, owns the shell's own state (theme,
// the keyboard's task, the list's width, the recent tasks) and renders the frame. Built-in screens
// come through the same register() as anyone's (plan §6.0.3): the shell's own pieces are the
// `shell` plugin in builtins.tsx.

import { render } from 'solid-js/web';
import type { JSX } from 'solid-js';
import { computed, effect, signal, untracked, type Client, type PeerId, type PluginClient, type Readable } from '@wisp/client';
import { afterFirstScreen as afterFirstScreenKit, firstScreenDrawn, GestureHostProvider, KitProvider, KIT_STRINGS, loadPlatformStyles, type KitString, type Toaster } from '@wisp/ui';
import type { PluginUiDef, PromptBox, SendHold, SendHoldAsk, SettingsPaneDef, ShellApi, ShellContext } from './contract.ts';
import { namespaceOf } from './define.ts';
import { SHELL_STRINGS } from './i18n/en.ts';
import { applyPlatformLook } from './platform/look.ts';
import { gestureHostOf } from './platform/gesture-host.ts';
import type { Platform } from './platform/types.ts';
import { createRegistry, type Warn } from './registry.ts';
import { createTheme } from './theme.ts';
import { ShellProvider } from './context.tsx';
import { App } from './frame/App.tsx';
import { NONE as NO_SELECTION, pruneSelect, selectAll, startSelect, toggleSelect, type ListSelection } from './frame/list-select.ts';
import { shellUi } from './builtins.tsx';
import { SHELL_SETTINGS, SIDE_MAX, SIDE_MIN } from './settings-decls.ts';
import { listShown } from './frame/pane-widths.ts';
import { keepsOverlaysOnLoad } from './frame/overlays-on-load.ts';
import { installKeys } from './keyboard.ts';
import { watchCoreTopics } from './watch.ts';
import { pluginStyles } from './theme-styles/mount.ts';
import { answeredInPlace, errorWords, wireNotices } from './toasts.ts';
import { defaultModelName } from './frame/default-models.ts';

/** The part of `window` the shell uses; tests pass happy-dom's. */
/** the latest the first screen counts as drawn after the render, whatever the home list does (ADR-0105) */
const FIRST_SCREEN_CAP_MS = 10_000;

export interface ShellWindow {
  readonly innerWidth: number;
  readonly document: Document;
  readonly localStorage?: Storage;
  /** how the page was loaded (a reload, a link) and its URL's search: what the first frame keeps open (overlays-on-load) */
  readonly performance?: { getEntriesByType?(type: string): readonly object[]; mark?(name: string): unknown };
  /** the first screen's frames (ADR-0105); without it (a test's window) a microtask stands in. Additive. */
  readonly requestAnimationFrame?: (cb: () => void) => number;
  readonly location?: { readonly search: string };
  matchMedia(q: string): MediaQueryList;
  addEventListener(type: string, fn: (e: Event) => void, opts?: boolean | AddEventListenerOptions): void;
  removeEventListener(type: string, fn: (e: Event) => void, opts?: boolean | EventListenerOptions): void;
}

export interface ShellOptions {
  client: Client;
  toaster: Toaster;
  platform: Platform;
  /** this client's own wisp: where the theme setting lives */
  home: PeerId;
  window: ShellWindow;
  warn?: Warn;
  /** the phone app: whether Tasks can open yet (ShellContext.tasksReady); absent: always */
  tasksReady?: Readable<boolean>;
}

export interface Shell extends ShellApi {
  /** `run` once the first screen is drawn (ADR-0105), with the plugins' later parts: deferred work (the overlays'
   *  machines) never competes with it. In the budget gate's run (`?first-screen`) it never runs. Additive. */
  afterFirstScreen(run: () => void): void;
  /** the shell's own plugin client (for its built-ins and tests) */
  readonly self: PluginClient;
  pluginClient(id: string): PluginClient | undefined;
  /** every plugin's Settings panes, each with the plugin that gave it (the Settings frame lists them) */
  settingsPanes(): () => readonly (SettingsPaneDef & { readonly plugin: string })[];
}

const RECENT_MAX = 20;

/** the wire's error codes (wisp-kernel `Code`) that have words of their own in `shell.err.*` */
const ERROR_CODES = new Set(['not_found', 'invalid', 'forbidden', 'conflict', 'unavailable', 'timeout', 'service_gone', 'vetoed', 'plugin_off', 'too_large', 'internal', 'unknown']);

// size-ok: the factory wires the shell's parts together; each part lives in its own module
export function createShell(o: ShellOptions): Shell {
  const { client, toaster, platform } = o;
  const warn: Warn = o.warn ?? ((msg, data) => client.devtools.record({ kind: 'warn', message: data === undefined ? msg : `${msg}: ${String(data)}` }));
  const self = client.plugin('shell');
  self.strings(SHELL_STRINGS);
  const kitScope = client.scope.child('kit');
  client.i18n.register('kit', Object.fromEntries(Object.entries(KIT_STRINGS).map(([k, v]) => [`kit.${k}`, v])), kitScope);
  // the answer blocks' copy (`block.*`): the screens that show blocks register it as they load; fetched here just after
  // the start for any other, apart from the first page's bundle (block-strings.ts)
  // (once the first screen is drawn, ADR-0105: Home draws no answer)
  afterFirstScreenKit(() => void import('./block-strings.ts').then((m) => m.blockStrings(client)).catch(() => undefined));
  const settings = self.settings(SHELL_SETTINGS);
  const t: ShellContext['t'] = (key, vars) => client.i18n.t(key, vars);
  const kitT = (key: KitString, vars?: Readonly<Record<string, string | number>>): string => client.i18n.t(`kit.${key}`, vars);

  const registry = createRegistry(warn);
  const clients = new Map<string, PluginClient>([['shell', self]]);
  // each plugin's tabs beside a task, with the client its tabs and body run as
  const sideTabs = computed(() =>
    registry.sideTabs().flatMap((d) => {
      const pc = clients.get(d.plugin);
      return pc ? [{ ...d, client: pc }] : [];
    }),
  );
  // each plugin's preview addresses (ADR-0064), with the client its map reads as
  const previewAddresses = computed(() =>
    registry.previewAddresses().flatMap((d) => {
      const pc = clients.get(d.plugin);
      return pc ? [{ ...d, client: pc }] : [];
    }),
  );
  // each plugin's account-menu links and Home tips (docs/ux/2026-11-18-grid.md #9), with the client they read as
  const accountLinks = computed(() =>
    registry.accountLinks().flatMap((d) => {
      const pc = clients.get(d.plugin);
      return pc ? [{ ...d, client: pc }] : [];
    }),
  );
  const homeTips = computed(() =>
    registry.homeTips().flatMap((d) => {
      const pc = clients.get(d.plugin);
      return pc ? [{ ...d, client: pc }] : [];
    }),
  );
  const theme = createTheme({
    pref: settings.get('shell.theme'),
    lockedBy: settings.lockedBy('shell.theme'),
    write: (p) => settings.set('shell.theme', p),
    root: o.window.document.documentElement,
    dark: o.window.matchMedia('(prefers-color-scheme: dark)'),
    storage: o.window.localStorage,
    onLocked: (by) => toaster.show(t('shell.toast.locked', { by })),
  });
  self.scope.add(() => theme.dispose());
  // the phone apps' look (ADR-0065 §5): data-platform on <html> and its stylesheet; the browser gets neither
  const look = applyPlatformLook(o.window.document.documentElement, platform);
  if (look) void loadPlatformStyles(look).catch((e: unknown) => warn('platform styles', e));

  const selected = signal<string | undefined>(undefined);
  const focusTargets = new Map<string, () => void>();
  const prompts = new Map<string, PromptBox>();
  // what may make a send wait (ADR-0091): each plugin's sendHolds, with its client, then any provided by hand
  const holdChecks = new Map<string, (a: SendHoldAsk) => Promise<SendHold | undefined>>();
  const holdDefs = (): { reason: string; check: (a: SendHoldAsk) => Promise<SendHold | undefined> }[] =>
    registry.sendHolds().flatMap((d) => {
      const pc = clients.get(d.plugin);
      return pc ? [{ reason: d.reason, check: (a: SendHoldAsk) => d.check({ client: pc }, a) }] : [];
    });
  const holdCheckList = (): ((a: SendHoldAsk) => Promise<SendHold | undefined>)[] => [...holdDefs().map((d) => d.check), ...holdChecks.values()];
  const holdCheckFor = (reason: string): ((a: SendHoldAsk) => Promise<SendHold | undefined>) | undefined =>
    holdDefs().find((d) => d.reason === reason)?.check ?? holdChecks.get(reason);
  answeredInPlace((reason) => holdCheckFor(reason) !== undefined);
  errorWords((code) => (ERROR_CODES.has(code) ? t(`shell.err.${code}`) : undefined), t('shell.pane.retry'));
  const taskCard = signal<((key: string) => JSX.Element) | undefined>(undefined);
  // the list's order (published by the list) and the phone list's selection mode (frame/list-select.ts)
  const listOrder = signal<readonly string[]>([]);
  const picking = signal<ListSelection>(NO_SELECTION);
  const pickOn = computed(() => picking().on);
  const pickKeys = computed(() => picking().keys);
  const recent = signal<readonly string[]>([]);
  const nav = client.nav.nav;
  const openTask = computed(() => {
    const m = nav().main;
    return m.kind === 'task' ? m.id : undefined;
  });

  const context: ShellContext = {
    client,
    nav,
    navigate: (intent) => client.nav.navigate(intent),
    t,
    toast: toaster,
    platform,
    ...(o.tasksReady && { tasksReady: o.tasksReady }),
    theme: { pref: theme.pref, resolved: theme.resolved, set: theme.set },
    selection: { get: () => selected(), set: (k) => selected(k) },
    sidebar: {
      open: () => settings.get('shell.sidebar.open')(),
      toggle: () => {
        const n = untracked(nav);
        // the side panel has the list's place (a window under 1180): [ and the sidebar button bring the list back by
        // putting the panel away, as v1's toggleSidebar did; there's room for one of the two
        if (n.layout === 'split' && n.side.kind !== 'none' && !listShown(true, true, o.window.innerWidth) && untracked(settings.get('shell.sidebar.open')) !== false) {
          client.nav.navigate({ t: 'closeSide' });
          return;
        }
        void settings.set('shell.sidebar.open', !untracked(settings.get('shell.sidebar.open')));
      },
      width: () => settings.get('shell.sidebar.width')(),
      setWidth: (px) => void settings.set('shell.sidebar.width', Math.round(Math.min(SIDE_MAX, Math.max(SIDE_MIN, px)))),
    },
    projectFilter: {
      value: settings.get('shell.projectFilter') as Readable<string>,
      set: (id) => void settings.set('shell.projectFilter', id || 'all'),
    },
    computerFilter: {
      value: settings.get('shell.computerFilter') as Readable<string>,
      set: (peer) => void settings.set('shell.computerFilter', peer || 'all'),
    },
    listOrder: {
      keys: listOrder,
      publish: (keys) => {
        const was = untracked(listOrder);
        if (was.length !== keys.length || was.some((k, i) => k !== keys[i])) listOrder(keys);
        // a picked row the list no longer shows leaves the selection
        const s = untracked(picking);
        const next = pruneSelect(s, keys);
        if (next !== s) picking(next);
      },
    },
    listSelect: {
      on: pickOn,
      keys: pickKeys,
      start: (key) => picking(startSelect(key)),
      toggle: (key) => picking(toggleSelect(untracked(picking), key)),
      all: () => picking(selectAll(untracked(picking), untracked(listOrder))),
      exit: () => picking(NO_SELECTION),
    },
    prompts: {
      provide(name, box) {
        prompts.set(name, box);
        return () => {
          if (prompts.get(name) === box) prompts.delete(name);
        };
      },
      get: (name) => prompts.get(name),
    },
    sendHolds: {
      provide(reason, check) {
        holdChecks.set(reason, check);
        return () => {
          if (holdChecks.get(reason) === check) holdChecks.delete(reason);
        };
      },
      async check(a) {
        for (const check of holdCheckList()) {
          const h = await check(a).catch(() => undefined);
          if (h) return h;
        }
        return undefined;
      },
      answers: (reason) => (reason === undefined ? undefined : holdCheckFor(reason)),
    },
    taskCards: {
      provide(render) {
        taskCard(render);
        return () => {
          if (untracked(taskCard) === render) taskCard(undefined);
        };
      },
      render: taskCard,
    },
    focusTargets: {
      provide(name, focus) {
        focusTargets.set(name, focus);
        return () => {
          if (focusTargets.get(name) === focus) focusTargets.delete(name);
        };
      },
      focus(name) {
        const fn = focusTargets.get(name);
        fn?.();
        return fn !== undefined;
      },
    },
    palette: { open: () => client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.palette' as never, params: null } }) },
    defaultModel: (agent, at) => defaultModelName(client, at.peer, agent, at.cwd),
    afterFirstScreen: (run) => afterFirstScreen(run),
    extensions: (input) => registry.input(input),
    settingsPanes: () => registry.settingsPanes,
    harnesses: () => registry.harnesses,
    promptSources: () => registry.prompt,
    launchTargets: () => registry.launchTargets,
    laterReady: () => {
      // asked for (a test, a screen that needs every plugin's later part): every deferred thing starts now, first
      // screen or not
      runDeferred();
      for (const retry of [...failedLaters]) retry();
      return laterLoads.size ? Promise.all([...laterLoads]).then(() => undefined) : undefined;
    },
    sideTabs: () => sideTabs,
    previewAddresses: () => previewAddresses,
    accountLinks: () => accountLinks,
    homeTips: () => homeTips,
    block: (type) => registry.block(type),
    view: (kind) => registry.view(kind),
  };

  /** each plugin's later part, waiting for the first screen (ADR-0105) */
  const laters: (() => void)[] = [];
  let rendered = false;
  /** the later parts go: from now on a plugin's later part loads as it registers */
  const startLaters = (): void => {
    rendered = true;
    for (const load of laters.splice(0)) load();
  };
  /** the app's own deferred work, waiting with the later parts */
  const afterFirst: (() => void)[] = [];
  /** everything that waits for the first screen (ADR-0105), once: the later parts, the app's, the kit's */
  function runDeferred(): void {
    startLaters();
    for (const run of afterFirst.splice(0)) run();
    // the kit's own deferred work (the phone's Settings pages)
    firstScreenDrawn();
  }
  function afterFirstScreen(run: () => void): void {
    if (rendered) run();
    else afterFirst.push(run);
  }
  /** later parts on their way, and the loads that failed (laterReady tries them again) */
  const laterLoads = new Set<Promise<void>>();
  const failedLaters = new Set<() => void>();

  function register(def: PluginUiDef): void {
    if (registry.has(def.id)) throw new Error(`plugin UI ${def.id} is registered twice`);
    const ns = namespaceOf(def.id);
    const pc = clients.get(def.id) ?? client.plugin(ns);
    clients.set(def.id, pc);
    if (def.catalogue && Object.keys(def.catalogue).length > 0) {
      // a string is the plugin's under its namespace (`crew.x`) or its full id (`wisp.crew.x`), as an
      // extension id may be; the full-id ones are registered under the id, so each key still has one owner
      const full = `${def.id}.`;
      // each half reads through to the catalogue, so strings its lazy code adds later (i18n/lazy.ts) show in both
      const cat = def.catalogue;
      const isId = (k: string): boolean => def.id !== ns && k.startsWith(full);
      const keys = Object.keys(cat);
      if (keys.some((k) => !isId(k))) pc.strings(keys.some(isId) ? liveView(cat, (k) => !isId(k)) : cat);
      if (keys.some(isId)) client.i18n.register(def.id, liveView(cat, isId), pc.scope);
    }
    for (const v of def.views ?? []) {
      pc.registerView({
        id: v.kind,
        slot: v.slot,
        ...(v.overlay && { overlay: v.overlay }),
        parse: (params) => {
          try {
            return v.parse(params) === undefined ? undefined : params;
          } catch {
            return undefined;
          }
        },
      });
    }
    for (const e of def.extensions ?? []) if (!e.id.startsWith(`${ns}.`) && !e.id.startsWith(`${def.id}.`)) warn(`${def.id}'s extension ${e.id} isn't prefixed with ${ns}.`);
    registry.add(def);
    let gone = false;
    let retry: (() => void) | undefined;
    pc.scope.add(() => {
      gone = true;
      if (retry) failedLaters.delete(retry);
      registry.remove(def.id);
      clients.delete(def.id);
    });
    def.setup?.(pc);
    const later = def.later;
    if (later) {
      const load = (): void => {
        failedLaters.delete(load);
        if (gone) return;
        const p: Promise<void> = later()
          .then(
            (m) => {
              if (!gone) registry.extend(def.id, m.default);
            },
            (err: unknown) => {
              if (!gone) failedLaters.add(load);
              warn(`${def.id}'s later part didn't load`, String(err));
            },
          )
          .finally(() => laterLoads.delete(p));
        laterLoads.add(p);
      };
      retry = load;
      // ADR-0067, ADR-0105: once the first screen is drawn (nothing on it waits for them), or at once after that
      if (rendered) load();
      else laters.push(load);
    }
  }

  register(shellUi(context));
  watchCoreTopics(client, o.home, self.scope);
  wireNotices(context, self.scope);
  // ADR-0007: trusted themes' contributes.partStyles, in @layer plugins, while the shell lives
  pluginStyles(client, self, o.home, o.window.document, self.scope.child('plugin-styles'));
  self.scope.add(platform.onOpen((target) => client.nav.navigate({ t: 'openFromToast', target })));

  const shell: Shell = {
    client,
    self,
    context,
    nav,
    register,
    unregister(id) {
      if (id === 'shell') return;
      clients.get(id)?.scope.dispose();
    },
    afterFirstScreen: (run) => afterFirstScreen(run),
    pluginClient: (id) => clients.get(id),
    settingsPanes: () => registry.settingsPanes as () => readonly (SettingsPaneDef & { readonly plugin: string })[],
    render(root) {
      // a reload (or the desktop app reopening its last place) brings back the place, never what was open over it: the
      // palette, Settings, Machines… (v1 kept only the open task; QA round 3 #9, round 4 #2). One rule for every overlay
      // (frame/overlays-on-load.ts); a link opened on purpose with `?o=` keeps its overlay. Closed before the first
      // frame, the URL replaced, no history entry
      const entry = o.window.performance?.getEntriesByType?.('navigation')[0] as { readonly type?: string } | undefined;
      if (!keepsOverlaysOnLoad({ navigation: entry?.type, search: o.window.location?.search ?? '' }))
        while (untracked(client.nav.nav).overlays.length > 0) client.nav.navigate({ t: 'closeOverlay' });
      const scope = self.scope.child('render');
      scope.add(trackRecent());
      scope.add(installKeys(o.window, context, registry, (id) => clients.get(id) ?? self));
      const onResize = (): void => client.nav.navigate({ t: 'resize', width: o.window.innerWidth });
      o.window.addEventListener('resize', onResize);
      scope.add(() => o.window.removeEventListener('resize', onResize));
      // the phone apps: the kit's gestures buzz and reach the native accessibility tree through the platform module
      const host = gestureHostOf(platform);
      const app = (): JSX.Element => <App clients={{ client: (id) => clients.get(id) ?? self, viewOwner: (kind) => registry.viewOwner(kind), palette: registry.palette, shortcuts: registry.shortcuts }} recent={recent} />;
      const stop = render(
        () => (
          <ShellProvider value={context}>
            <KitProvider t={kitT}>{host ? <GestureHostProvider host={host}>{app()}</GestureHostProvider> : app()}</KitProvider>
          </ShellProvider>
        ),
        root,
      );
      scope.add(stop);
      // the plugins' later parts (their palette entries, panes, blocks) wait for the first screen: drawn, the home list
      // here, painted (ADR-0105); the budget gate counts what loads before that mark (tools/first-screen.ts)
      // `?first-screen` (the budget gate's run, tools/first-screen.ts): they never start, so what the first screen loads
      // is all that loads, to the end of its lazy chains
      const measuring = /[?&]first-screen(?:&|=|$)/.test(o.window.location?.search ?? '');
      scope.add(
        whenFirstScreen(o.window, () => {
          if (!measuring) runDeferred();
        }),
      );
      return () => scope.dispose();
    },
    dispose() {
      for (const [id, pc] of clients) if (id !== 'shell') pc.scope.dispose();
      self.scope.dispose();
      kitScope.dispose();
    },
  };

  /** `run` once the first screen is there (ADR-0105): the home list's snapshot here (or restored as whole), or the home
   *  computer away (asleep, unreachable: nothing more is coming), then two frames painted; `wisp:first-screen` is
   *  marked then and `run` goes when the page is idle. Returns its stop. */
  function whenFirstScreen(win: ShellWindow, run: () => void): () => void {
    let done = false;
    let stopped = false;
    const frame = (f: () => void): void => {
      if (typeof win.requestAnimationFrame === 'function') win.requestAnimationFrame(() => f());
      else queueMicrotask(f);
    };
    const idle = (f: () => void): void => {
      const ric = (win as { requestIdleCallback?: (cb: () => void, o?: { timeout: number }) => number }).requestIdleCallback;
      if (typeof ric === 'function') ric(() => f(), { timeout: 1000 });
      else frame(f);
    };
    const go = (): void => {
      if (done) return;
      done = true;
      frame(() =>
        frame(() => {
          win.performance?.mark?.('wisp:first-screen');
          idle(() => !stopped && run());
        }),
      );
    };
    // a safety cap: a home list that never arrives (a refused subscription, a peer stuck connecting) still lets the
    // deferred work start, FIRST_SCREEN_CAP_MS after the render
    const cap = self.scope.timeout(FIRST_SCREEN_CAP_MS, () => untracked(go));
    const watch = effect(() => {
      const here = client.entities.loaded(o.home, 'list' as never)();
      const status = client.peers.status(o.home)().kind;
      if (here || (status !== 'connecting' && status !== 'online' && status !== 'waking')) untracked(go);
    });
    return () => {
      stopped = true;
      cap();
      watch();
    };
  }

  /** the tasks you were on, latest first (⌃Tab's order): the open task moves to the front */
  function trackRecent(): () => void {
    return effect(() => {
      const id = openTask();
      if (id !== undefined) untracked(() => recent([id, ...recent().filter((x) => x !== id)].slice(0, RECENT_MAX)));
    });
  }

  return shell;
}

/** `cat`'s keys that pass `keep`, read live: a key added to `cat` later shows here too. */
function liveView(cat: Readonly<Record<string, string>>, keep: (key: string) => boolean): Readonly<Record<string, string>> {
  return new Proxy(cat, {
    get: (o, k) => (typeof k === 'string' && keep(k) ? o[k] : undefined),
    has: (o, k) => typeof k === 'string' && keep(k) && k in o,
    ownKeys: (o) => Reflect.ownKeys(o).filter((k) => typeof k === 'string' && keep(k)),
    getOwnPropertyDescriptor: (o, k) => (typeof k === 'string' && keep(k) ? Reflect.getOwnPropertyDescriptor(o, k) : undefined),
  });
}
