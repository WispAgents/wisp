// What waits on you in an open task, from its row (runtime keeps it in `ext["wisp.runtime"].ask` and `queue`):
// the agent's question or a tool's approval, and the follow-ups queued behind the running turn. Pure.

export interface Ask {
  readonly id: string;
  readonly question: string;
  readonly options: readonly string[];
  /** a tool asking to run: Allow / Always / Deny */
  readonly approval: boolean;
  /** an agent asking to change the user's setup (ADR-0113): Allow / Deny, what it would change, each command verbatim */
  readonly setup?: SetupAsk;
}
export interface SetupAsk {
  /** `Ada wants to add the connector `linear`: runs `npx -y @linear/mcp`` */
  readonly headline: string;
  /** the other facts, one a line (`role: Reviewer`, `needs LINEAR_KEY from you`) */
  readonly lines: readonly string[];
  /** every command the change would run, verbatim */
  readonly commands: readonly string[];
  /** everything that will be written, in full (Show all: kit bodies, briefs, a shared crew's members) */
  readonly full: string;
}
export interface Queued {
  readonly id: string;
  readonly text: string;
}

type Raw = { readonly body?: { readonly ext?: Record<string, unknown>; readonly queue?: readonly unknown[] }; readonly queue?: readonly unknown[] };

const str = (v: unknown): string => (typeof v === 'string' ? v : '');

/** The live question, if one waits. */
export function askOf(row: unknown): Ask | undefined {
  const r = row as Raw | undefined;
  const a = (r?.body?.ext?.['wisp.runtime'] as { ask?: Record<string, unknown> | null } | undefined)?.ask;
  if (!a || typeof a !== 'object') return undefined;
  const options = Array.isArray(a['options']) ? a['options'].map((o) => (typeof o === 'string' ? o : str((o as { label?: unknown })?.label))).filter(Boolean) : [];
  const question = str(a['question']);
  if (!question) return undefined;
  const id = str(a['id']) || question;
  // runtime's `setup`: an agent asking to change the user's setup, answered Allow or Deny, never Always
  const setup = setupOf(a['setup']);
  if (setup) return { id, question, options, approval: false, setup };
  // runtime's ask carries `permission` for a live approval (the tool, its detail, what it would do)
  const approval = !!a['permission'] || a['kind'] === 'approval' || options.some((o) => o.toLowerCase() === 'allow');
  return { id, question, options, approval };
}

const strs = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string' && x !== '') : []);

function setupOf(v: unknown): SetupAsk | undefined {
  if (!v || typeof v !== 'object') return undefined;
  const o = v as Record<string, unknown>;
  const headline = str(o['headline']);
  return headline ? { headline, lines: strs(o['lines']), commands: strs(o['commands']), full: str(o['full']) } : undefined;
}

/** The follow-ups waiting for the running turn, in order. */
export function queueOf(row: unknown): Queued[] {
  const r = row as Raw | undefined;
  const q = r?.body?.queue ?? r?.queue ?? [];
  return q.map((x) => ({ id: str((x as { id?: unknown }).id), text: str((x as { text?: unknown }).text) })).filter((x) => x.id && x.text);
}
