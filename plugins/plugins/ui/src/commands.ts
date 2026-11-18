// Settings → Plugins' reads and writes: every plugin the kernel has (plugins.list) and its switch
// (plugins.setEnabled). The list is read when the pane opens and after a switch.
import { defineCommand, type PeerId } from '@wisp/client';
import './i18n/lazy.ts';

export interface PluginRow {
  readonly id: string;
  readonly version: string;
  /** active | resolved | off | refused */
  readonly state: string;
  readonly enabled: boolean;
  readonly builtin: boolean;
  readonly reason?: string;
  /** a plugin of yours: trusted or not (ADR-0048); absent for a built-in */
  readonly trusted?: boolean;
  /** a plugin of yours: what it asks for */
  readonly permissions?: readonly string[];
  /** a plugin of yours: its manifest's name */
  readonly name?: string;
}

/** Trust a plugin of yours, or take the trust back (`plugins.trust`, ADR-0048): it reloads, on again if it was. */
export const trust = defineCommand<{ peer: PeerId; id: string; trusted: boolean }>({
  id: 'plugins.trust',
  owner: (a) => a.peer,
  wire: (a) => ({ id: a.id, trusted: a.trusted }),
  optimistic: 'none',
  pending: 'the plugin’s Trust',
});

/** What a row says (audit A F19): its name, its version (and id, for one of yours), its trust and what that means, and
 *  what it asks for; Trust or Untrust for one of yours. */
export function rowAbout(p: PluginRow, say: (k: string, v?: Readonly<Record<string, string>>) => string): { title: string; lines: string[]; trust?: { label: string; next: boolean } } {
  const version = say('plugins.version', { version: p.version });
  if (p.builtin || p.trusted === undefined) return { title: p.name ?? p.id.replace(/^wisp\./, ''), lines: [p.reason ?? version] };
  const asks = p.permissions ?? [];
  return {
    title: p.name ?? p.id,
    lines: [
      p.reason ?? `${version} · ${p.id}`,
      say(p.trusted ? 'plugins.trusted' : 'plugins.untrusted'),
      ...(asks.length > 0 ? [`${say('plugins.mayDo')} ${asks.map((x) => permissionWords(x, say)).join(', ')}`] : []),
    ],
    trust: p.trusted ? { label: say('plugins.untrust'), next: false } : { label: say('plugins.trust'), next: true },
  };
}

/** The folders a typed path could go on to (wisp.runtime's `runtime.folders`, v1 GET /fs?q=): the install box's
 * suggestions. Answered in the form typed (`~/…` stays `~/…`). */
export const folders = defineCommand<{ peer: PeerId; q: string }, string[]>({ id: 'runtime.folders', owner: (a) => a.peer, wire: (a) => ({ q: a.q }), optimistic: 'none', pending: 'the folder suggestions' });

export const list = defineCommand<{ peer: PeerId }, PluginRow[]>({ id: 'plugins.list', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the plugins list' });
export const setEnabled = defineCommand<{ peer: PeerId; id: string; enabled: boolean }>({
  id: 'plugins.setEnabled',
  owner: (a) => a.peer,
  wire: (a) => ({ id: a.id, enabled: a.enabled }),
  optimistic: 'none',
  pending: 'the plugin’s switch',
});

/** What `plugins.install` answers: the plugin, off, and what it asks for (switching it on is the consent). */
export interface Installed {
  readonly id: string;
  readonly name: string;
  readonly version: string;
  readonly permissions: readonly string[];
  readonly folder: string;
}

export const install = defineCommand<{ peer: PeerId; source: string }, Installed>({
  id: 'plugins.install',
  owner: (a) => a.peer,
  wire: (a) => ({ source: a.source }),
  optimistic: 'none',
  pending: 'Install',
});
export const uninstall = defineCommand<{ peer: PeerId; id: string }>({
  id: 'plugins.uninstall',
  owner: (a) => a.peer,
  wire: (a) => ({ id: a.id }),
  optimistic: 'none',
  pending: 'the plugin’s Remove',
});

/** A permission in words (`proc:exec{bin:git}` → "run git"); one not known here reads as it is. */
export function permissionWords(p: string, say: (k: string, v?: Readonly<Record<string, string>>) => string): string {
  const m = /^([a-z-]+):([a-zA-Z.*-]+)(?:\{(.*)\})?$/.exec(p.trim());
  if (!m) return p;
  const [, ns = '', verb = '', args = ''] = m;
  const kv = new Map(
    args
      .split(',')
      .map((x) => x.split(':').map((y) => y.trim()))
      .filter((x): x is [string, string] => x.length === 2),
  );
  const arg = (k: string): string => kv.get(k) ?? '';
  if (ns === 'proc' && verb === 'exec') return say('plugins.perm.exec', { bin: arg('bin') || '*' });
  if (ns === 'net') return say('plugins.perm.net', { host: args || '*' });
  if (ns === 'fs') return say(verb === 'write' ? 'plugins.perm.fsWrite' : 'plugins.perm.fsRead', { path: args || '*' });
  if (verb === 'read' || verb === 'list' || verb === 'get') return say('plugins.perm.read', { what: ns });
  return say('plugins.perm.do', { what: `${ns} ${verb}` });
}

/** Cast (v1 spell-ui.js cast): a task whose agent writes the plugin you described, as a user plugin, and installs it off. */
export const cast = defineCommand<{ peer: PeerId; prompt: string; title: string; ext: { 'wisp.plugins': { kind: 'cast' } } }, { id: string }>({
  id: 'runtime.create',
  owner: (a) => a.peer,
  optimistic: 'none',
  pending: 'the cast box',
});

/** What the agent is asked: the change in your words, and how a wisp plugin is made and installed (ADR-0046). */
export const castPrompt = (ask: string): string =>
  [
    `Make a wisp plugin that does this: ${ask.trim()}`,
    '',
    'Write it as a user plugin, not a change to wisp or to this repo: run `wisp plugin new <short-name>` (it makes ~/.wisp/plugins/<short-name>/wisp-plugin.json), build what is asked there, then `wisp plugin install ~/.wisp/plugins/<short-name>`.',
    'It installs off. End by saying what it asks for, and that it is switched on in Settings → Plugins.',
  ].join('\n');
