// A finished turn's full transcript (v1 features/transcript.js renderTranscript), drawn with the kit's Timeline:
// the head ("Transcript · 1 message · 4 steps"), each step on the rail (an edit with its +/−, a command that isn't
// already its label, what it printed or the error in full behind a chevron), the agent's messages as prose, and
// where the turn ended. The answer follows it (TaskView). What a step printed of a .env file (`cat .env`) reads
// KEY=•••• under v1's Reveal line (env-reveal.ts keeps what's shown, per task).
import { Index, Show, createMemo, createSignal, type JSX } from 'solid-js';
import { Answer, EnvMask, Icon, Markdown, Text, Timeline, TimelineEnd, TimelineGroup, TimelineMessage, TimelineStep, DOTS, envFileNamed, envName, maskText, readsEnvFile, type BlockContext, type EnvReveal, type IconName } from '@wisp/ui';
import type { ShellContext } from '../contract.ts';
import { parseInlineChatBlocks } from './chat-blocks.ts';
import { changeFigures } from './rows-view.ts';
import { clusterLabel, groupFinishedSteps, type ClusterCounts } from './transcript-group.ts';
import '../i18n/lazy.ts';

const ICON: Readonly<Record<string, IconName>> = { read: 'read', edit: 'pen', run: 'terminal', error: 'warn', text: 'text', thinking: 'spark', tool: 'tool', search: 'search', agent: 'agent' };

function groupIcon(c: ClusterCounts): IconName {
  if ((c.edits ?? 0) > 0) return 'pen';
  if (c.reads > 0) return 'read';
  if (c.searches > 0) return 'search';
  if (c.commands > 0) return 'terminal';
  if (c.thoughts > 0) return 'spark';
  return 'tool';
}

export interface TranscriptStepRow {
  readonly kind: string;
  readonly label?: string;
  /** the whole title when the label was cut (v1 #780): what the opened step shows */
  readonly title?: string;
  readonly detail?: string;
  readonly out?: string;
  readonly bad?: boolean;
  readonly file?: string;
  /** the event (its per-session seq) that carried `out`: what Reveal asks the server for */
  readonly seq?: number;
}

type Change = Parameters<typeof changeFigures>[0][number];

/** What a step's opened detail shows of its output: masked when it read a .env file and isn't revealed. The server
 *  sends it masked already (ADR-0050): shown, it is what Reveal brought back (`raw`), the masked text until then;
 *  masking here again is the second layer for an older server's step. */
export function stepOutput(a: TranscriptStepRow, maskOn: boolean, shown: boolean, raw?: string): { readonly text: string; readonly env: boolean } | undefined {
  if (a.out === undefined) return undefined;
  const masked = maskText(a.out);
  const env = maskOn && readsEnvFile(a) && (masked !== a.out || a.out.includes(DOTS));
  if (!env) return { text: a.out, env };
  // shown: the server's copy; until it comes, the masked text (an older server's step is its own copy already)
  return { text: shown ? (raw ?? (a.out.includes(DOTS) ? masked : a.out)) : masked, env };
}

function Step(props: {
  readonly a: TranscriptStepRow;
  readonly index: number;
  readonly changes: readonly Change[];
  readonly here: (s: string) => string;
  readonly env: EnvReveal | undefined;
  readonly t: ShellContext['t'];
}): JSX.Element {
  const [open, setOpen] = createSignal(false);
  // Read props.a and props.changes reactively: <Index> keeps this Step mounted while live tool_output/changes arrive.
  const id = (): string => (props.a.seq !== undefined ? `step:${props.a.seq}` : `index:${props.index}`);
  const shown = (): boolean => props.env?.shown(id()) ?? false;
  const rawOut = (): string | undefined => {
    const r = props.env?.raw?.(id());
    const v = r?.['output'] ?? r?.['text'];
    return typeof v === 'string' ? v : undefined;
  };
  const out = () => stepOutput(props.a, props.env?.on() ?? true, shown(), rawOut());
  const t = props.t;
  const nums = () => {
    if (props.a.kind !== 'edit') return undefined;
    const change = props.changes.find((c) => (c as { file?: string }).file === props.a.file);
    return change ? changeFigures([change]) : undefined;
  };
  const command = (): string | undefined =>
    props.a.kind === 'run' && props.a.detail && !(props.a.label ?? '').includes(props.a.detail.slice(0, 24)) ? props.here(props.a.detail) : undefined;
  const more = (): string | undefined => props.a.out ?? (props.a.kind === 'error' ? props.a.label : undefined);
  // a step whose label was cut opens to its whole title (v1 #779/#780), with its output when it has one
  const opens = (): boolean => more() !== undefined || props.a.title !== undefined;
  return (
    <TimelineStep
      icon={ICON[props.a.kind] ?? 'tool'}
      label={props.here(open() && props.a.title ? props.a.title : (props.a.label ?? props.a.kind))}
      kind={props.a.kind}
      {...(command() !== undefined ? { command: command() as string } : {})}
      {...(nums() ? { nums: { add: nums()!.plus, del: nums()!.minus } } : {})}
      {...(props.a.bad || props.a.kind === 'error' ? { tone: 'bad' as const } : props.a.kind === 'thinking' ? { tone: 'quiet' as const } : {})}
      {...(opens() && {
        expanded: open(),
        onToggle: () => setOpen(!open()),
        detail: () => {
          const m = more();
          return m === undefined ? null : (
            <>
              <Show when={out()?.env}>
                <EnvMask
                  part="turn.transcript.envMask"
                  shown={shown()}
                  what={t(shown() ? 'shell.env.shown' : 'shell.env.hidden', { name: envName(props.a.file || envFileNamed(props.a.detail) || envFileNamed(props.a.label)) })}
                  action={t(shown() ? 'shell.env.hide' : 'shell.env.reveal')}
                  onToggle={(on) => props.env?.set(id(), on)}
                />
              </Show>
              <pre>{props.here(out()?.text ?? m)}</pre>
            </>
          );
        },
      })}
    />
  );
}

export function TurnTranscript(props: {
  readonly steps: readonly TranscriptStepRow[];
  /** masked .env output shown (Reveal), per task; without it the output stays masked */
  readonly env?: EnvReveal;
  readonly changes: readonly Change[];
  readonly here: (s: string) => string;
  readonly status: string;
  /** Chat mode: inline stream without the Transcript head or Finished footer. */
  readonly chat?: boolean;
  /** Chat mode with Transcript off: fold every run of actions between messages into a compact expandable pill. */
  readonly quiet?: boolean;
  /** Block host context so mid-chat messages containing ```wisp fences render rich blocks inline. */
  readonly answerCtx?: BlockContext;
  readonly t: ShellContext['t'];
}): JSX.Element {
  const t = props.t;
  const live = (): boolean => props.status === 'running' || props.status === 'starting';
  const msgs = (): number => props.steps.filter((a) => a.kind === 'text').length;
  const steps = (): number => props.steps.length - msgs();
  const entries = createMemo(() => groupFinishedSteps(props.steps, live(), props.here, props.quiet === true));
  const [opened, setOpened] = createSignal<ReadonlySet<string>>(new Set());
  const toggleGroup = (key: string): void => {
    setOpened((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };
  const count = (): string =>
    [
      msgs() ? t(msgs() === 1 ? 'shell.task.messages.one' : 'shell.task.messages.other', { n: msgs() }) : '',
      steps() ? t(steps() === 1 ? 'shell.task.steps.one' : 'shell.task.steps.other', { n: steps() }) : '',
    ]
      .filter(Boolean)
      .join(' · ');
  return (
    <Timeline
      part="turn.transcript"
      live={live()}
      {...(props.chat !== undefined && { chat: props.chat })}
      {...(!props.chat && {
        head: (
          <>
            <Icon name="transcript" size="sm" />
            <Text size="sm" weight="medium" tone="muted">
              {t('shell.task.transcript')}
            </Text>
            <Show when={count()}>
              <Text size="sm" tone="faint" truncate>{`· ${count()}`}</Text>
            </Show>
          </>
        ),
      })}
    >
      {/* by place, updated in place: the steps are made again as new objects whenever the turn's row changes (a 👍),
          and a list keyed by object made every step again */}
      <Index each={entries()}>
        {(entry) => (
          <Show
            when={entry().kind === 'group'}
            fallback={(() => {
              const s = (): TranscriptStepRow => (entry() as { kind: 'step'; step: TranscriptStepRow; index: number }).step;
              const idx = (): number => (entry() as { kind: 'step'; step: TranscriptStepRow; index: number }).index;
              return (
                <Show
                  when={s().kind === 'text'}
                  fallback={<Step a={s()} index={idx()} changes={props.changes} here={props.here} env={props.env} t={t} />}
                >
                  <TimelineMessage>
                    {(() => {
                      const raw = props.here(s().label ?? '');
                      const ctx = props.answerCtx;
                      const inline = props.chat && ctx ? parseInlineChatBlocks(raw) : undefined;
                      return inline && ctx ? <Answer blocks={inline} ctx={ctx} /> : <Markdown source={raw} />;
                    })()}
                  </TimelineMessage>
                </Show>
              );
            })()}
          >
            {(() => {
              const g = () => entry() as Extract<ReturnType<typeof entries>[number], { kind: 'group' }>;
              return (
                <TimelineGroup
                  part="turn.transcript.group"
                  icon={groupIcon(g().counts)}
                  label={clusterLabel(g().counts, g().items.length, t)}
                  count={g().items.length}
                  files={g().files}
                  expanded={opened().has(g().key)}
                  onToggle={() => toggleGroup(g().key)}
                >
                  {g().rest.length > 0 ? (
                    <Index each={g().rest}>
                      {(it) => <Step a={it().step} index={it().index} changes={props.changes} here={props.here} env={props.env} t={t} />}
                    </Index>
                  ) : undefined}
                </TimelineGroup>
              );
            })()}
          </Show>
        )}
      </Index>
      {/* a running turn's timeline so far (v1 renderLiveTranscript) has no end yet; Chat mode omits Finished */}
      <Show when={!props.chat && !live()}>
        <TimelineEnd
          icon={props.status === 'error' ? 'warn' : 'done'}
          label={t(props.status === 'error' ? 'shell.task.endedError' : 'shell.task.finished')}
          tone={props.status === 'error' ? 'bad' : 'good'}
        />
      </Show>
    </Timeline>
  );
}
