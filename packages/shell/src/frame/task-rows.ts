// The open task's rows as its topic sends them (runtime's turn and event rows), and the small helpers the task page
// and its turns share (TaskView.tsx, TaskTurn.tsx).
import type { IconName } from '@wisp/ui';

export interface TurnBody {
  readonly prompt?: string;
  /** the files sent with the prompt (runtime's Attachment: an upload's id, name and mime) */
  readonly attachments?: readonly { readonly id?: string; readonly name?: string; readonly mime?: string }[];
  readonly startedAt?: number;
  readonly endedAt?: number | null;
  readonly checkpoint?: { readonly commit?: string; readonly branch?: string };
  readonly presentation?: { readonly blocks?: readonly { readonly type: string; readonly title?: string; readonly summary?: string }[] };
  readonly stats?: { readonly costUsd?: number; readonly durationMs?: number; readonly inputTokens?: number; readonly outputTokens?: number };
  readonly activity?: readonly { readonly kind: string; readonly label?: string }[];
  readonly raw?: string;
  readonly presentationNotes?: string;
  readonly changes?: readonly { readonly file: string; readonly edits?: readonly unknown[] }[];
  /** where runtime keeps a turn's changes today */
  readonly ext?: { readonly 'wisp.runtime'?: { readonly changes?: readonly { readonly file: string; readonly edits?: readonly unknown[] }[] } };
}

/** A step of a turn, as the task's topic sends it (runtime's event log: `kind`, and a tool's `tool` and `label`). */
/** A tool's output carried by its own `tool_output` event, as a step's `out` and the event that carried it. */
export interface EventRow {
  readonly seq: number;
  readonly turn_id: string;
  /** when it happened (ms) */
  readonly t?: number;
  readonly kind?: string;
  readonly body?: {
    readonly kind?: string;
    readonly tool?: string;
    readonly label?: string;
    /** its whole title when the label was cut (v1 #780) */
    readonly title?: string;
    readonly text?: string;
    readonly detail?: string;
    readonly file?: string;
    readonly output?: string;
    readonly callId?: string;
    /** when a call's result came in */
    readonly outAt?: number;
    readonly name?: string;
  };
}

export interface TurnRow {
  readonly id: string;
  readonly session_id: string;
  readonly idx: number;
  readonly prompt: string;
  readonly started_at: number;
  readonly status?: string;
  /** the agent's final message as it wrote it (the presentation's blocks are parsed from it) */
  readonly answer?: string | null;
  readonly body: TurnBody;
}

export const ACT_ICON: Readonly<Record<string, IconName>> = { read: 'read', edit: 'pen', run: 'terminal', error: 'warn', text: 'text', thinking: 'spark', tool: 'tool' };

/** The markdown text of a turn's final answer (used to deduplicate trailing text events in Chat mode). */
export function finalMarkdownOf(blocks: readonly { readonly type: string; readonly text?: string }[]): string | undefined {
  for (let i = blocks.length - 1; i >= 0; i--) {
    const b = blocks[i];
    if (b?.type === 'markdown' && typeof b.text === 'string' && b.text.trim()) return b.text;
  }
  return undefined;
}

/** An object that reads `of()`'s latest value on every property read: in a JSX attribute or a memo that read is
 * tracked, so a turn's view follows its row without being made again (B35). */
export function through<T extends object>(of: () => T | undefined): T {
  return new Proxy({} as T, {
    get: (_t, k) => (of() as Record<PropertyKey, unknown> | undefined)?.[k],
    has: (_t, k) => k in ((of() ?? {}) as object),
    ownKeys: () => Reflect.ownKeys((of() ?? {}) as object),
    getOwnPropertyDescriptor: (_t, k) => {
      const v = of() as Record<PropertyKey, unknown> | undefined;
      return v && k in v ? { value: v[k], enumerable: true, configurable: true } : undefined;
    },
  });
}
