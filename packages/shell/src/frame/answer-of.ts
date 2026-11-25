// What an answer reads off its task and turn (answer-ctx.ts): a named file's URL and what a turn's refusals allow.
// Plain, so their tests needn't compile the slot and the kit that answer-ctx.ts reaches.

/** A picture or video an answer named → a URL the page loads (v1 ctx.fileUrl): a link stays; a file comes from the
 *  task's own computer, this wisp's or another through it (`/api/peer/<machine>`). */
export function mediaUrl(home: string, key: string, path: string): string | undefined {
  if (!path) return undefined;
  if (/^(https?:|data:|blob:)/.test(path)) return path;
  const peer = key.slice(0, key.indexOf(':'));
  const base = peer && peer !== home ? `/api/peer/${encodeURIComponent(peer)}` : '';
  return `${base}/api/sessions/${encodeURIComponent(key.slice(key.indexOf(':') + 1))}/file?path=${encodeURIComponent(path)}`;
}

/** What a turn's refusals allow (v1 #777): the server keeps them (`ext['wisp.runtime'].blocked`) only for a run that
 *  wasn't in Full auto; a Full auto run that its harness still refused can't be retried, and a card from before
 *  has nothing to allow. */
export function blockedOf(turn: unknown): 'allowable' | 'unallowed' | 'none' {
  const o = (turn ?? {}) as { readonly ext?: Readonly<Record<string, unknown>>; readonly mode?: unknown };
  const kept = (o.ext?.['wisp.runtime'] as { readonly blocked?: unknown } | undefined)?.blocked;
  if (Array.isArray(kept) && kept.length > 0) return 'allowable';
  return o.mode === 'auto' ? 'unallowed' : 'none';
}

/** A file sent with a prompt (a turn's attachment) → its bytes' URL (v1 turns.js `/api/uploads/<id>`), through the
 *  task's own computer when it is another one. No id (a channel's file that never was an upload): none. */
export function uploadUrl(home: string, key: string, id: string): string | undefined {
  if (!id) return undefined;
  const peer = key.slice(0, key.indexOf(':'));
  const base = peer && peer !== home ? `/api/peer/${encodeURIComponent(peer)}` : '';
  return `${base}/api/uploads/${encodeURIComponent(id)}`;
}

const stripPrivate = (p: string): string => p.replace(/\\/g, '/').replace(/^\/private(?=\/)/, '').replace(/\/+$/, '');

/** Maps a `doc` block's path to a clean path relative to the task's folder `dir` for live reads (`worktrees.source`).
 *  Returns undefined for paths outside `dir`, parent traversals, or `.env` files. */
export function docRelativePath(dir: string, path: string): string | undefined {
  const raw = path.trim();
  if (!raw) return undefined;
  const base = raw.split(/[\\/]/).pop() ?? '';
  if (/^\.env($|\.)/i.test(base) && !/^\.env\.(example|sample|template|dist|defaults|schema)$/i.test(base)) return undefined;
  const norm = raw.replace(/\\/g, '/');
  let rel: string;
  if (norm.startsWith('/') || /^[A-Za-z]:\//.test(norm)) {
    const root = stripPrivate(dir.trim());
    const target = stripPrivate(norm);
    if (!root || !target.startsWith(`${root}/`)) return undefined;
    rel = target.slice(root.length + 1);
  } else if (norm.startsWith('~')) {
    return undefined;
  } else {
    rel = norm;
  }
  const parts = rel.split('/').filter((s) => s !== '' && s !== '.');
  if (parts.length === 0 || parts.some((s) => s === '..')) return undefined;
  return parts.join('/');
}
